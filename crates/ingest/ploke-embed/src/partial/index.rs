use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use cozo::{CallbackOp, NamedRows};
use tokio::{
    sync::{broadcast, mpsc, Mutex},
    time::{self, Instant},
};
use tracing::instrument;

use crate::{error::{truncate_string, EmbedError}, indexer::{log_stuff, IndexStatus, IndexerCommand, IndexerTask, IndexingStatus}};

// TODO: Consider returning a reset version of Self instead of consuming self here.
// In the same vein consider not dropping the callback item.
#[allow(unused_mut)]
#[cfg(feature = "update_embeds")]
pub async fn index_files(
    task: Arc<IndexerTask>,
    workspace_dir: &[ String ],
    // db_callback: crossbeam_channel::Receiver<Result<(CallbackOp, NamedRows, NamedRows), EmbedError>>
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
    todo!();
    // let (cancellation_token, cancel_handle) = CancellationToken::new();
    tracing::info!("Starting index_files:\n{}", workspace_dir.join("\n"));
    let db_clone = Arc::clone(&task.db);
    let total_count_not_indexed = db_clone.count_nodes_for_update()?;

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
            tracing::trace!(
                "row {: <2}: {} | {:?} {: >30}",
                i,
                is_not_indexed,
                name,
                idx
            );
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
