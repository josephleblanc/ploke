use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use cozo::{CallbackOp, NamedRows};
use tokio::sync::{broadcast, mpsc};
use tracing::instrument;

use crate::{
    error::{EmbedError, truncate_string},
    indexer::{IndexStatus, IndexerCommand, IndexerTask, IndexingStatus},
};

// TODO: Consider returning a reset version of Self instead of consuming self here.
// In the same vein consider not dropping the callback item.
#[allow(unused_mut)]
#[cfg(feature = "update_embeds")]
pub async fn index_files(
    _task: Arc<IndexerTask>,
    _workspace_dir: &[String],
    // db_callback: crossbeam_channel::Receiver<Result<(CallbackOp, NamedRows, NamedRows), EmbedError>>
    _progress_tx: Arc<broadcast::Sender<IndexingStatus>>,
    _progress_rx: broadcast::Receiver<IndexingStatus>,
    _control_rx: mpsc::Receiver<IndexerCommand>,
    _callback_handler: std::thread::JoinHandle<Result<(), ploke_db::DbError>>,
    _db_callbacks: crossbeam_channel::Receiver<
        Result<(CallbackOp, NamedRows, NamedRows), ploke_db::DbError>,
    >,
    _counter: Arc<AtomicUsize>,
    _shutdown: crossbeam_channel::Sender<()>,
) -> Result<(), ploke_error::Error> {
    Err(ploke_error::Error::Internal(
        ploke_error::InternalError::NotImplemented(
            "partial update embedding indexing is not implemented".to_string(),
        ),
    ))
}

#[instrument(
        name = "Indexer::run",
        skip(indexer_task, progress_tx, control_rx),
        fields(num_not_proc, recent_processed, status="Running")  // Track key state
    )]
#[cfg(feature = "update_embeds")]
pub async fn run(
    indexer_task: &IndexerTask,
    progress_tx: Arc<broadcast::Sender<IndexingStatus>>,
    mut control_rx: mpsc::Receiver<IndexerCommand>,
) -> Result<(), EmbedError> {
    let num_not_proc = indexer_task.db.count_unembedded_nonfiles()?;
    tracing::info!("Starting indexing with {} unembedded nodes", num_not_proc);
    let mut state = IndexingStatus {
        status: IndexStatus::Running,
        recent_processed: 0,
        num_not_proc,
        current_file: None,
        errors: Vec::new(),
    };
    progress_tx.send(state.clone())?;

    while let Some(batch) = indexer_task.next_batch(num_not_proc).await? {
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

        match indexer_task
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

    let total_processed = indexer_task.total_processed.load(Ordering::SeqCst);
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
