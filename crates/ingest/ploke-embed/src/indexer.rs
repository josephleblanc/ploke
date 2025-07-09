use crate::local::LocalEmbedder;
use crate::providers::hugging_face::HuggingFaceBackend;
use crate::providers::openai::OpenAIBackend;
use crate::{config::CozoConfig, error::truncate_string};
use ploke_core::EmbeddingData;
use ploke_db::Database;
use ploke_io::IoManagerHandle;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
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
    pub cursor: Arc<Mutex<usize>>,
}

impl IndexerTask {
    pub fn new(
        db: Arc<Database>,
        io: IoManagerHandle,
        embedding_processor: EmbeddingProcessor,
        cancellation_token: CancellationToken,
        batch_size: usize,
    ) -> Self {
        Self {
            db,
            io,
            embedding_processor,
            cancellation_token,
            batch_size,
            cursor: Arc::new(Mutex::new(0)),
        }
    }

    #[instrument(skip_all)]
    pub async fn run(
        &self,
        progress_tx: broadcast::Sender<IndexingStatus>,
        mut control_rx: mpsc::Receiver<IndexerCommand>,
    ) -> Result<(), EmbedError> {
        let total = self.db.count_unembedded_nonfiles()?;
        tracing::info!("Starting indexing with {} unembedded nodes", total);
        let mut state = IndexingStatus {
            status: IndexStatus::Running,
            processed: 0,
            total,
            current_file: None,
            errors: Vec::new(),
        };

        progress_tx.send(state.clone())?;

        // BUG: Infinite loop in `test_next_batch`, either
        //
        //      1. self.next_batch().await? never returns `None`
        //          - `next_batch` calls `db.get_unembedded_node_data`, meaning it relies on the
        //          `cursor` of the database return to return none.
        //      2. never `send` the finishing trigger?
        //
        //      NOTE: To be clear, this loop never exits, and the timeout never triggers.
        //      I think that suggests that the node hangs either when it is trying to send or after
        //      the loop or something. Not exactly sure.
        //      - Actually, we never get the "Break: " tracing message, so I think it hangs
        //      sometime when processed
        while let Some(batch) = self.next_batch(total).await? {
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
                .process_batch(batch, |current, total| {
                    tracing::info!("Indexed {current}/{total}")
                })
                .await
            {
                Ok(_) => {
                    state.processed += batch_len;
                    tracing::info!("Processed batch: {}/{}", state.processed, state.total);
                    if state.processed == total {
                        tracing::info!("Break: {} == {}", state.processed, state.total);
                        break;
                    }
                }
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

    /// This function next_batch:
    /// - It locks the `last_id` (an `Arc<Mutex<Option<uuid::Uuid>>>`).
    /// - Then it calls `db.get_unembedded_node_data(batch_size, *last_id_guard)`.
    /// - It updates the `last_id` to the last node in the batch (if any).
    /// - If the cancellation token is cancelled, it returns an error.
    /// - If the batch is empty, it returns `None`; otherwise, it returns the batch.
    #[instrument(skip_all)]
    async fn next_batch(&self, total: usize) -> Result<Option<Vec<EmbeddingData>>, EmbedError> {
        let mut last_id_guard = self.cursor.lock().await;
        let batch_min = self.batch_size.min(total);
        let batch = self
            .db
            .get_unembedded_node_data(batch_min, *last_id_guard)
            .map_err(EmbedError::PlokeCore)?;
        tracing::debug!("old batch id: {:?}", last_id_guard);

        *last_id_guard += batch.len();

        tracing::debug!("new batch id: {:?}", last_id_guard);

        if self.cancellation_token.is_cancelled() {
            return Err(EmbedError::Cancelled("Processing cancelled".into()));
        }

        tracing::debug!(
            "batch: {} | batch_min: {batch_min} | last_id_guard: {last_id_guard}",
            batch.len()
        );
        match batch.is_empty() {
            true => Ok(None),
            false => Ok(Some(batch)),
        }
    }

    #[instrument(skip_all, fields(batch_size = nodes.len()))]
    pub async fn process_batch(
        &self,
        nodes: Vec<EmbeddingData>,
        report_progress: impl Fn(usize, usize) + Send + Sync,
    ) -> Result<(), EmbedError> {
        let ctx_span = info_span!("process_batch");
        let _guard = ctx_span.enter();

        let total_nodes = nodes.len();
        tracing::info!("Processing batch of {} nodes", total_nodes);
        // TODO: Get rid of this `clone` somehow
        let snippet_results = self
            .io
            .get_snippets_batch(nodes.clone())
            .await
            .inspect_err(|e| {
                tracing::error!(
                    "Error processing batch, with start node {:#?}\nend node {:#?}",
                    nodes.first(),
                    nodes.last()
                );
            })
            .map_err(|arg0: ploke_io::RecvError| {
                EmbedError::SnippetFetch(ploke_io::IoError::Recv(arg0))
            })?;

        let mut valid_snippets = Vec::new();
        let mut valid_nodes = Vec::new();
        let mut valid_indices = Vec::new();

        for (i, (node, snippet_result)) in nodes.into_iter().zip(snippet_results).enumerate() {
            report_progress(i + 1, total_nodes);
            match snippet_result {
                Ok(snippet) => {
                    valid_snippets.push(snippet);
                    valid_nodes.push(node);
                    valid_indices.push(i + 1);
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
    use tokio::{
        sync::{
            broadcast::{self, error::TryRecvError},
            mpsc,
        },
        time::Instant,
    };
    use tracing_subscriber::{filter, fmt, prelude::*, EnvFilter};

    use crate::{
        cancel_token::CancellationToken,
        indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexerTask},
        local::{EmbeddingConfig, EmbeddingError, LocalEmbedder},
    };

    fn init_test_tracing() {
        let filter = filter::Targets::new()
            .with_target("cozo", tracing::Level::WARN)
            .with_target("", tracing::Level::DEBUG);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr) // Write to stderr
                    .with_ansi(true) // Disable colors for cleaner output
                    .pretty()
                    .without_time(), // Optional: remove timestamps
            )
            .with(filter)
            .init();
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

    // The test does the following:
    //  1. Initializes tracing for the test.
    //  2. Sets up a test database using `setup_db_full("fixture_nodes")`.
    //  3. Creates an `IoManagerHandle`.
    //  4. Creates a `LocalEmbedder` for embeddings.
    //  5. Creates an `EmbeddingProcessor` with the `LocalEmbedder`.
    //  6. Creates a `CancellationToken` and its handle.
    //  7. Creates an `IndexerTask` with the database, I/O handle, embedding processor, cancellation token, and a batch size of 100.
    //  8. Creates a broadcast channel for progress and an mpsc channel for control commands.
    //  9. Spawns the `IndexerTask::run` in a separate tokio task.
    //  10. Then it waits for the indexing to complete by listening to progress updates and the task handle.
    //  The issue: the test times out without receiving a completion signal.
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

        let idx_tag = IndexerTask::new(db, io, embedding_processor, cancellation_token, batch_size);
        let (progress_tx, mut progress_rx) = broadcast::channel(1000);
        let (control_tx, control_rx) = mpsc::channel(4);

        let mut handle = tokio::spawn(async move { idx_tag.run(progress_tx, control_rx).await });

        let mut received_completed = false;
        let start = Instant::now();
        let timeout = Duration::from_secs(10); // Increased timeout

        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if start.elapsed() > timeout {
                        panic!("Test timed out without completion signal");
                    }
                }

                status = progress_rx.recv() => {
                    match status {
                        Ok(status) if status.status == IndexStatus::Completed => {
                            tracing::debug!("Progress: {:?}", status);
                            received_completed = true;
                            break;
                        }
                        Ok(status) => {
                            tracing::debug!("Progress: {:?}", status);
                            if matches!(status.status, IndexStatus::Failed(_)) {
                                panic!("Indexing failed: {:?}", status.errors);
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Received Error: {:?}", e);
                            break;
                        }, // Channel closed
                    }
                }

                res = &mut handle => {
                    let task_result = res.expect("Task panicked");
                    task_result?; // Propagate any errors
                    break;
                }

            }
        }

        // let res = handle.await.map_err(EmbeddingError::from)?;
        // res?;

        assert!(
            received_completed,
            "Indexer completed without sending completion status"
        );
        Ok(())
    }
}
