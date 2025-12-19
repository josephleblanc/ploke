#![allow(unused_mut)]
mod unit_tests;

use crate::local::{EmbeddingConfig, LocalEmbedder};
use crate::providers::hugging_face::HuggingFaceBackend;
use crate::providers::openai::OpenAIBackend;
use crate::providers::openrouter::OpenRouterBackend;
use crate::runtime::EmbeddingRuntime;
use crate::{config::CozoConfig, error::truncate_string};
use cozo::{CallbackOp, DataValue, NamedRows};
use ploke_core::EmbeddingData;
use ploke_db::{bm25_index, CallbackManager, Database, NodeType, TypedEmbedData};
use ploke_io::IoManagerHandle;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{self, Instant};
use tracing::{info_span, instrument};
use uuid::Uuid;

use crate::cancel_token::CancellationHandle;
use crate::{
    cancel_token::{CancellationListener, CancellationToken},
    error::EmbedError,
};
use ploke_db::bm25_index::{bm25_service, CodeTokenizer, DocData, DocMeta};

#[derive(Debug)]
pub struct EmbeddingProcessor {
    source: EmbeddingSource,
}

#[derive(Debug)]
pub enum EmbeddingSource {
    Local(LocalEmbedder),
    HuggingFace(HuggingFaceBackend),
    OpenAI(OpenAIBackend),
    OpenRouter(OpenRouterBackend),
    Cozo(CozoBackend),
}

fn count_tyemb(tyemb_vec: &[TypedEmbedData]) -> usize {
    tyemb_vec.iter().fold(0, |acc, i| acc + i.v.len())
}

impl EmbeddingProcessor {
    pub fn new(source: EmbeddingSource) -> Self {
        Self { source }
    }

    /// Create a lightweight mock embedder for tests.
    /// Uses the Cozo backend placeholder with a fixed dimension.
    pub fn new_mock() -> Self {
        Self {
            source: EmbeddingSource::Cozo(CozoBackend {
                endpoint: "mock://cozo".to_string(),
                dimensions: 384,
            }),
        }
    }

    pub async fn generate_embeddings(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.generate_embeddings_with_cancel(snippets, None).await
    }

    #[instrument(skip_all, fields(source = ?self.source, target = "embed-pipeline"))]
    pub async fn generate_embeddings_with_cancel(
        &self,
        snippets: Vec<String>,
        cancel: Option<&CancellationListener>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        tracing::trace!(target: "embed-pipeline", "Starting generate_embeddings with EmbeddingSource dimensions: {:#?} with {} snippets\nfirst snippet: {:?}\nlast snippet: {:?}",
            self.dimensions(),
            snippets.len(),
            snippets.first(),
            snippets.last(),
        );
        match &self.source {
            EmbeddingSource::Local(backend) => {
                let text_slices: Vec<&str> = snippets.iter().map(|s| s.as_str()).collect();
                Ok(backend
                    .embed_batch(&text_slices)
                    .inspect(|v| {
                        tracing::trace!("OK Returning from embed_batch with vec(s): {:?}", v);
                    })
                    .inspect_err(|e| {
                        tracing::trace!(
                            "Error Returning from embed_batch with error: {:?}",
                            e.to_string()
                        );
                    })?)
            }
            EmbeddingSource::HuggingFace(backend) => backend.compute_batch(snippets).await,
            EmbeddingSource::OpenAI(backend) => backend.compute_batch(snippets).await,
            EmbeddingSource::OpenRouter(backend) => backend.compute_batch(snippets, cancel).await,
            EmbeddingSource::Cozo(backend) => backend.compute_batch(snippets).await,
        }
    }

    pub fn dimensions(&self) -> usize {
        match &self.source {
            EmbeddingSource::Local(backend) => backend.dimensions(),
            EmbeddingSource::HuggingFace(backend) => backend.dimensions,
            EmbeddingSource::OpenAI(backend) => backend.dimensions,
            EmbeddingSource::OpenRouter(backend) => backend.dimensions,
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
    pub embedding_runtime: Arc<EmbeddingRuntime>,
    pub cancellation_token: CancellationToken,
    // Keep the cancellation channel sender alive for the duration of the IndexerTask.
    // Without this, `CancellationListener::cancelled()` completes immediately (sender dropped),
    // which incorrectly cancels remote embedding requests.
    #[allow(dead_code)]
    cancellation_handle: CancellationHandle,
    pub batch_size: usize,
    pub bm25_tx: Option<mpsc::Sender<bm25_service::Bm25Cmd>>,
    pub cursors: Mutex<HashMap<NodeType, Uuid>>,
    pub total_processed: AtomicUsize,
}

impl IndexerTask {
    pub fn new(
        db: Arc<Database>,
        io: IoManagerHandle,
        embedding_runtime: Arc<EmbeddingRuntime>,
        cancellation_token: CancellationToken,
        cancellation_handle: CancellationHandle,
        batch_size: usize,
    ) -> Self {
        Self {
            db,
            io,
            embedding_runtime,
            cancellation_token,
            cancellation_handle,
            batch_size,
            bm25_tx: None,
            cursors: Mutex::new(HashMap::new()),
            total_processed: AtomicUsize::new(0),
        }
    }

    pub fn with_bm25_tx(mut self, bm25_tx: mpsc::Sender<bm25_service::Bm25Cmd>) -> Self {
        self.bm25_tx = Some(bm25_tx);
        self
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
        progress_tx: Arc<broadcast::Sender<IndexingStatus>>,
        mut progress_rx: broadcast::Receiver<IndexingStatus>,
        control_rx: mpsc::Receiver<IndexerCommand>,
        callback_handler: std::thread::JoinHandle<Result<(), ploke_db::DbError>>,
        db_callbacks: crossbeam_channel::Receiver<
            Result<(CallbackOp, NamedRows, NamedRows), ploke_db::DbError>,
        >,
        counter: Arc<AtomicUsize>,
        shutdown: crossbeam_channel::Sender<()>,
    ) -> Result<(), ploke_error::Error> {
        // let (cancellation_token, cancel_handle) = CancellationToken::new();
        tracing::info!("Starting index_workspace: {}", &workspace_dir);
        let db_clone = Arc::clone(&task.db);
        ploke_db::create_index_primary(&db_clone)?;
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
                        // shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");
                        match shutdown.send(()) {
                            Ok(_) => tracing::info!("Sending shutdown message"),
                            Err(e) => tracing::error!("Cannot send shutdown message, other side dropped"),
                        };
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
            .map(|(i, r)| (i, r[0].clone(), r[1].clone(), r[2].clone()))
            .for_each(|(i, idx, at, name)| {
                let is_not_indexed = all_pending_rows.rows.iter().any(|r| r[0] == idx);
                tracing::trace!(
                    "row {: <2}: {} | {:?} - {} - {: >30}",
                    i,
                    is_not_indexed,
                    at,
                    name,
                    idx
                );
                let node_data = (i, at, name, idx);
                if is_not_indexed {
                    not_indexed.push(node_data);
                } else {
                    indexed.push(node_data);
                }
            });
        for (i, at, name, idx) in indexed {
            tracing::trace!(target: "dbg_rows", "row indexed {: <2} | {:?} - {} - {: >30}", i, at, name, idx);
        }
        for (i, at, name, idx) in all_pending_rows
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r[0].clone(), r[1].clone(), r[2].clone()))
        {
            tracing::trace!(target: "dbg_rows","row not_indexed {: <2} | {:?} - {} - {: >30}", i, at, name, idx);
        }

        ploke_db::create_index_primary_with_index(&db_clone)?;

        tracing::info!("Ending index_workspace: {workspace_dir}");
        let inner = counter.load(std::sync::atomic::Ordering::SeqCst);
        tracing::info!("Ending index_workspace: {workspace_dir}: total count {inner}, counter {total_count_not_indexed} | {inner}/{total_count_not_indexed}");

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
        // Ensure the active embedding set is ready (relations exist) before any batch writes.
        // This is critical for remote embeddings where provider/model/dims differ from defaults.
        use ploke_db::multi_embedding::db_ext::EmbeddingExt as _;
        self.db.ensure_embedding_set_relation()?;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.embedding_runtime.current_active_set()?;
        self.db.put_embedding_set(&active_embedding_set)?;
        self.db
            .ensure_vector_embedding_relation(&active_embedding_set)?;

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
            tracing::trace!("node_count after next_batch: {}", node_count);

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

                    // Fail fast: if a batch cannot be processed, the UI should surface failure
                    // rather than appearing to "stall" at 0% with repeated batch errors.
                    let msg = state
                        .errors
                        .last()
                        .cloned()
                        .unwrap_or_else(|| "batch process failed".to_string());
                    state.status = IndexStatus::Failed(msg);
                    progress_tx.send(state.clone())?;
                    return Err(e);
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

            // If BM25 is configured, request FinalizeSeed and await ack before committing completion.
            if let Some(tx) = &self.bm25_tx {
                let (resp_tx, resp_rx) = oneshot::channel::<Result<(), String>>();
                if let Err(e) = tx
                    .send(bm25_service::Bm25Cmd::FinalizeSeed { resp: resp_tx })
                    .await
                {
                    let msg = format!("Failed to send BM25 FinalizeSeed: {}", e);
                    tracing::error!("{}", &msg);
                    state.status = IndexStatus::Failed(msg);
                    progress_tx.send(state)?;
                    return Err(EmbedError::NotImplemented(
                        "BM25 FinalizeSeed send failed".into(),
                    ));
                }
                match resp_rx.await {
                    Ok(Ok(())) => {
                        tracing::info!("BM25 FinalizeSeed acknowledged");
                    }
                    Ok(Err(err_msg)) => {
                        let msg = format!("BM25 FinalizeSeed failed: {}", err_msg);
                        tracing::error!("{}", &msg);
                        state.status = IndexStatus::Failed(msg);
                        progress_tx.send(state)?;
                        return Err(EmbedError::NotImplemented(
                            "BM25 FinalizeSeed failed".into(),
                        ));
                    }
                    Err(recv_err) => {
                        let msg = format!("BM25 FinalizeSeed channel closed: {}", recv_err);
                        tracing::error!("{}", &msg);
                        state.status = IndexStatus::Failed(msg);
                        progress_tx.send(state)?;
                        return Err(EmbedError::NotImplemented(
                            "BM25 FinalizeSeed channel closed".into(),
                        ));
                    }
                }
            }

            state.status = IndexStatus::Completed;
            self.reset_cursors().await;
            progress_tx.send(state)?;
        } else {
            tracing::warn!("Indexing cancelled");
            state.status = IndexStatus::Cancelled;
            progress_tx.send(state)?;
        };
        Ok(())
    }

    pub async fn reset_cursors(&self) {
        let mut cursors = self.cursors.lock().await;
        for value in cursors.values_mut() {
            *value = Uuid::nil();
        }
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
    pub(crate) async fn next_batch(
        &self,
        num_not_proc: usize,
    ) -> Result<Option<Vec<TypedEmbedData>>, EmbedError> {
        tracing::trace!("starting next_batch");
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
            tracing::debug!(
                target: "ploke-embed::next_batch",
                rel = %node_type.relation_str(),
                fetched = nodes.len(),
                total_counted,
                fetch_size,
                cursor = %cursor,
            );

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
            } else {
                tracing::trace!(
                    target: "ploke-embed::next_batch",
                    "no nodes returned for {rel}",
                    rel = node_type.relation_str()
                );
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
        tracing::debug!(target: "embed-pipeline",
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

        // Send DocMeta to BM25 service instead of full snippets
        if let Some(tx) = &self.bm25_tx {
            let docs_data: Vec<DocData> = valid_data
                .iter()
                .zip(valid_snippets.iter())
                .map(DocData::from_embed_clone)
                .collect();

            tracing::debug!(
                "Sending {} processed snippets to BM25 service",
                docs_data.len()
            );

            if let Err(e) = tx.try_send(bm25_service::Bm25Cmd::IndexBatch { docs: docs_data }) {
                tracing::warn!("BM25 IndexBatch try_send failed: {}", e);
            }
        } else {
            tracing::trace!("BM25 service not configured; skipping sparse indexing for this batch");
        }

        let embeddings = self
            .embedding_runtime
            .generate_embeddings_with_cancel(
                valid_snippets,
                Some(&self.cancellation_token.listener()),
            )
            .await?;
        tracing::trace!(
            "Processed embeddings {} with dimension {:?}",
            embeddings.len(),
            embeddings.first().map(|v| v.len())
        );

        let dims = self.embedding_runtime.dimensions()?;
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
        self.db.update_embeddings_batch(updates)?;
        tracing::info!("Finished processing batch");
        Ok(())
    }
}

fn log_row(r: Vec<DataValue>) {
    for (i, row) in r.iter().enumerate() {
        tracing::info!("{}: {:?}", i, row);
    }
}
pub(crate) fn log_stuff(
    call: CallbackOp,
    new: NamedRows,
    old: NamedRows,
    counter: Arc<AtomicUsize>,
) {
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
