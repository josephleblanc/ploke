use crate::local::LocalEmbedder;
use crate::providers::hugging_face::HuggingFaceBackend;
use crate::providers::openai::OpenAIBackend;
use crate::{config::CozoConfig, error::truncate_string};
use ploke_core::EmbeddingData;
use ploke_db::{Database, NodeType, TypedEmbedData};
use ploke_io::IoManagerHandle;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc};
use tokio::time;
use tracing::{info_span, instrument};
use uuid::Uuid;

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

fn count_tyemb(tyemb_vec: &[TypedEmbedData]) -> usize {
    tyemb_vec.iter().fold(0, |acc, i| acc + i.v.len())
}

impl EmbeddingProcessor {
    pub fn new(source: EmbeddingSource) -> Self {
        Self { source }
    }

    pub async fn generate_embeddings(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        tracing::trace!("Starting generate_embeddings with EmbeddingSource dimensions: {:#?} with {} snippets\nfirst snippet: {:?}\nlast snippet: {:?}",
            self.dimensions(),
            snippets.len(),
            snippets.first(),
            snippets.last(),
        );
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
    pub recent_processed: usize,
    pub num_not_proc: usize,
    pub current_file: Option<PathBuf>,
    pub errors: Vec<String>,
}

impl IndexingStatus {
    pub fn calc_progress(&self) -> IndexProgress {
        if self.num_not_proc == 0 {
            0.0
        } else {
            self.recent_processed as f64 / self.num_not_proc as f64
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
    pub cursors: Mutex<HashMap<NodeType, Uuid>>,
    pub total_processed: AtomicUsize,
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
            cursors: Mutex::new(HashMap::new()),
            total_processed: AtomicUsize::new(0),
        }
    }

    #[instrument(
        name = "Indexer::run",
        skip(self, progress_tx, control_rx),
        fields(num_not_proc, recent_processed, status="Running")  // Track key state
    )]
    pub async fn run(
        &self,
        progress_tx: broadcast::Sender<IndexingStatus>,
        mut control_rx: mpsc::Receiver<IndexerCommand>,
    ) -> Result<(), EmbedError> {
        let num_not_proc = self.db.count_unembedded_nonfiles()?;
        tracing::info!("Starting indexing with {} unembedded nodes", num_not_proc);
        let mut state = IndexingStatus {
            status: IndexStatus::Running,
            recent_processed: 0,
            num_not_proc,
            current_file: None,
            errors: Vec::new(),
        };
        progress_tx.send(state.clone())?;

        while let Some(batch) = self.next_batch(num_not_proc).await? {
            time::sleep(Duration::from_millis(500)).await;
            // state.recent_processed = 0;
            let node_count = batch.iter().fold(0, |acc, b| acc + b.v.len());

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

            state.current_file = batch
                .iter()
                .filter_map(|v| v.first().map(|i| i.clone().file_path))
                .next();

            match self
                .process_batch(batch, |current, num_not_proc| {
                    tracing::info!("Indexed {current}/{num_not_proc}")
                })
                .await
            {
                Ok(_) => {
                    state.recent_processed += node_count;
                    tracing::info!(
                        "Processed batch: {}/{}",
                        state.recent_processed,
                        state.num_not_proc
                    );
                    if state.recent_processed >= num_not_proc {
                        if state.recent_processed > num_not_proc {
                            tracing::warn!(
                                "state.recent_processed > num_not_proc | there is a miscount of nodes somewhere"
                            );
                        }
                        tracing::info!(
                            "Break: {} >= {}",
                            state.recent_processed,
                            state.num_not_proc
                        );
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
            tracing::debug!(
                "Retrieved batch of {} nodes\nCurrent file: {:?}",
                node_count,
                state.current_file
            );
        }

        if state.recent_processed >= state.num_not_proc {
            tracing::info!(
                "Indexing completed: {}/{}",
                state.recent_processed,
                state.num_not_proc
            );
            state.status = IndexStatus::Completed;
            progress_tx.send(state)?;
        } else {
            tracing::warn!("Indexing cancelled");
            state.status = IndexStatus::Cancelled;
            progress_tx.send(state)?;
        };
        Ok(())
    }

    /// This function next_batch:
    /// - It locks the `last_id` (an `Arc<Mutex<Option<uuid::Uuid>>>`).
    /// - Then it calls `db.get_unembedded_node_data(batch_size, *last_id_guard)`.
    /// - It updates the `last_id` to the last node in the batch (if any).
    /// - If the cancellation token is cancelled, it returns an error.
    /// - If the batch is empty, it returns `None`; otherwise, it returns the batch.
    #[instrument(
        skip_all,
        fields(total_counted, num_not_proc, recent_processed, status="Running", batch_size)  // Track key state
    )]
    async fn next_batch(
        &self,
        num_not_proc: usize,
    ) -> Result<Option<Vec<TypedEmbedData>>, EmbedError> {
        let mut batch = Vec::new();
        let mut total_counted = 0;

        let mut rel_count = 0;
        for node_type in NodeType::primary_nodes().into_iter() {
            let fetch_size =
                std::cmp::min(self.batch_size, num_not_proc).saturating_sub(total_counted);

            if fetch_size == 0 {
                break;
            }
            let cursor = {
                let cursors_lock = self.cursors.lock().await;
                *cursors_lock
                    .get(&node_type)
                    .or(Some(&Uuid::nil()))
                    .ok_or_else(|| {
                        EmbedError::NotImplemented("could not lock cursor".to_string())
                    })?
            };

            tracing::trace!(
                "getting_rel {} with fetch_size = {fetch_size} and cursor {cursor}",
                node_type.relation_str()
            );
            let nodes = self.db.get_rel_with_cursor(node_type, fetch_size, cursor)?;

            if !nodes.is_empty() {
                tracing::info!("<<< Processing relation {rel_count} relations processed: {} | total_processed before: {:?} >>>", 
                    node_type.relation_str(), self.total_processed);
                rel_count += 1;
                let mut cursors_lock = self.cursors.lock().await;
                cursors_lock.insert(node_type, nodes.last().unwrap().id);

                let node_count = nodes.len();
                if node_count > 0 {
                    total_counted += node_count;
                    batch.push(nodes);
                }
            }
        }

        self.total_processed
            .fetch_add(total_counted, Ordering::SeqCst);
        tracing::info!(
            "<<< | total_processed after: {:?} >>>",
            self.total_processed,
        );
        if !batch.is_empty() {
            Ok(Some(batch))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip_all, fields(batch_size))]
    pub async fn process_batch(
        &self,
        nodes: Vec<TypedEmbedData>,
        report_progress: impl Fn(usize, usize) + Send + Sync,
    ) -> Result<(), EmbedError> {
        let node_count = nodes.iter().fold(0, |acc, b| acc + b.v.len());
        let mut counter = 0;
        tracing::info!(
            "process_batch with {} relations and {} nodes of EmbeddingData",
            nodes.len(),
            node_count
        );

        // TODO: Get rid of this `clone` somehow

        let (ty_vec, emb_vec): (Vec<NodeType>, Vec<EmbeddingData>) = nodes
            .clone()
            .into_iter()
            .flat_map(|n| n.v.into_iter().map(move |emb| (n.ty, emb)))
            .unzip();
        let num_to_embed = emb_vec.len();
        tracing::warn!("-- -- -- num to embed {} nodes -- -- --", num_to_embed);
        let snippet_results = self
            .io
            .get_snippets_batch(emb_vec.clone())
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

        let mut valid_nodes = Vec::new();
        let mut valid_data = Vec::new();
        let mut valid_snippets = Vec::new();

        for (ty, (emb, snippet_result)) in ty_vec
            .into_iter()
            .zip(emb_vec.into_iter().zip(snippet_results))
        {
            counter += 1;
            report_progress(counter, node_count);
            match snippet_result {
                Ok(snippet) => {
                    valid_nodes.push(ty);
                    valid_data.push(emb);
                    valid_snippets.push(snippet);
                }
                Err(e) => tracing::warn!("Snippet error: {:?}", e),
            }
        }
        tracing::info!(
            "snippet results | num_to_embed: {}, valid_nodes: {}, valid_emb_data: {}, valid_snippets: {}",
            num_to_embed,
            valid_nodes.len(),
            valid_data.len(),
            valid_snippets.len(),
        );

        if valid_snippets.is_empty() {
            tracing::error!("Empty valid snippets detected.");
            panic!("AAaaaaaaaah")
        }
        let embeddings = self
            .embedding_processor
            .generate_embeddings(valid_snippets)
            .await?;
        tracing::trace!(
            "Processed embeddings {} with dimension {:?}",
            embeddings.len(),
            embeddings.first().map(|v| v.len())
        );

        let dims = self.embedding_processor.dimensions();
        for embedding in &embeddings {
            if embedding.len() != dims {
                return Err(EmbedError::DimensionMismatch {
                    expected: dims,
                    actual: embedding.len(),
                });
            }
        }

        let updates = valid_data
            .into_iter()
            .zip(embeddings)
            .zip(valid_nodes.into_iter())
            .map(|((embs, embedding), ty)| (embs.id, embedding))
            .collect();

        tracing::info!("Updating database... ");
        self.db.update_embeddings_batch(updates).await?;
        tracing::info!("Finished processing batch");
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::{
        ops::Deref,
        sync::{atomic::AtomicUsize, Arc},
        time::Duration,
    };

    use cozo::{CallbackOp, DataValue, MemStorage, NamedRows};
    use ploke_db::{CallbackManager, Database, NodeType};
    use ploke_error::Error;
    use ploke_io::IoManagerHandle;
    use ploke_test_utils::{init_test_tracing, setup_db_full};
    use tokio::{
        sync::{
            broadcast::{self, error::TryRecvError},
            mpsc, Mutex,
        },
        time::Instant,
    };
    use tracing::Level;
    use tracing_subscriber::{filter, fmt, prelude::*, EnvFilter};

    use crate::{
        cancel_token::CancellationToken,
        error::EmbedError,
        indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexerTask, IndexingStatus},
        local::{EmbeddingConfig, EmbeddingError, LocalEmbedder},
    };

    #[tokio::test]
    async fn test_full_fixture_nodes() -> Result<(), Error> {
        test_next_batch("fixture_nodes").await
    }
    #[tokio::test]
    async fn test_full_fixture_nodes() -> Result<(), Error> {
        test_next_batch("fixture_nodes").await
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
    async fn test_next_batch(fixture: &'static str) -> Result<(), ploke_error::Error> {
        init_test_tracing(Level::TRACE);
        tracing::info!("Starting test_next_batch");

        let cozo_db = setup_db_full("fixture_nodes")?;
        let db = Arc::new(Database::new(cozo_db));
        let io = IoManagerHandle::new();

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = EmbeddingProcessor { source };

        let (cancellation_token, cancel_handle) = CancellationToken::new();
        let batch_size = 8;

        let (callback_manager, db_callbacks, unreg_codes_arc, shutdown) =
            CallbackManager::new_bounded(Arc::clone(&db), 1000)?;
        let counter = callback_manager.clone_counter();

        let idx_tag = IndexerTask::new(
            Arc::clone(&db),
            io,
            embedding_processor,
            cancellation_token,
            batch_size,
        );
        let (progress_tx, mut progress_rx) = broadcast::channel(1000);
        let (control_tx, control_rx) = mpsc::channel(4);

        let callback_handler = std::thread::spawn(move || callback_manager.run());
        let mut idx_handle =
            tokio::spawn(async move { idx_tag.run(progress_tx, control_rx).await });

        let mut received_completed = false;
        let start = Instant::now();
        let timeout = Duration::from_secs(1200); // Increased timeout

        let all_results = Arc::new(Mutex::new(Vec::new()));

        loop {
            match db_callbacks.try_recv() {
                Ok(c) => match c {
                    Ok((call, new, old)) => {
                        log_stuff(call, new.clone(), old, Arc::clone(&counter));
                        all_results.lock().await.push(new.to_owned());
                    }
                    Err(e) => {
                        tracing::debug!("[in IndexerTask.run db_callback | {e}")
                    }
                },
                Err(e) => {
                    if e.is_disconnected() {
                        tracing::debug!("[in IndexerTask.run db_callback | {e}");
                    }
                }
            };
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if start.elapsed() > timeout {
                        panic!("Test timed out without completion signal");
                    }
                }

                status = progress_rx.recv() => {
                    match status {
                        Ok(status) => {
                            match status.status {
                                IndexStatus::Failed(s)=>{tracing::debug!("Indexing failed with message: {}\nErrors: {:?}",s,status.errors);panic!("Indexing failed with message: {}\nErrors: {:?}",s,status.errors);tracing::debug!("Non-completed Progress: {:?}",status);}
                                IndexStatus::Idle => {todo!()},
                                IndexStatus::Running => {},
                                IndexStatus::Paused => {todo!()},
                                IndexStatus::Completed => {
                                    tracing::debug!("Progress: {:?}", status);
                                    received_completed = true;
                                    tracing::error!("Indexer callback_handler did not finish.");
                                    if callback_handler.is_finished() {
                                        tracing::info!("Callback Handler is Finished: {:?}", callback_handler);
                                        let result = callback_handler.join().map_err(|e| EmbedError::JoinFailed("x".to_string()))?;
                                        result?;
                                        shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");
                                        break;
                                    }
                                },
                                IndexStatus::Cancelled => {
                                    tracing::debug!("Cancelled Task | Progress: {:?}", status);
                                    break;
                                },
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Received Error: {:?}", e);
                        }, // Channel closed
                    }
                }

                res = &mut idx_handle => {
                    let task_result = res.expect("Task panicked");
                    task_result?; // Propagate any errors
                    break;
                }


            }
        }
        if idx_handle.is_finished() {
            tracing::info!("Indexer Handle is Finished: {:?}", idx_handle);
        } else {
            tracing::error!("Indexer Handle did not finish.")
        }
        // if callback_handler.is_finished() {
        //     tracing::info!("Callback Handler is Finished: {:?}", callback_handler);
        // } else {
        //     tracing::error!("Indexer callback_handler did not finish.");
        // }
        let all_pending_rows = db.get_pending_test()?;
        let total_rows = all_results.lock_owned().await;
        let mut not_found = Vec::new();
        let mut found = Vec::new();
        total_rows
            .clone()
            .into_iter()
            .flat_map(|nr| nr.rows)
            .enumerate()
            .map(|(i, r)| (i, r[0].clone(), r[1].clone()))
            .for_each(|(i, idx, name)| {
                let is_found = all_pending_rows.rows.iter().any(|r| r[0] == idx);
                tracing::info!("row {: <2}: {} | {:?} {: >30}", i, is_found, name, idx);
                let node_data = (i, name, idx);
                if is_found {
                    found.push(node_data);
                } else {
                    not_found.push(node_data);
                }
            });
        for (i, name, idx) in not_found {
            tracing::info!(target: "dbg_rows", "row not found {: <2} | {:?} {: >30}", i, name, idx);
        }
        for (i, name, idx) in all_pending_rows
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r[0].clone(), r[1].clone()))
        {
            tracing::info!(target: "dbg_rows","row found {: <2} | {:?} {: >30}", i, name, idx);
        }

        assert!(
            received_completed,
            "Indexer completed without sending completion status"
        );
        Ok(())
    }
    fn log_row(r: Vec<DataValue>) {
        for (i, row) in r.iter().enumerate() {
            tracing::info!("{}: {:?}", i, row);
        }
    }
    fn log_stuff(call: CallbackOp, new: NamedRows, old: NamedRows, counter: Arc<AtomicUsize>) {
        let new_count = new.rows.len();
        let last_count = counter.fetch_add(new_count, std::sync::atomic::Ordering::Relaxed);
        let header = new.headers.clone();
        let (i, first_row) = new
            .clone()
            .into_iter()
            .enumerate()
            .next()
            .map(|(i, mut r)| {
                r.pop();
                (i, r)
            })
            .unwrap_or_else(|| (0, vec![]));
        let (j, last_row) = new
            .clone()
            .into_iter()
            .enumerate()
            .next_back()
            .map(|(j, mut r)| {
                r.pop();
                (j, r)
            })
            .unwrap_or_else(|| (0, vec![]));
        tracing::trace!(
            "| call_op: {} | new_rows: {}, old_rows: {} | {}{:=^20}\n{:?}\n{:=^20}\n{:=^10}\n{:?}\n{:=^20}\n{:=^10}\n{:?}",
            call,
            new.rows.len(),
            old.rows.len(),
            "",
            "Header",
            header.join("|"),
            "FirstRow",
            i,
            first_row,
            "LastRow number ",
            j,
            last_row,
        );
        tracing::trace!(
            "{:=^80}\n{:=^30}ATOMIC COUNTER: {:?}\n{:=^30}{:=^80}",
            "",
            "",
            counter,
            "",
            ""
        );
    }
}
