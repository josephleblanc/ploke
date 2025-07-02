use std::sync::Arc;
use ploke_core::TrackingHash;
use ploke_db::{embedding::EmbeddingNode, Database};
use ploke_io::{IoManagerHandle, SnippetRequest};
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
        Self { dummy_dimensions: 384 }
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
            None => Err(ploke_error::Error::Internal(
                ploke_error::InternalError::Generic("No embedding backend configured".into()),
            )),
        }
    }

    pub fn dimensions(&self) -> usize {
        self.local_backend
            .as_ref()
            .map(|b| b.dimensions())
            .unwrap_or(0)
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
    async fn run(&self) -> Result<(), BatchError> {
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

    async fn next_batch(&self) -> Result<Option<Vec<EmbeddingNode>>, BatchError> {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    #[error("Snippet fetch error: {0}")]
    SnippetFetch(#[from] ploke_error::Error),
    #[error("Embedding generation error: {0}")]
    Embedding(#[from] ploke_error::Error),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Dimension mismatch: expected {expected}, actual {actual}")]
    DimensionMismatch {
        expected: usize,
        actual: usize,
    },
}

/// Processes a batch of nodes for embedding generation
#[instrument(skip_all, fields(batch_size = nodes.len()))]
pub async fn process_batch(
    db: &Database,
    io_manager: &IoManagerHandle,
    embedding_processor: &EmbeddingProcessor,
    nodes: Vec<EmbeddingNode>,
    report_progress: impl Fn(usize, usize) + Send + Sync,
) -> Result<(), BatchError> {
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
        .map_err(|e| BatchError::SnippetFetch(e.into()))?;

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
            });
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
        .map_err(|e| BatchError::Database(format!("{:?}", e)))?;

    report_progress(total_nodes, total_nodes);
    Ok(())
}

pub struct CancellationToken {}
