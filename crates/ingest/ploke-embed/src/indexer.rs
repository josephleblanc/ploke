use ploke_core::TrackingHash;
use ploke_db::{embedding::EmbeddingNode, Database};
use ploke_io::{IoManagerHandle, SnippetRequest};
use std::sync::Arc;
use std::path::PathBuf;
use tracing::{info_span, instrument};

use crate::{cancel_token::CancellationToken, error::EmbedError};

#[derive(Debug)]
pub struct EmbeddingProcessor {
    local_backend: Option<LocalModelBackend>,
}

#[derive(Debug)]
pub struct LocalModelBackend {
    dummy_dimensions: usize,
}

pub type IndexProgress = f64;
 // New state to track indexing
 #[derive(Debug, Clone)]
 pub struct IndexingStatus {
     pub status: IndexStatus,
     pub processed: usize,
     pub total: usize,
     pub current_file: Option<PathBuf>,
     pub errors: Vec<String>,
 }

impl IndexingStatus {
    fn calc_progress(&self) -> IndexProgress {
        self.processed.div_euclid(self.processed) as f64
    }
}

 #[derive(Debug, Clone, PartialEq)]
 pub enum IndexStatus {
     Idle,
     Running,
     Paused,
     Completed,
     Failed(String),
     Cancelled,
 }

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum IndexerCommand {
    Pause,
    Resume,
    Cancel,
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

    pub async fn compute_batch(&self, snippets: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError> {
        snippets
            .into_iter()
            .map(|_| vec![0.0; self.dummy_dimensions])
            .map(Ok)
            .collect()
    }
}

impl EmbeddingProcessor {
    pub fn new(local_backend: Option<LocalModelBackend>) -> Self {
        Self { local_backend }
    }

    pub async fn generate_embeddings(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        match &self.local_backend {
            Some(backend) => backend.compute_batch(snippets).await,
            None => Err(EmbedError::Embedding(
                "No embedding backend configured".to_string(),
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

#[derive(Debug)]
pub struct IndexerTask {
    pub db: Arc<Database>,
    pub io: IoManagerHandle,
    pub embedding_processor: EmbeddingProcessor,
    pub cancellation_token: CancellationToken,
    pub batch_size: usize,
}

impl IndexerTask {
    // async fn run(&self) -> Result<(), ploke_error::Error> {
    //     while let Some(batch) = self.next_batch().await? {
    //         process_batch(
    //             &self.db,
    //             &self.io,
    //             &self.embedding_processor,
    //             batch,
    //             |current, total| tracing::info!("Indexed {current}/{total}"),
    //         )
    //         .await?;
    //     }
    //     Ok(())
    // }

    pub async fn run(
        &self,
        progress_tx: broadcast::Sender<IndexingStatus>,
        mut control_rx: mpsc::Receiver<IndexerCommand>
    ) -> Result<(), EmbedError> {
        let total = self.db.count_pending_embeddings()?;
        let mut state = IndexingStatus {
            status: IndexStatus::Running,
            processed: 0,
            total,
            current_file: None,
            errors: Vec::new(),
        };

        progress_tx.send(state.clone())?;
        
        while let Some(batch) = self.next_batch().await? {
            // Check for control commands
            if let Ok(cmd) = control_rx.try_recv() {
                match cmd {
                    IndexerCommand::Pause => state.status = IndexStatus::Paused,
                    IndexerCommand::Resume => state.status = IndexStatus::Running,
                    IndexerCommand::Cancel => {
                        state.status = IndexStatus::Cancelled;
                        break;
                    }
                }
                progress_tx.send(state.clone())?;
            }
            
            if state.status != IndexStatus::Running {
                continue;
            }
            
            state.current_file = batch.first().map(|n| n.path.clone());
            progress_tx.send(state.clone())?;
            
            let batch_len = batch.len();
            match process_batch(
                &self.db,
                &self.io,
                &self.embedding_processor,
                batch,
                |current, total| tracing::info!("Indexed {current}/{total}"),
            ).await {
                Ok(_) => state.processed += batch_len,
                Err(e) => state.errors.push(e.to_string()),
            }
            
            progress_tx.send(state.clone())?;
        }
        
        state.status = if state.processed >= state.total {
            IndexStatus::Completed
        } else {
            IndexStatus::Cancelled
        };
        progress_tx.send(state)?;
        Ok(())
    }
    async fn next_batch(&self) -> Result<Option<Vec<EmbeddingNode>>, EmbedError> {
        static LAST_ID: tokio::sync::Mutex<Option<uuid::Uuid>> =
            tokio::sync::Mutex::const_new(None);

        let mut last_id_guard = LAST_ID.lock().await;
        let last_id = last_id_guard.take();

        let batch = self
            .db
            .get_nodes_for_embedding(self.batch_size, last_id)
            .map_err(EmbedError::PlokeCore)?;

        *last_id_guard = batch.last().map(|node| node.id);

        if self.cancellation_token.is_cancelled() {
            return Err(EmbedError::Cancelled("Processing cancelled".into()));
        }

        match batch.is_empty() {
            true => Ok(None),
            false => Ok(Some(batch)),
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

    let snippet_results =
        io_manager
            .get_snippets_batch(requests)
            .await
            .map_err(|arg0: ploke_io::RecvError| {
                EmbedError::SnippetFetch(ploke_io::IoError::Recv(arg0))
            })?;

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

    db.update_embeddings_batch(updates).await?;

    report_progress(total_nodes, total_nodes);
    Ok(())
}

use tokio::sync::{broadcast, mpsc};

