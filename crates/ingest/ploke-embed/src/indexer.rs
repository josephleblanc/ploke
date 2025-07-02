use ploke_core::TrackingHash;
use ploke_db::{embedding::EmbeddingNode, Database};
use ploke_io::{IoManagerHandle, SnippetRequest};
use std::sync::Arc;
use tracing::{info_span, instrument};

use crate::error::BatchError;

// Replace trait with concrete processor
pub struct EmbeddingProcessor {
    // Will support multi-backend later
    local_backend: Option<LocalModelBackend>,
}

pub struct LocalModelBackend {
    dummy_dimensions: usize,
}

impl LocalModelBackend {
    pub fn dummy() -> Self {
        Self {
            dummy_dimensions: 384,
        }
    }

    pub fn dimensions(&self) -> usize {
        self.dummy_dimensions
    }

    pub async fn compute_batch(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, ploke_error::Error> {
        // Dummy implementation
        Ok(snippets
            .into_iter()
            .map(|_| vec![0.0; self.dummy_dimensions])
            .collect())
    }
}

impl EmbeddingProcessor {
    pub async fn generate_embeddings(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, ploke_error::Error> {
        match &self.local_backend {
            Some(backend) => backend.compute_batch(snippets).await,
            None => Err(BatchError::Generic("No embedding backend configured".into()).into()),
        }
    }

    pub fn dimensions(&self) -> usize {
        self.local_backend
            .as_ref()
            .map(|b| b.dimensions())
            .unwrap_or(0)
    }
}

use candle_core::{Device, Tensor};
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

// A helper struct to manage the model and tokenizer
pub struct EmbeddingModel {
    model: BertModel,
    tokenizer: Tokenizer,
}

impl EmbeddingModel {
    pub fn new() -> Result<Self> {
        // Use CPU by default, or check for CUDA feature
        let device = Device::Cpu; 

        // Download model and tokenizer files from Hugging Face Hub
        let api = Api::new()?;
        let repo = api.repo(Repo::new(
            "sentence-transformers/all-MiniLM-L6-v2".to_string(), // A good starting model
            RepoType::Model,
        ));
        let config_filename = repo.get("config.json")?;
        let tokenizer_filename = repo.get("tokenizer.json")?;
        let weights_filename = repo.get("model.safetensors")?;

        // Setup the model configuration and load weights
        let config = std::fs::read_to_string(config_filename)?;
        let config: Config = serde_json::from_str(&config)?;
        let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(anyhow::Error::msg)?;
        
        let vb = candle_nn::VarBuilder::from_safetensors(vec![weights_filename], DTYPE, &device)?;
        let model = BertModel::load(vb, &config)?;

        Ok(Self { model, tokenizer })
    }

    pub fn get_embeddings(&self, snippets: &[&str]) -> Result<Tensor> {
        let tokens = self
            .tokenizer
            .encode_batch(snippets.to_vec(), true)
            .map_err(anyhow::Error::msg)?;

        let token_ids = tokens
            .iter()
            .map(|t| Ok(Tensor::new(t.get_ids(), &self.model.device)?))
            .collect::<Result<Vec<_>>>()?;

        let token_ids = Tensor::stack(&token_ids, 0)?;
        
        // Run inference
        let embeddings = self.model.forward(&token_ids)?;

        // Perform pooling (mean of the last hidden state)
        // and normalization
        let (_n_sentence, n_tokens, _hidden_size) = embeddings.dims3()?;
        let embeddings = (embeddings.sum(1)? / (n_tokens as f64))?;
        let embeddings = embeddings.broadcast_div(&embeddings.sqr()?.sum_keepdim(1)?.sqrt()?)?;

        Ok(embeddings)
    }
}

pub struct IndexerTask {
    db: Arc<Database>,
    io: IoManagerHandle,
    embedding_processor: EmbeddingProcessor, // Static type
    cancellation_token: CancellationToken,
    batch_size: usize,
}

impl IndexerTask {
    async fn run(&self) -> Result<(), ploke_error::Error> {
        while let Some(batch) = self.next_batch().await? {
            process_batch(
                &self.db,
                &self.io,
                &self.embedding_processor,
                batch,
                |current, total| tracing::info!("Indexed {current}/{total}"),
            )
            .await?;
        }
        Ok(())
    }

    async fn next_batch(&self) -> Result<Option<Vec<EmbeddingNode>>, ploke_error::Error> {
        // State management - track last ID across batches
        static LAST_ID: tokio::sync::Mutex<Option<uuid::Uuid>> = tokio::sync::Mutex::const_new(None);
        
        let mut last_id_guard = LAST_ID.lock().await;
        let last_id = last_id_guard.take();

        // Fetch batch of nodes from database
        // TODO: Handle the error from get_nodes_from embedding better maybe? Does it need extra
        // information here?
        let batch = self.db.get_nodes_for_embedding(self.batch_size, last_id)?;
        
        // Update cursor for next batch
        *last_id_guard = batch.last().map(|node| node.id);

        // Handle cancellation token
        if self.cancellation_token.is_cancelled() {
            return Err(BatchError::Generic("Processing cancelled".into()).into())
        }

        match batch.is_empty() {
            true => Ok(None),
            false => Ok(Some(batch))
        }
    }
}

/// Processes a batch of nodes for embedding generation
#[instrument(skip_all, fields(batch_size = nodes.len()))]
pub async fn process_batch(
    db: &Database,
    io_manager: &IoManagerHandle,
    embedding_processor: &EmbeddingProcessor,
    nodes: Vec<EmbeddingNode>,
    report_progress: impl Fn(usize, usize) + Send + Sync,
) -> Result<(), ploke_error::Error> {
    let ctx_span = info_span!("process_batch");
    let _guard = ctx_span.enter();

    // Convert nodes to snippet requests (use file_tracking_hash)
    let requests = nodes
        .iter()
        .map(|node| SnippetRequest {
            path: node.path.clone(),
            file_tracking_hash: TrackingHash(node.file_tracking_hash),
            start: node.start_byte,
            end: node.end_byte,
        })
        .collect::<Vec<_>>();

    // Fetch snippets
    let snippet_results = io_manager
        .get_snippets_batch(requests)
        .await
        .map_err(ploke_io::IoError::Recv)?;

    // Batch snippets and nodes for efficiency
    let mut valid_snippets = Vec::new();
    let mut valid_nodes = Vec::new();
    let mut valid_indices = Vec::new();
    let total_nodes = nodes.len();

    for (i, (node, snippet_result)) in nodes.into_iter().zip(snippet_results).enumerate() {
        report_progress(i, total_nodes);
        match snippet_result {
            Ok(snippet) => {
                valid_snippets.push(snippet);
                valid_nodes.push(node);
                valid_indices.push(i);
            }
            Err(e) => tracing::warn!("Snippet error: {:?}", e),
        }
    }

    // Process embeddings in batch
    let embeddings = embedding_processor
        .generate_embeddings(valid_snippets)
        .await
        .map_err(BatchError::Embedding)?;

    // Validate vector dimensions
    let dims = embedding_processor.dimensions();
    for (i, embedding) in embeddings.iter().enumerate() {
        if embedding.len() != dims {
            return Err(BatchError::DimensionMismatch {
                expected: dims,
                actual: embedding.len(),
            }.into());
        }
        report_progress(valid_indices[i], total_nodes);
    }

    // Prepare updates using valid nodes and embeddings
    let updates = valid_nodes
        .into_iter()
        .zip(embeddings)
        .map(|(node, embedding)| (node.id, embedding))
        .collect();

    // Update database in bulk
    db.update_embeddings_batch(updates)
        .await
        .map_err(BatchError::Database)?;

    report_progress(total_nodes, total_nodes);
    Ok(())
}

use tokio::sync::watch;

pub struct CancellationToken {
    pub token: Arc<watch::Receiver<bool>>,
}

impl CancellationToken {
    pub(crate) fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { token: Arc::new(rx) }
    }

    pub fn is_cancelled(&self) -> bool {
        *self.token.borrow()
    }

    // pub fn cancel(&self) {
    //     match self.token.sender() {
    //         Some(tx) => {
    //             let _ = tx.send(true);
    //         }
    //         _ => (),
    //     }
    // }
}
