
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

pub struct LocalEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    config: EmbeddingConfig,
    max_length: usize,
    dimensions: usize,
}

use tracing::{info, warn};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DevicePreference {
    Auto,
    ForceCpu,
    ForceGpu,
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
        // Add token from environment
        let token = std::env::var("HF_TOKEN").ok();
        let api = api.with_token(token);

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
        Self::validate_file_contents(&config_path)?;
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| EmbeddingError::ModelDownload(e.to_string()))?;
        let mut model_config: Config = serde_json::from_str(&config_str)
            .map_err(|e| EmbeddingError::Config(e.to_string()))?;
        
        if config.approximate_gelu {
            model_config.hidden_act = HiddenAct::GeluApproximate;
        }

        // Download and validate tokenizer
        let tokenizer_path = repo_api.get("tokenizer.json")
            .or_else(|_| repo_api.get("tokenizer.model"))?;
        Self::validate_file_contents(&tokenizer_path)?;
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
        Self::validate_file_contents(&weights_path)?;

        let vb = if weights_path.extension().and_then(|e| e.to_str()) == Some("safetensors") {
            Self::load_safetensors(&weights_path, device)?
        } else {
            VarBuilder::from_pth(&weights_path, DType::F32, device)?
        };

        let model = BertModel::load(vb, &model_config)
            .map_err(|e| EmbeddingError::ModelConfig(e.to_string()))?;

        Ok((model, tokenizer, model_config))
    }

    fn load_safetensors<'a>(
        path: &'a PathBuf,
        device: &'a Device,
    ) -> Result<VarBuilder<'a>, EmbeddingError> {
        let data = std::fs::read(path)?;
        let tensors = safetensors::load_buffer(&data, device)
            .map_err(|e| EmbeddingError::Safetensors(e.to_string()))?;
        
        // Convert to F32 if needed
        let tensors = tensors
            .into_iter()
            .map(|(k, v)| {
                if v.dtype() != DType::F32 {
                    v.to_dtype(DType::F32)
                        .map(|v| (k, v))
                        .map_err(EmbeddingError::Tensor)
                } else {
                    Ok((k, v))
                }
            })
            .collect::<Result<HashMap<_, _>, _>>()?;
        
        Ok(VarBuilder::from_tensors(tensors.into_iter().collect(), DType::F32, device))
    }

    fn validate_file_contents(path: &PathBuf) -> Result<(), EmbeddingError> {
        let content = std::fs::read(path)?;
        if std::str::from_utf8(&content)
            .map(|s| s.contains("private or gated model"))? 
        {
            return Err(EmbeddingError::ModelDownload(
                "Authentication required for gated model".to_string()
            ));
        }
        Ok(())
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
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

    fn process_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
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
        let attention_mask = Tensor::stack(&attention_mask?, 0)?
            .to_dtype(DType::F32)?;

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
}

