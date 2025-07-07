use crate::local::LocalEmbedder;
use crate::providers::hugging_face::HuggingFaceBackend;
use crate::providers::openai::OpenAIBackend;
use crate::{config::CozoConfig, error::truncate_string};
use ploke_core::EmbeddingData;
use ploke_db::Database;
use ploke_io::IoManagerHandle;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{info_span, instrument};

use crate::{cancel_token::CancellationToken, error::EmbedError};

#[derive(Debug)]
pub struct EmbeddingProcessor {
    source: EmbeddingSource,
}

#[derive(Debug)]
pub enum EmbeddingSource {
    Local(LocalEmbedder),
    HuggingFace(HuggingFaceBackend),
    OpenAI(OpenAIBackend),
    Cozo(CozoBackend),
}

// impl Default for EmbeddingProcessor {
//     fn default() -> Self {
//         let source = EmbeddingSource::Local();
//         Self { source }
//     }
// }

impl EmbeddingProcessor {
    pub fn new(source: EmbeddingSource) -> Self {
        Self { source }
    }

    pub async fn generate_embeddings(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        match &self.source {
            EmbeddingSource::Local(backend) => {
                let text_slices: Vec<&str> = snippets.iter().map(|s| s.as_str()).collect();
                Ok(backend.embed_batch(&text_slices)?)
            }
            EmbeddingSource::HuggingFace(backend) => backend.compute_batch(snippets).await,
            EmbeddingSource::OpenAI(backend) => backend.compute_batch(snippets).await,
            EmbeddingSource::Cozo(backend) => backend.compute_batch(snippets).await,
        }
    }

    pub fn dimensions(&self) -> usize {
        match &self.source {
            EmbeddingSource::Local(backend) => backend.dimensions(),
            EmbeddingSource::HuggingFace(backend) => backend.dimensions,
            EmbeddingSource::OpenAI(backend) => backend.dimensions,
            EmbeddingSource::Cozo(backend) => backend.dimensions,
        }
    }
}

// Cozo placeholder backend
#[derive(Debug)]
pub struct CozoBackend {
    endpoint: String,
    dimensions: usize,
}

impl CozoBackend {
    pub fn new(_config: &CozoConfig) -> Self {
        Self {
            endpoint: "https://embedding.cozo.com".to_string(),
            dimensions: 512, // example dimensions
        }
    }

    pub async fn compute_batch(&self, _snippets: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError> {
        Err(EmbedError::NotImplemented(
            "Cozo embeddings not implemented".to_string(),
        ))
    }
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
    pub fn calc_progress(&self) -> IndexProgress {
        if self.total == 0 {
            0.0
        } else {
            self.processed as f64 / self.total as f64
        }
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

#[derive(Debug)]
pub struct IndexerTask {
    pub db: Arc<Database>,
    pub io: IoManagerHandle,
    pub embedding_processor: EmbeddingProcessor,
    pub cancellation_token: CancellationToken,
    pub batch_size: usize,
}

use tracing::instrument;

#[instrument(skip_all)]
impl IndexerTask {
    #[instrument(skip_all)]
    pub async fn run(
        &self,
        progress_tx: broadcast::Sender<IndexingStatus>,
        mut control_rx: mpsc::Receiver<IndexerCommand>,
    ) -> Result<(), EmbedError> {
        let total = self.db.count_pending_embeddings()?;
        tracing::info!("Starting indexing with {} unembedded nodes", total);
        let mut state = IndexingStatus {
            status: IndexStatus::Running,
            processed: 0,
            total,
            current_file: None,
            errors: Vec::new(),
        };

        progress_tx.send(state.clone())?;

        // TODO: Use `tokio::select!` here?
        //  see ploke-embed/docs/better_index.md
        while let Some(batch) = self.next_batch().await? {
            tracing::debug!("Retrieved batch of {} nodes", batch.len());
            
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
                // Skip batch processing
                continue;
            }

            state.current_file = batch.first().map(|n| n.file_path.clone());
            progress_tx.send(state.clone())?;

            let batch_len = batch.len();
            match self
                .process_batch(
                    // &self.db,
                    // &self.io,
                    // &self.embedding_processor,
                    batch,
                    |current, total| tracing::info!("Indexed {current}/{total}"),
                )
                .await
            {
                Ok(_) => {
                    state.processed += batch_len;
                    tracing::info!("Processed batch: {}/{}", state.processed, state.total);
                },
                Err(e) => {
                    let error_str = match &e {
                        EmbedError::HttpError { status, body, url } => format!(
                            "HTTP {} at {}: {}",
                            status,
                            truncate_string(url, 40),
                            truncate_string(body, 80)
                        ),
                        _ => e.to_string(),
                    };
                    state.errors.push(error_str);

                    // Log with full context for diagnostics
                    tracing::error!("Batch process failed: {e:?}");
                }
            }

            progress_tx.send(state.clone())?;
        }

        state.status = if state.processed >= state.total {
            tracing::info!("Indexing completed");
            IndexStatus::Completed
        } else {
            tracing::warn!("Indexing cancelled");
            IndexStatus::Cancelled
        };
        progress_tx.send(state)?;
        Ok(())
    }

    #[instrument(skip_all)]
    async fn next_batch(&self) -> Result<Option<Vec<EmbeddingData>>, EmbedError> {
        static LAST_ID: tokio::sync::Mutex<Option<uuid::Uuid>> =
            tokio::sync::Mutex::const_new(None);

        let mut last_id_guard = LAST_ID.lock().await;
        let last_id = last_id_guard.take();
        tracing::debug!("Last ID: {:?}", last_id);

        let batch = self
            .db
            .get_unembedded_node_data(self.batch_size, last_id)
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

    #[instrument(skip_all, fields(batch_size = nodes.len()))]
    pub async fn process_batch(
        &self,
        // db: &Database,
        // io_manager: &IoManagerHandle,
        // embedding_processor: &EmbeddingProcessor,
        nodes: Vec<EmbeddingData>,
        report_progress: impl Fn(usize, usize) + Send + Sync,
    ) -> Result<(), EmbedError> {
        let ctx_span = info_span!("process_batch");
        let _guard = ctx_span.enter();

        tracing::info!("Processing batch of {} nodes", nodes.len());
        let total_nodes = nodes.len();
        let snippet_results = self.io.get_snippets_batch(nodes.clone()).await.map_err(
            |arg0: ploke_io::RecvError| EmbedError::SnippetFetch(ploke_io::IoError::Recv(arg0)),
        )?;

        let mut valid_snippets = Vec::new();
        let mut valid_nodes = Vec::new();
        let mut valid_indices = Vec::new();

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

        let embeddings = self
            .embedding_processor
            .generate_embeddings(valid_snippets)
            .await?;

        let dims = self.embedding_processor.dimensions();
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

        self.db.update_embeddings_batch(updates).await?;

        report_progress(total_nodes, total_nodes);
        tracing::info!("Finished processing batch");
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::{sync::Arc, time::Duration};

    use cozo::MemStorage;
    use ploke_db::Database;
    use ploke_io::IoManagerHandle;
    use ploke_test_utils::setup_db_full;
    use tokio::{sync::{
        broadcast::{self, error::TryRecvError},
        mpsc,
    }, time::Instant};
    use tracing_subscriber::{fmt, EnvFilter};

    use crate::{
        cancel_token::CancellationToken,
        indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexerTask},
        local::{EmbeddingConfig, EmbeddingError, LocalEmbedder},
    };

    fn init_test_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
    }

    async fn setup_local_model_config() -> Result<LocalEmbedder, ploke_error::Error> {
        let cozo_db = setup_db_full("fixture_nodes")?;
        let db = Arc::new(Database::new(cozo_db));
        let io = IoManagerHandle::new();

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        Ok(model)
    }

    async fn setup_local_model_embedding_processor(
    ) -> Result<EmbeddingProcessor, ploke_error::Error> {
        let model = setup_local_model_config().await?;
        let source = EmbeddingSource::Local(model);
        Ok(EmbeddingProcessor { source })
    }

    #[tokio::test]
    async fn test_local_model_config() -> Result<(), ploke_error::Error> {
        setup_local_model_config().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_local_model_embedding_processor() -> Result<(), ploke_error::Error> {
        setup_local_model_embedding_processor().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_next_batch() -> Result<(), ploke_error::Error> {
        init_test_tracing();
        tracing::info!("Starting test_next_batch");

        let cozo_db = setup_db_full("fixture_nodes")?;
        let db = Arc::new(Database::new(cozo_db));
        let io = IoManagerHandle::new();

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = EmbeddingProcessor { source };

        let (cancellation_token, cancel_handle) = CancellationToken::new();
        let batch_size = 100;

        let idx_tag = IndexerTask {
            db,
            io,
            embedding_processor,
            cancellation_token,
            batch_size,
        };
        let (progress_tx, mut progress_rx) = broadcast::channel(1000);
        let (control_tx, control_rx) = mpsc::channel(4);

        let handle = tokio::spawn(async move { idx_tag.run(progress_tx, control_rx).await });
        let start_time = Instant::now();
        let timeout = Duration::from_secs(120);
        let mut received_completed = false;

        while Instant::now().duration_since(start_time) < timeout { 
            match progress_rx.try_recv() {
                Ok(status) if status.status == IndexStatus::Completed => {
                    received_completed = true;
                    tracing::debug!("update: {:?} and received_completed = {}", status, received_completed);
                    break;
                },
                Err(TryRecvError::Closed | TryRecvError::Empty) => {
                    tracing::debug!("update | closed");
                }
                Err(TryRecvError::Lagged(backlog)) => {
                    tracing::debug!("update | lagged: {:?}", backlog);
                }
                _ => tokio::time::sleep(Duration::from_millis(10)).await
            }
        }
        assert!(received_completed, "Test timed out without completion signal");
        let res = handle.await.map_err(EmbeddingError::from)?;
        res?;

        Ok(())
    }
}
