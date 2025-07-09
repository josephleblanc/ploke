
use std::{collections::HashMap, fmt, path::PathBuf};
use candle_core::{safetensors, DType, Device, Error as CandleError, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, HiddenAct};
use hf_hub::{api::sync::Api, api::sync::ApiError as HubError, Repo, RepoType};
use ploke_error::Error as PlokeError;
use ploke_transform::error::TransformError;
use tokenizers::{Error as TokenizerError, PaddingParams, Tokenizer, TruncationParams};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("Tokenizer initialization failed: {0}")]
    Tokenizer(#[from] TokenizerError),
    #[error("Model download failed: {0}")]
    ModelDownload(#[from] HubError),
    #[error("I/O operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Tensor operation failed: {0}")]
    Tensor(#[from] CandleError),
    #[error("Serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid model configuration: {0}")]
    Config(String),
    #[error("Batch processing failed: {0}")]
    BatchProcessing(String),
    #[error("Dimension mismatch: {0}")]
    Dimension(String),
    #[error("Empty input batch")]
    EmptyBatch,
    #[error("Device selection failed: {0}")]
    DeviceSelection(String),
    #[error("Safetensors load failed: {0}")]
    Safetensors(String),
    #[error("Invalid model configuration: {0}")]
    ModelConfig(String),
    #[error("Unsupported model format: {0}")]
    UnsupportedFormat(String),
    #[error("GPU required but unavailable")]
    GpuUnavailable,
    #[error("Tokenization failed: {0}")]
    Tokenization(String),
    #[error("Tokenization failed: {0}")]
    Timeout(#[from] tokio::task::JoinError),
}

impl From<EmbeddingError> for PlokeError {
    fn from(error: EmbeddingError) -> Self {
        match error {
            // Classify as InternalError for infrastructure issues
            EmbeddingError::ModelDownload(_) | 
            // TODO: Not sure whether this makes sense, improve later when refactoring error
            // handling
            EmbeddingError::Io(_) => 
                 error.into(),
            
            // All others as TransformError
            // TODO: Placeholder, improve later.
            _ => TransformError::Transformation(error.to_string()).into(),
        }
    }
}


impl fmt::Debug for LocalEmbedder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalEmbedder")
         .field("tokenizer", &self.tokenizer)
         .field("device", &self.device)
         .field("max_length", &self.max_length)
         .field("model", &"BertModel { ... }") // Placeholder for non-Debug model
         .finish()
    }
}

use tracing::{info, instrument, warn};

#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub model_id: String,
    pub revision: Option< String >,
    pub device_preference: DevicePreference,
    pub cuda_device_index: usize,
    pub allow_fallback: bool,
    pub approximate_gelu: bool,
    pub use_pth: bool,
    pub batch_size: usize,  // NEW: Configurable batch size
    pub max_length: Option<usize>,  // NEW: Optional max length override
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
                    model_id: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                    revision: None,
                    device_preference: DevicePreference::Auto,
                    cuda_device_index: 0,
                    allow_fallback: true,
                    approximate_gelu: false,
                    use_pth: false,
                    batch_size: 8,
                    max_length: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DevicePreference {
    #[default]
    Auto,
    ForceCpu,
    ForceGpu,
}

pub struct LocalEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    config: EmbeddingConfig,
    max_length: usize,
    dimensions: usize,
}

impl LocalEmbedder {
    pub fn new(config: EmbeddingConfig) -> Result<Self, EmbeddingError> {
        let device = Self::select_device(&config)?;
        let (model, mut tokenizer, model_config) = Self::load_model(&config, &device)?;
        
        // Determine max length (user override or model default)
        let max_length = config.max_length
            .unwrap_or(model_config.max_position_embeddings);
        
        // Configure tokenizer
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length,
                stride: 0,
                strategy: tokenizers::TruncationStrategy::LongestFirst,
                direction: tokenizers::TruncationDirection::Right,
            }))
            .map_err(EmbeddingError::Tokenizer)?
            .with_padding(Some(PaddingParams {
                strategy: tokenizers::PaddingStrategy::BatchLongest,
                ..Default::default()
            }));
            // .map_err(|e| EmbeddingError::Tokenizer(e.into()))?;

        Ok(Self {
            model,
            tokenizer,
            device,
            dimensions: model_config.hidden_size,
            max_length,
            config,
        })
    }

    fn select_device(config: &EmbeddingConfig) -> Result<Device, EmbeddingError> {
        match config.device_preference {
            DevicePreference::ForceCpu => Ok(Device::Cpu),
            
            DevicePreference::ForceGpu => {
                Device::new_cuda(config.cuda_device_index).or_else(|e| {
                    if config.allow_fallback {
                        tracing::warn!("GPU unavailable ({e}), falling back to CPU");
                        Ok( Device::Cpu )
                    } else {
                        Err( EmbeddingError::GpuUnavailable )
                    }
                })
            }
            
            DevicePreference::Auto => {
                Device::cuda_if_available(config.cuda_device_index).or_else(|e| {
                    tracing::warn!("GPU unavailable ({e}), falling back to CPU");
                    Ok(Device::Cpu)
                })
            }
        }
    }

     fn load_model(
         config: &EmbeddingConfig,
         device: &Device,
     ) -> Result<(BertModel, Tokenizer, Config), EmbeddingError> {
         let api = Api::new().map_err(EmbeddingError::ModelDownload)?;
         let repo = match &config.revision {
             Some(revision) => Repo::with_revision(
                 config.model_id.clone(),
                 RepoType::Model,
                 revision.clone(),
             ),
             None => Repo::new(config.model_id.to_owned(), RepoType::Model)
        };

        // Get repository API handle
        let repo_api = api.repo(repo);

        // Download and validate config
        let config_path = repo_api.get("config.json")
            .map_err(EmbeddingError::ModelDownload)?;
        let config_str = std::fs::read_to_string(&config_path)?;
        // NOTE: The current default model "sentence-transformers/all-MiniLM-L6-v2" has this size,
        // but that does not mean that each other model will as well. We should probably find a
        // good way to configure this in a good way.
        // Self::validate_file_size(&config_path, 612)?;
        let mut model_config: Config = serde_json::from_str(&config_str)
            .map_err(|e| EmbeddingError::Config(e.to_string()))?;

        if config.approximate_gelu {
            model_config.hidden_act = HiddenAct::GeluApproximate;
        }

        // Download and validate tokenizer
        let tokenizer_path = repo_api.get("tokenizer.json")
            .or_else(|_| repo_api.get("tokenizer.model"))?;
        Self::validate_file_size(&tokenizer_path, 1024)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(EmbeddingError::Tokenizer)?;

        // Download weights with priority: safetensors > pth
        let weights_path = if config.use_pth {
            repo_api.get("pytorch_model.bin")?
        } else {
            // Try safetensors first, then PyTorch weights
            repo_api.get("model.safetensors")
                .or_else(|_| repo_api.get("pytorch_model.bin"))?
        };
        Self::validate_file_size(&weights_path, 1024)?;

        let is_safetensors = weights_path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext == "safetensors")
            .unwrap_or(false);
        
        let vb = if is_safetensors {
            Self::load_safetensors(&weights_path, device)
        } else {
            VarBuilder::from_pth(&weights_path, DType::F32, device)
                .map_err(EmbeddingError::Tensor)
        }?;
        
        let model = BertModel::load(vb, &model_config)
            .map_err(|e| EmbeddingError::ModelConfig(e.to_string()))?;

         Ok((model, tokenizer, model_config))
     }

fn validate_file_size(path: &PathBuf, min_size: u64) -> Result<(), EmbeddingError> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() < min_size {
        return Err(EmbeddingError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("File too small: {} bytes (minimum expected: {})", 
                    metadata.len(), min_size),
        )));
    }
    Ok(())
}
    fn load_safetensors<'a>(
        path: &'a PathBuf,
        device: &'a Device,
    ) -> Result<VarBuilder<'a>, EmbeddingError> {
        let data = std::fs::read(path)?;
        let tensors = safetensors::load_buffer(&data, device)
            .map_err(|e| EmbeddingError::Safetensors(e.to_string()))?;
        
        Ok(VarBuilder::from_tensors(tensors.into_iter().collect(), DType::F32, device))
    }

    // fn validate_file_contents(path: &PathBuf) -> Result<(), EmbeddingError> {
    //     todo!()
    // }

    pub fn old_embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyBatch);
        }
        
        let mut results = Vec::new();
        for chunk in texts.chunks(8) {
            let batch_results = self.process_batch(chunk)?;
            results.extend(batch_results);
        }
        Ok(results)
    }

    fn old_process_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        // Tokenize with attention masks
        let tokens = self.tokenizer.encode_batch(texts.to_vec(), true)?;

        // Prepare inputs with proper error context
        let token_ids: Result<Vec<Tensor>, _> = tokens.iter()
            .map(|t| Tensor::new(t.get_ids(), &self.device))
            .collect();
        
        let attention_mask: Result<Vec<Tensor>, _> = tokens.iter()
            .map(|t| Tensor::new(t.get_attention_mask(), &self.device))
            .collect();

        let token_ids = Tensor::stack(&token_ids?, 0)?;
        let attention_mask = Tensor::stack(&attention_mask?, 0)?;
            // .to_dtype(DType::F32)?;

        // Forward pass with attention masks
        let token_type_ids = Tensor::zeros(token_ids.shape(), DType::F32, &self.device)?;

        let outputs = self.model.forward(
            &token_ids,
            &token_type_ids,  // token_type_ids argument
            None              // position_ids argument
        )
        .map_err(EmbeddingError::Tensor)?;

        // Mean pooling with attention masks
        let weights = attention_mask.broadcast_as(outputs.shape())
            .map_err(|e| EmbeddingError::Dimension(e.to_string()))?;
        
        let sum_embeddings = (&outputs * &weights)?.sum_keepdim(1)?;
        let sum_weights = weights.sum_keepdim(1)?.clamp(1e-9, f32::MAX)?;
        let embeddings = (sum_embeddings / sum_weights)?;

        // Normalize embeddings
        let embeddings = embeddings.broadcast_div(
            &embeddings.sqr()?.sum_keepdim(1)?.sqrt()?
        )?;

        // Convert to Vec<Vec<f32>> with proper error handling
        let mut results = Vec::with_capacity(texts.len());
        for i in 0..texts.len() {
            let row = embeddings.i((i, ..))
                .map_err(|_| EmbeddingError::Dimension(
                    format!("Embedding index {} out of range", i)
                ))?;
            results.push(row.to_vec1()?);
        }

        Ok(results)
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        // TODO: NOW
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyBatch);
        }
        
        let mut results = Vec::new();
        // FIXED: Use configurable batch size instead of hardcoded 8
        for chunk in texts.chunks(self.config.batch_size) {
            let batch_results = self.process_batch(chunk)?;
            results.extend(batch_results);
        }
        Ok(results)
    }

    #[instrument(
        skip(self, texts),
        fields(batch_size, cursor),
        level = "INFO"  // Demote to debug level
    )]
    fn process_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        // Tokenize with attention masks
        tracing::debug!("Processing tokens");
        let tokens = self.tokenizer.encode_batch(texts.to_vec(), true)?;

        // Prepare inputs with proper error context
        tracing::debug!("Processing token_ids");
        let token_ids: Result<Vec<Tensor>, _> = tokens.iter()
            .map(|t| Tensor::new(t.get_ids(), &self.device))
            .collect();
        
        tracing::debug!("Processing attention_mask");
        let attention_mask: Result<Vec<Tensor>, _> = tokens.iter()
            .map(|t| Tensor::new(t.get_attention_mask(), &self.device))
            .collect();

        // FIXED: Keep token_ids as the correct integer type (U32/I64)
        tracing::debug!("Processing token_ids with Tensor::stack");
        let token_ids = Tensor::stack(&token_ids?, 0)?;
        
        // FIXED: Keep attention_mask as integer type initially, convert later if needed
        tracing::debug!("Processing attention_mask with Tensor::stack");
        let attention_mask = Tensor::stack(&attention_mask?, 0)?;

        // FIXED: Create token_type_ids with the same dtype as token_ids (not F32)
        tracing::debug!("Processing token-type ids with Tensor::zeros");
        let token_type_ids = Tensor::zeros(token_ids.shape(), token_ids.dtype(), &self.device)?;

        // Forward pass with correct dtypes
        tracing::debug!("Processing outputs with self.model.forward");
        let outputs = self.model.forward(
            &token_ids,
            &token_type_ids,
            None
        )
        .map_err(EmbeddingError::Tensor)?;

        // FIXED: Convert attention_mask to F32 only when needed for arithmetic operations
        tracing::debug!("Prodcessing attention_mask_f32 with to_dtype(DType::F32)");
        let attention_mask_f32 = attention_mask.to_dtype(DType::F32)?;

        // Mean pooling with attention masks
        tracing::debug!("Prodcessing attention_mask_f32 with attention_mask_f32.broadcase_as");
        // Mean pooling with attention masks
        let weights = attention_mask_f32
            .unsqueeze(candle_core::D::Minus1)?
            .broadcast_as(outputs.shape())
            .map_err(|e| EmbeddingError::Dimension(e.to_string()))?;
        
        tracing::debug!("Prodcessing sums");
        let sum_embeddings = (&outputs * &weights)?.sum_keepdim(1)?;
        let sum_weights = weights.sum_keepdim(1)?.clamp(1e-9, f32::MAX)?;
        let embeddings = (sum_embeddings / sum_weights)?;

        // Normalize embeddings
        tracing::debug!("Normalize embeddings");
        let embeddings = embeddings.broadcast_div(
            &embeddings.sqr()?.sum_keepdim(1)?.sqrt()?
        )?;

        // Convert to Vec<Vec<f32>> with proper error handling
        tracing::debug!("Convert to Vec<Vec<f32>> with proper error handling");
        let mut results = Vec::with_capacity(texts.len());
        for i in 0..texts.len() {
            let row = embeddings.i((i, ..))
                .map_err(|e| EmbeddingError::Dimension(
                    format!("Embedding index {} out of range: {}", i, e)
                ))?
                .squeeze(0)?;
            results.push(row.to_vec1()?);
        }

        Ok(results)
    }
}

