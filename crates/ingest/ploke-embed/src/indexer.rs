use ploke_core::TrackingHash;
use ploke_db::{embedding::EmbeddingNode, Database};
use ploke_io::{IoManagerHandle, SnippetRequest};
use std::sync::Arc;
use tracing::{info_span, instrument};

use crate::error::EmbedError;

pub struct EmbeddingProcessor {
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
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        snippets.into_iter()
            .map(|_| vec![0.0; self.dummy_dimensions])
            .map(|v| Ok(v))
            .collect()
    }
}

impl EmbeddingProcessor {
    pub async fn generate_embeddings(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        match &self.local_backend {
            Some(backend) => backend.compute_batch(snippets).await,
            None => Err(EmbedError::Generic("No embedding backend configured".to_string())),
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
    embedding_processor: EmbeddingProcessor,
    cancellation_token: CancellationToken,
    batch_size: usize,
}

impl IndexerTask {
    async fn run(&self) -> Result<(), EmbedError> {
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

    async fn next_batch(&self) -> Result<Option<Vec<EmbeddingNode>>, EmbedError> {
        static LAST_ID: tokio::sync::Mutex<Option<uuid::Uuid>> = tokio::sync::Mutex::const_new(None);
        
        let mut last_id_guard = LAST_ID.lock().await;
        let last_id = last_id_guard.take();

        let batch = self.db.get_nodes_for_embedding(self.batch_size, last_id)?;
        
        *last_id_guard = batch.last().map(|node| node.id);

        if self.cancellation_token.is_cancelled() {
            return Err(EmbedError::Cancelled("Processing cancelled".into()));
        }

        match batch.is_empty() {
            true => Ok(None),
            false => Ok(Some(batch))
        }
    }
}

#[instrument(skip_all, fields(batch_size = nodes.len()))]
pub async fn process_batch(
    db: &Database,
    io_manager: &IoManagerHandle,
    embedding_processor: &EmbeddingProcessor,
    nodes: Vec<EmbeddingNode>,
    report_progress: impl Fn(usize, usize) + Send + Sync,
) -> Result<(), EmbedError> {
    let ctx_span = info_span!("process_batch");
    let _guard = ctx_span.enter();

    let requests = nodes
        .iter()
        .map(|node| SnippetRequest {
            path: node.path.clone(),
            file_tracking_hash: TrackingHash(node.file_tracking_hash),
            start: node.start_byte,
            end: node.end_byte,
        })
        .collect::<Vec<_>>();

    let snippet_results = io_manager
        .get_snippets_batch(requests)
        .await
        .map_err(EmbedError::SnippetFetch)?;

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

    let embeddings = embedding_processor
        .generate_embeddings(valid_snippets)
        .await?;

    let dims = embedding_processor.dimensions();
    for embedding in &embeddings {
        if embedding.len() != dims {
            return Err(EmbedError::DimensionMismatch {
                expected: dims,
                actual: embedding.len(),
            });
        }
    }

    let updates = valid_nodes
        .into_iter()
        .zip(embeddings)
        .map(|(node, embedding)| (node.id, embedding))
        .collect();

    db.update_embeddings_batch(updates)
        .await?;

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
}
