#![allow(unused_mut)]
use crate::local::{EmbeddingConfig, LocalEmbedder};
use crate::providers::hugging_face::HuggingFaceBackend;
use crate::providers::openai::OpenAIBackend;
use crate::{config::CozoConfig, error::truncate_string};
use cozo::{CallbackOp, DataValue, NamedRows};
use ploke_core::EmbeddingData;
use ploke_db::{CallbackManager, Database, NodeType, TypedEmbedData};
use ploke_io::IoManagerHandle;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{self, Instant};
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
                Ok(backend.embed_batch(&text_slices).inspect(|v| {
                    tracing::trace!("OK Returning from embed_batch with vec(s): {:?}", v);
                }).inspect_err(|e| {
                    tracing::trace!("Error Returning from embed_batch with error: {:?}", e.to_string());
                    })?)
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
            0.1
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
    pub embedding_processor: Arc< EmbeddingProcessor >,
    pub cancellation_token: CancellationToken,
    pub batch_size: usize,
    pub cursors: Mutex<HashMap<NodeType, Uuid>>,
    pub total_processed: AtomicUsize,
}

impl IndexerTask {
    pub fn new(
        db: Arc<Database>,
        io: IoManagerHandle,
        embedding_processor: Arc< EmbeddingProcessor >,
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

    #[allow(unused_variables)]
    pub async fn index_workspace_test(
        task: Arc<Self>,
        workspace_dir: String,
        // db_callback: crossbeam_channel::Receiver<Result<(CallbackOp, NamedRows, NamedRows), EmbedError>>
        progress_tx: Arc<broadcast::Sender<IndexingStatus>>,
        mut progress_rx: broadcast::Receiver<IndexingStatus>,
        control_rx: mpsc::Receiver<IndexerCommand>,
    ) -> Result<(), ploke_error::Error> {
        time::sleep(Duration::from_secs(2)).await;
        Err(ploke_error::Error::Internal(
            ploke_error::InternalError::NotImplemented("Error forwarding works".to_string()),
        ))
    }

    // TODO: Consider returning a reset version of Self instead of consuming self here.
    // In the same vein consider not dropping the callback item.
    #[allow(unused_mut)]
    pub async fn index_workspace(
        task: Arc<Self>,
        workspace_dir: String,
        // db_callback: crossbeam_channel::Receiver<Result<(CallbackOp, NamedRows, NamedRows), EmbedError>>
        progress_tx: Arc<broadcast::Sender<IndexingStatus>>,
        mut progress_rx: broadcast::Receiver<IndexingStatus>,
        control_rx: mpsc::Receiver<IndexerCommand>,
        callback_handler: std::thread::JoinHandle<Result<(), ploke_db::DbError>>,
        db_callbacks: crossbeam_channel::Receiver<Result<(CallbackOp, NamedRows, NamedRows), ploke_db::DbError>>,
        counter: Arc<AtomicUsize>,
        shutdown: crossbeam_channel::Sender<()>,
    ) -> Result<(), ploke_error::Error> {
        // let (cancellation_token, cancel_handle) = CancellationToken::new();
        tracing::info!("Starting index_workspace: {}", &workspace_dir);
        let db_clone = Arc::clone(&task.db);
        let total_count_not_indexed = db_clone.count_unembedded_nonfiles()?;

        let mut idx_handle = tokio::spawn(async move { task.run(progress_tx, control_rx).await });

        let received_completed = AtomicBool::new(false);
        let start = Instant::now();
        let timeout = Duration::from_secs(1200); // Increased timeout

        let callback_closed = AtomicBool::new(false);
        let all_results = Arc::new(Mutex::new(Vec::new()));

        let mut ticker = time::interval(Duration::from_secs(1));
        ticker.tick().await;
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
                        break;
                    }
                }
            };
            tokio::select! {
                biased;

                status = progress_rx.recv() => {
                    match status {
                        Ok(status) => {
                            match status.status {
                                IndexStatus::Failed(s)=>{
                                    tracing::debug!("Indexing failed with message: {}\nErrors: {:?}",
                                        s,status.errors);
                                        panic!("Indexing failed with message: {}\nErrors: {:?}",s,status.errors);
                                }
                                IndexStatus::Idle => {todo!()},
                                IndexStatus::Running => {},
                                IndexStatus::Paused => {todo!()},
                                IndexStatus::Completed => {
                                    tracing::debug!("Progress: {:?}", status);
                                    received_completed.store(true, std::sync::atomic::Ordering::SeqCst);
                                    if callback_handler.is_finished() {
                                        callback_closed.store(true, std::sync::atomic::Ordering::Relaxed);
                                        tracing::info!("Callback Handler is Finished: {:?}", callback_handler);
                                        callback_handler.join().expect("Callback errror - not finished")?;
                                        break;
                                    } else {
                                        tracing::warn!("Sending shutdown signal to CallbackManager.");
                                        shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");
                                        // break;
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
                    if callback_handler.is_finished() {
                        callback_closed.store(true, std::sync::atomic::Ordering::Relaxed);
                        tracing::info!("Callback Handler is Finished: {:?}", callback_handler);
                        callback_handler.join().expect("Callback errror - not finished")?;
                        break;
                    } else {
                        tracing::warn!("Sending shutdown signal to CallbackManager.");
                        shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");
                        // break;
                    }
                    let task_result = res.expect("Task panicked");
                    task_result?; // Propagate any errors
                    break;
                }
                // res = &mut idx_handle => {
                //     let task_result = res.expect("Task panicked");
                //     let _ = task_result.as_ref().map_err(|e| tracing::debug!("idx_handle ended with error: {}", e.to_string())); // Propagate any errors
                //     break;
                // }

                x = ticker.tick() => {
                    tracing::info!("Ticking with time: {:.2}", x.duration_since(start).as_secs_f32());
                }

                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if start.elapsed() > timeout {
                        panic!("Test timed out without completion signal");
                    }
                }

            }
        }
        if idx_handle.is_finished() {
            tracing::info!("Indexer Handle is Finished: {:?}", idx_handle);
            // inner result
        } else {
            tracing::error!("Indexer Handle did not finish.")
        }
        if !callback_closed.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("CallbackManager not closed?");
        }
        let all_pending_rows = db_clone.get_pending_test()?;
        let total_non_indexed_rows = all_results.lock_owned().await;
        let mut indexed = Vec::new();
        let mut not_indexed = Vec::new();
        total_non_indexed_rows
            .clone()
            .into_iter()
            .flat_map(|nr| nr.rows)
            .enumerate()
            .map(|(i, r)| (i, r[0].clone(), r[1].clone()))
            .for_each(|(i, idx, name)| {
                let is_not_indexed = all_pending_rows.rows.iter().any(|r| r[0] == idx);
                tracing::trace!("row {: <2}: {} | {:?} {: >30}", i, is_not_indexed, name, idx);
                let node_data = (i, name, idx);
                if is_not_indexed {
                    not_indexed.push(node_data);
                } else {
                    indexed.push(node_data);
                }
            });
        for (i, name, idx) in indexed {
            tracing::trace!(target: "dbg_rows", "row indexed {: <2} | {:?} {: >30}", i, name, idx);
        }
        for (i, name, idx) in all_pending_rows
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r[0].clone(), r[1].clone()))
        {
            tracing::trace!(target: "dbg_rows","row not_indexed {: <2} | {:?} {: >30}", i, name, idx);
        }
        tracing::info!("Ending index_workspace: {workspace_dir}");
        let inner = counter.load(std::sync::atomic::Ordering::SeqCst);
        tracing::info!("Ending index_workspace: {workspace_dir}: total count {inner}, counter {total_count_not_indexed} | {inner}/{total_count_not_indexed}");

        // tracing::info!(
        //     "Indexer completed? {}",
        //     received_completed.load(std::sync::atomic::Ordering::SeqCst),
        // );
        Ok(())
    }

    #[instrument(
        name = "Indexer::run",
        skip(self, progress_tx, control_rx),
        fields(num_not_proc, recent_processed, status="Running")  // Track key state
    )]
    pub async fn run(
        &self,
        progress_tx: Arc<broadcast::Sender<IndexingStatus>>,
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
            // time::sleep(Duration::from_millis(500)).await;
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

        let total_processed = self.total_processed.load(Ordering::SeqCst);
        if total_processed >= state.num_not_proc {
            tracing::info!(
                "Indexing completed: {}/{} - recently_processed: {}",
                total_processed,
                state.num_not_proc,
                state.recent_processed,
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
            // panic!("AAaaaaaaaah")
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

#[cfg(test)]
mod tests {

    use std::{
        collections::BTreeMap,
        ops::Deref,
        sync::{
            atomic::{AtomicBool, AtomicUsize},
            Arc,
        },
        time::Duration,
    };

    use cozo::{CallbackOp, DataValue, MemStorage, NamedRows};
    use itertools::Itertools;
    use ploke_db::{hnsw_all_types, CallbackManager, Database, DbError, NodeType};
    use ploke_error::Error;
    use ploke_io::IoManagerHandle;
    use ploke_test_utils::{init_test_tracing, setup_db_full};
    use tokio::{
        sync::{
            broadcast::{self, error::TryRecvError},
            mpsc, Mutex,
        },
        time::{self, Instant},
    };
    use tracing::Level;
    use tracing_subscriber::{filter, fmt, prelude::*, EnvFilter};

    use crate::{
        cancel_token::CancellationToken,
        error::EmbedError,
        indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexerTask, IndexingStatus},
        local::{EmbeddingConfig, EmbeddingError, LocalEmbedder},
    };

    pub fn init_test_tracing_temporary(level: tracing::Level) {
        let filter = filter::Targets::new()
            .with_target("cozo", tracing::Level::ERROR)
            .with_target("ploke", level)
            .with_target("ploke-db", level)
            .with_target("ploke-embed", level)
            .with_target("ploke-io", level)
            .with_target("ploke-transform", level)
            .with_target("transform_functions", level);

        let layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_file(true)
            .with_line_number(true)
            .with_target(false) // Show module path
            .with_level(true) // Show log level
            .without_time() // Remove timestamps
            .pretty(); // Use compact format
        tracing_subscriber::registry()
            .with(layer)
            .with(filter)
            .init();
    }

    #[tokio::test]
    // NOTE: passing
    async fn test_next_batch_only() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_nodes").await
    }
    #[tokio::test]
    #[ignore = "requires further improvement of syn_parser"]
    // FIX: failing on step:
    // INFO  Parse: build module tree
    //    at crates/test-utils/src/lib.rs:65
    async fn test_batch_file_dir_detection() -> Result<(), Error> {
        init_test_tracing(Level::TRACE);
        test_next_batch("file_dir_detection").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_attributes() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_attributes").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_cyclic_types() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_cyclic_types").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_edge_cases() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_edge_cases").await
    }
    #[tokio::test]
    // INFO: passing
    async fn test_batch_generics() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_generics").await
    }
    #[tokio::test]
    // INFO: passing
    async fn test_batch_macros() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_macros").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_path_resolution() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_path_resolution").await
    }
    #[tokio::test]
    #[ignore = "requires further improvement of syn_parser"]
    // WARN: failing (dependent upon other improvements in cfg?)
    async fn test_batch_spp_edge_cases_cfg() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_spp_edge_cases").await
    }
    #[tokio::test]
    #[ignore = "requires further improvement of syn_parser"]
    // WARN: failing (dependent upon other improvements)
    async fn test_batch_spp_edge_cases_no_cfg() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_spp_edge_cases_no_cfg").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_tracking_hash() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_tracking_hash").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_types() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch("fixture_types").await
    }
    async fn test_full() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        let start = Instant::now();
        let mut results = Vec::new();
        results.push(test_next_batch("fixture_nodes").await);
        results.push(test_next_batch("duplicate_name_fixture_1").await);
        results.push(test_next_batch("duplicate_name_fixture_2").await);
        results.push(test_next_batch("example_crate").await);
        // results.push(test_next_batch("file_dir_detection").await);
        results.push(test_next_batch("fixture_attributes").await);
        results.push(test_next_batch("fixture_conflation").await);
        results.push(test_next_batch("fixture_cyclic_types").await);
        results.push(test_next_batch("fixture_edge_cases").await);
        results.push(test_next_batch("fixture_generics").await);
        results.push(test_next_batch("fixture_macros").await);
        results.push(test_next_batch("fixture_path_resolution").await);
        // results.push(test_next_batch("fixture_spp_edge_cases").await);
        // results.push(test_next_batch("fixture_spp_edge_cases_no_cfg").await);
        results.push(test_next_batch("fixture_tracking_hash").await);
        results.push(test_next_batch("fixture_types").await);

        let time_taken = Instant::now().duration_since(start);
        for (i, res) in results.clone().into_iter().enumerate() {
            match res {
                Ok(_) => eprintln!("{: ^2} + Succeed!", i),
                Err(e) => {
                    eprintln!("{: <2} - Error!\n{}", i, e);
                }
            }
        }
        eprintln!("Total Time Taken: {}", time_taken.as_secs());
        for res in results {
            res?;
        }
        Ok(())
    }

    async fn setup_local_model_config(
        fixture: &'static str,
    ) -> Result<LocalEmbedder, ploke_error::Error> {
        let cozo_db = setup_db_full(fixture)?;
        let db = Arc::new(Database::new(cozo_db));
        let io = IoManagerHandle::new();

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        Ok(model)
    }

    async fn setup_local_model_embedding_processor(
    ) -> Result<EmbeddingProcessor, ploke_error::Error> {
        let model = setup_local_model_config("fixture_nodes").await?;
        let source = EmbeddingSource::Local(model);
        Ok(EmbeddingProcessor { source })
    }

    #[tokio::test]
    async fn test_local_model_config() -> Result<(), ploke_error::Error> {
        setup_local_model_config("fixture_nodes").await?;
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
    // async fn test_next_batch(fixture: &'static str) -> Result<(), ploke_error::Error> {
    // init_test_tracing(Level::INFO);
    async fn test_next_batch(fixture: &'static str) -> Result<(), ploke_error::Error> {
        tracing::info!("Starting test_next_batch: {fixture}");

        let cozo_db = setup_db_full(fixture)?;
        let db = Arc::new(Database::new(cozo_db));
        let total_count = db.count_unembedded_nonfiles()?;
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
            Arc::new( embedding_processor ),
            cancellation_token,
            batch_size,
        );
        let (progress_tx_nonarc, mut progress_rx) = broadcast::channel(1000);
        let progress_tx_arc = Arc::new(progress_tx_nonarc);
        let (control_tx, control_rx) = mpsc::channel(4);

        let callback_handler = std::thread::spawn(move || callback_manager.run());
        let mut idx_handle =
            tokio::spawn(async move { idx_tag.run(progress_tx_arc, control_rx).await });

        let received_completed = AtomicBool::new(false);
        let callback_closed = AtomicBool::new(false);
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
                        break;
                    }
                }
            };
            tokio::select! {
                biased;

                status = progress_rx.recv() => {
                    match status {
                        Ok(status) => {
                            match status.status {
                                IndexStatus::Failed(s)=>{
                                    tracing::debug!("Indexing failed with message: {}\nErrors: {:?}",
                                        s,status.errors);
                                    panic!("Indexing failed with message: {}\nErrors: {:?}",s,status.errors);
                                }
                                IndexStatus::Idle => {todo!()},
                                IndexStatus::Running => {},
                                IndexStatus::Paused => {todo!()},
                                IndexStatus::Completed => {
                                    tracing::debug!("Progress: {:?}", status);
                                    received_completed.store(true, std::sync::atomic::Ordering::SeqCst);
                                    if callback_handler.is_finished() {
                                        callback_closed.store(true, std::sync::atomic::Ordering::Relaxed);
                                        tracing::info!("Callback Handler is Finished: {:?}", callback_handler);
                                        callback_handler.join().expect("Callback errror - not finished")?;
                                        break;
                                    } else {
                                        tracing::warn!("Sending shutdown signal to CallbackManager.");
                                        shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");
                                        // break;
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
                    if callback_handler.is_finished() {
                        callback_closed.store(true, std::sync::atomic::Ordering::Relaxed);
                        tracing::info!("Callback Handler is Finished: {:?}", callback_handler);
                        callback_handler.join().expect("Callback errror - not finished")?;
                        break;
                    } else {
                        tracing::warn!("Sending shutdown signal to CallbackManager.");
                        shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");
                        // break;
                    }
                    let task_result = res.expect("Task panicked");
                    task_result?; // Propagate any errors
                    break;
                }


                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if start.elapsed() > timeout {
                        panic!("Test timed out without completion signal");
                    }
                }
            }
        }
        if idx_handle.is_finished() {
            tracing::info!("Indexer Handle is Finished: {:?}", idx_handle);
            // inner result
        } else {
            tracing::error!("Indexer Handle did not finish.")
        }
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
                tracing::trace!("row {: <2}: {} | {:?} {: >30}", i, is_found, name, idx);
                let node_data = (i, name, idx);
                if is_found {
                    found.push(node_data);
                } else {
                    not_found.push(node_data);
                }
            });
        for (i, name, idx) in not_found.iter() {
            tracing::trace!(target: "dbg_rows", "row not found {: <2} | {:?} {: >30}", i, name, idx);
        }
        for (i, name, idx) in all_pending_rows
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r[0].clone(), r[1].clone()))
        {
            tracing::trace!(target: "dbg_rows","row found {: <2} | {:?} {: >30}", i, name, idx);
        }
        if !callback_closed.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("CallbackManager not closed?");
        }
        let inner = counter.load(std::sync::atomic::Ordering::SeqCst);
        tracing::info!(
            "updated rows: {}, pending db callback: {}",
            not_found.len(),
            found.len()
        );
        tracing::info!("Ending test_next_batch: {fixture}: total count {inner}, counter {total_count} | {inner}/{total_count}");
        assert!(
            total_count == counter.load(std::sync::atomic::Ordering::SeqCst),
            // received_completed.load(std::sync::atomic::Ordering::SeqCst),
            "Indexer completed without sending completion status: Miscount: {inner}/{total_count}"
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

    async fn test_next_batch_ss(fixture: &'static str) -> Result<(), ploke_error::Error> {
        tracing::info!("Starting test_next_batch: {fixture}");

        let cozo_db = setup_db_full(fixture)?;
        let db = Arc::new(Database::new(cozo_db));
        let total_count = db.count_unembedded_nonfiles()?;
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
            Arc::new( embedding_processor ),
            cancellation_token,
            batch_size,
        );
        let (progress_tx_nonarc, mut progress_rx) = broadcast::channel(1000);
        let progress_tx_arc = Arc::new(progress_tx_nonarc);
        let (control_tx, control_rx) = mpsc::channel(4);

        let callback_handler = std::thread::spawn(move || callback_manager.run());
        let mut idx_handle =
            tokio::spawn(async move { idx_tag.run(progress_tx_arc, control_rx).await });

        let received_completed = AtomicBool::new(false);
        let callback_closed = AtomicBool::new(false);
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
                        break;
                    }
                }
            };
            tokio::select! {
                biased;

                status = progress_rx.recv() => {
                    match status {
                        Ok(status) => {
                            match status.status {
                                IndexStatus::Failed(s)=>{
                                    tracing::debug!("Indexing failed with message: {}\nErrors: {:?}",
                                        s,status.errors);
                                    panic!("Indexing failed with message: {}\nErrors: {:?}",s,status.errors);
                                }
                                IndexStatus::Idle => {todo!()},
                                IndexStatus::Running => {},
                                IndexStatus::Paused => {todo!()},
                                IndexStatus::Completed => {
                                    tracing::debug!("Progress: {:?}", status);
                                    received_completed.store(true, std::sync::atomic::Ordering::SeqCst);
                                    if callback_handler.is_finished() {
                                        callback_closed.store(true, std::sync::atomic::Ordering::Relaxed);
                                        tracing::info!("Callback Handler is Finished: {:?}", callback_handler);
                                        callback_handler.join().expect("Callback errror - not finished")?;
                                        break;
                                    } else {
                                        tracing::warn!("Sending shutdown signal to CallbackManager.");
                                        shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");
                                        // break;
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
                    if callback_handler.is_finished() {
                        callback_closed.store(true, std::sync::atomic::Ordering::Relaxed);
                        tracing::info!("Callback Handler is Finished: {:?}", callback_handler);
                        callback_handler.join().expect("Callback errror - not finished")?;
                        break;
                    } else {
                        tracing::warn!("Sending shutdown signal to CallbackManager.");
                        shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");
                        // break;
                    }
                    let task_result = res.expect("Task panicked");
                    task_result?; // Propagate any errors
                    break;
                }


                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if start.elapsed() > timeout {
                        panic!("Test timed out without completion signal");
                    }
                }
            }
        }
        if idx_handle.is_finished() {
            tracing::info!("Indexer Handle is Finished: {:?}", idx_handle);
            // inner result
        } else {
            tracing::error!("Indexer Handle did not finish.")
        }
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
                tracing::trace!("row {: <2}: {} | {:?} {: >30}", i, is_found, name, idx);
                let node_data = (i, name, idx);
                if is_found {
                    found.push(node_data);
                } else {
                    not_found.push(node_data);
                }
            });
        let k = 2;
        let ef = 2;
        // tracing::info!("{:?}", query_embedding);
        for (i, name, idx) in not_found.iter() {
            tracing::trace!(target: "dbg_rows", "row not found {: <2} | {:?} {: >30}", i, name, idx);
        }
        let mut test_vec: Vec<Vec<f32>> = Vec::new();
        for (i, name, idx) in all_pending_rows
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r[0].clone(), r[1].clone()))
        {
            tracing::trace!(target: "dbg_rows","row found {: <2} | {:?} {: >30}", i, name, idx);
        }
        for ty in NodeType::primary_nodes() {
            let db_ret = ploke_db::create_index_warn(&db, ty);
            tracing::info!("db_ret = {:?}", db_ret);
        }

        let mut no_error = true;
        for ty in NodeType::primary_nodes() {
            match ploke_db::hnsw_of_type(&db, ty, ef, k) {
                Ok(indexed_count) => {
                    tracing::info!("db_ret = {:?}", indexed_count);
                }
                Err(w) if w.is_warning() => {
                    tracing::warn!("No index found for rel: {}", ty.relation_str());
                }
                Err(e) => {
                    tracing::error!("{}", e.to_string());
                }
            };
        }
        assert!(no_error);
        let db_ret = db
            .run_script(
                "::indices function",
                BTreeMap::new(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(DbError::from)?;
        tracing::info!("db_ret = {:?}", db_ret);
        if !callback_closed.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("CallbackManager not closed?");
        }
        match hnsw_all_types(&db, k, ef) {
            Ok(indexed_count) => {
                tracing::info!("db_ret = {:?}", indexed_count);
            }
            Err(w) if w.is_warning() => {
                tracing::warn!("No index found for unknown rel");
            }
            Err(e) => {
                tracing::error!("{}", e.to_string());
            }
        };
        let inner = counter.load(std::sync::atomic::Ordering::SeqCst);
        tracing::info!(
            "updated rows: {}, pending db callback: {}",
            not_found.len(),
            found.len()
        );
        tracing::info!("Ending test_next_batch: {fixture}: total count {inner}, counter {total_count} | {inner}/{total_count}");
        assert!(
            total_count == counter.load(std::sync::atomic::Ordering::SeqCst),
            // received_completed.load(std::sync::atomic::Ordering::SeqCst),
            "Indexer completed without sending completion status: Miscount: {inner}/{total_count}"
        );
        // .filter_map(|r| r.last().map(|c| c.get_slice().unwrap()))
        // .collect::<Vec<f32>>();
        // ploke_db::(&db, &query_embedding, k, ef);
        Ok(())
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_ss_nodes() -> Result<(), Error> {
        init_test_tracing_temporary(Level::INFO);
        test_next_batch_ss("fixture_nodes").await
    }

    #[tokio::test]
    #[ignore = "requires further improvement of syn_parser"]
    // FIX: failing on step:
    // INFO  Parse: build module tree
    //    at crates/test-utils/src/lib.rs:65
    async fn test_batch_ss_file_dir_detection() -> Result<(), Error> {
        init_test_tracing(Level::TRACE);
        test_next_batch_ss("file_dir_detection").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_ss_attributes() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_attributes").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_ss_cyclic_types() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_cyclic_types").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_ss_edge_cases() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_edge_cases").await
    }
    #[tokio::test]
    // INFO: passing
    async fn test_batch_ss_generics() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_generics").await
    }
    #[tokio::test]
    // INFO: passing
    async fn test_batch_ss_macros() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_macros").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_ss_path_resolution() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_path_resolution").await
    }
    #[tokio::test]
    #[ignore = "requires further improvement of syn_parser"]
    // WARN: failing (dependent upon other improvements in cfg?)
    async fn test_batch_ss_spp_edge_cases_cfg() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_spp_edge_cases").await
    }
    #[tokio::test]
    #[ignore = "requires further improvement of syn_parser"]
    // WARN: failing (dependent upon other improvements)
    async fn test_batch_ss_spp_edge_cases_no_cfg() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_spp_edge_cases_no_cfg").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_ss_tracking_hash() -> Result<(), Error> {
        init_test_tracing_temporary(Level::INFO);
        test_next_batch_ss("fixture_tracking_hash").await
    }
    #[tokio::test]
    // NOTE: passing
    async fn test_batch_ss_types() -> Result<(), Error> {
        init_test_tracing(Level::INFO);
        test_next_batch_ss("fixture_types").await
    }
}
