use candle_core::{IndexOp, DType, Device, Tensor, Error as CandleError};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::Api, api::sync::ApiError as HubError, Repo, RepoType};
use ploke_error::Error as PlokeError;
use ploke_transform::error::TransformError;
use tokenizers::{Error as TokenizerError, PaddingParams, Tokenizer};
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

pub struct LocalEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    max_length: usize,
}

impl LocalEmbedder {
    pub fn new(model_id: &str) -> Result<Self, EmbeddingError> {
        let device = Device::cuda_if_available(0).or_else(|_| {
            tracing::warn!("CUDA not available, falling back to CPU");
            Ok::<Device, EmbeddingError>(Device::Cpu)
        })?;

        let api = Api::new().map_err(EmbeddingError::ModelDownload)?;
        let repo = Repo::new(model_id.to_string(), RepoType::Model);
        
        // Load configuration
        let config_path = api.repo(repo.clone()).get("config.json")?;
        let config: Config = serde_json::from_str(
            &std::fs::read_to_string(config_path)?
        )?;
        
        // Initialize tokenizer with smart padding
        let tokenizer_path = api.repo(repo.clone()).get("tokenizer.json")?;
        let mut tokenizer = Tokenizer::from_file(tokenizer_path)?;
        tokenizer.with_padding(Some(PaddingParams {
            pad_to_multiple_of: Some( 8 ),
            ..Default::default()
        }));

        // Load model weights
        let weights_path = api.repo(repo).get("model.safetensors")?;
        let vb = VarBuilder::from_pth(
            &weights_path, 
            DType::F32, 
            &device
        )?;
        
        let model = BertModel::load(vb, &config).map_err(|e| {
            EmbeddingError::Config(format!("Failed to load model: {}", e))
        })?;

        Ok(Self {
            model,
            tokenizer,
            device,
            // TODO: Consider how to use max_length
            max_length: 256,
        })
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
}

// Example usage at crate boundary
pub fn create_embedder() -> Result<LocalEmbedder, PlokeError> {
    LocalEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
        .map_err(Into::into)
}
