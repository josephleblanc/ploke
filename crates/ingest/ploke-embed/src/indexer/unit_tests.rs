#![cfg(test)]

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
use ploke_db::{bm25_index::{self, bm25_service::Bm25Cmd}, hnsw_all_types, CallbackManager, Database, DbError, NodeType};
use ploke_error::Error;
use ploke_io::IoManagerHandle;
use ploke_test_utils::{setup_db_full, setup_db_full_crate};
use tokio::{
    sync::{
        broadcast::{self, error::TryRecvError},
        mpsc, Mutex,
    },
    time::{self, Instant},
};
use tracing::{level_filters::LevelFilter, Level};
use tracing_subscriber::{filter, fmt, prelude::*, EnvFilter};

use crate::{
    cancel_token::CancellationToken,
    error::EmbedError,
    indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexerTask, IndexingStatus},
    local::{EmbeddingConfig, EmbeddingError, LocalEmbedder},
};

pub fn init_test_tracing(level: impl Into<LevelFilter> + Into<Level> + Copy) -> tracing::subscriber::DefaultGuard {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("ploke", level)
        .with_target("ploke-db", level)
        .with_target("ploke-embed", level)
        .with_target("ploke-io", level)
        .with_target("ploke-transform", level)
        .with_target("transform_functions", level)
        .with_target("cozo", tracing::Level::ERROR);

    let fmt = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .with_level(true)
        .without_time()
        .pretty();

    // Build a subscriber and set it as the *default for the current scope only*
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt)
        .set_default()
}

#[tokio::test]
// NOTE: passing
async fn test_next_batch_only() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_nodes").await
}
#[tokio::test]
#[ignore = "requires further improvement of syn_parser"]
// FIX: failing on step:
// INFO  Parse: build module tree
//    at crates/test-utils/src/lib.rs:65
async fn test_batch_file_dir_detection() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::TRACE);
    test_next_batch("file_dir_detection").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_attributes() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_attributes").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_cyclic_types() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_cyclic_types").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_edge_cases() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_edge_cases").await
}
#[tokio::test]
// INFO: passing
async fn test_batch_generics() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_generics").await
}
#[tokio::test]
// INFO: passing
async fn test_batch_macros() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_macros").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_path_resolution() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_path_resolution").await
}
#[tokio::test]
#[ignore = "requires further improvement of syn_parser"]
// WARN: failing (dependent upon other improvements in cfg?)
async fn test_batch_spp_edge_cases_cfg() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_spp_edge_cases").await
}
#[tokio::test]
#[ignore = "requires further improvement of syn_parser"]
// WARN: failing (dependent upon other improvements)
async fn test_batch_spp_edge_cases_no_cfg() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_spp_edge_cases_no_cfg").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_tracking_hash() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_tracking_hash").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_types() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch("fixture_types").await
}
async fn test_full() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
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
// let _guard = init_test_tracing(Level::INFO);
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

    let bm25_tx = bm25_index::bm25_service::start(0.0);

    let idx_tag = IndexerTask::new(
        Arc::clone(&db),
        io,
        Arc::new(embedding_processor),
        cancellation_token,
        batch_size,
    ).with_bm25_tx(bm25_tx);
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

async fn test_next_batch_ss(target_crate: &'static str) -> Result<(), ploke_error::Error> {
    tracing::info!("Starting test_next_batch: {target_crate}");

    let cozo_db = if target_crate.starts_with("fixture") { 
        setup_db_full(target_crate)
    } else if target_crate.starts_with("crates") {
        let crate_name = target_crate.trim_start_matches("crates/");
        setup_db_full_crate(crate_name)
    } else { 
        return Err(ploke_error::Error::Fatal(ploke_error::FatalError::SyntaxError("Incorrect usage of test_next_batch_ss test input.".to_string())));
    }?;
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
        Arc::new(embedding_processor),
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
    tracing::info!("Ending test_next_batch: {target_crate}: total count {inner}, counter {total_count} | {inner}/{total_count}");
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
async fn test_index_bm25() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::TRACE);
    test_next_batch_ss("fixture_nodes").await?;

    Ok(())
}

#[tokio::test]
// NOTE: passing
async fn test_batch_ss_nodes() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_nodes").await
}

// #[ignore = "requires further improvement of syn_parser"]
// FIX: failing on step:
// INFO  Parse: build module tree
//    at crates/test-utils/src/lib.rs:65
#[tokio::test]
async fn test_batch_ss_file_dir_detection() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::TRACE);
    test_next_batch_ss("file_dir_detection").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_ss_attributes() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_attributes").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_ss_cyclic_types() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_cyclic_types").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_ss_edge_cases() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_edge_cases").await
}
#[tokio::test]
// INFO: passing
async fn test_batch_ss_generics() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_generics").await
}
#[tokio::test]
// INFO: passing
async fn test_batch_ss_macros() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_macros").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_ss_path_resolution() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_path_resolution").await
}
#[tokio::test]
#[ignore = "requires further improvement of syn_parser"]
// WARN: failing (dependent upon other improvements in cfg?)
async fn test_batch_ss_spp_edge_cases_cfg() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_spp_edge_cases").await
}
#[tokio::test]
#[ignore = "requires further improvement of syn_parser"]
// WARN: failing (dependent upon other improvements)
async fn test_batch_ss_spp_edge_cases_no_cfg() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_spp_edge_cases_no_cfg").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_ss_tracking_hash() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_tracking_hash").await
}
#[tokio::test]
// NOTE: passing
async fn test_batch_ss_types() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("fixture_types").await
}
// ------- test on own crates -------
#[tokio::test]
// NOTE: passing - takes about 800 seconds
async fn test_batch_ss_syn() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ingest/syn_parser").await
}
#[tokio::test]
// NOTE: passing - takes about 575 seconds
async fn test_batch_ss_transform() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ingest/ploke-transform").await
}
#[tokio::test]
// NOTE: passing - takes about 260 seconds
// - embedded 83/83
async fn test_batch_ss_embed() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ingest/ploke-embed").await
}
#[tokio::test]
// NOTE: passing - takes about 98 seconds
// - embedded 28/28
async fn test_batch_ss_core() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ploke-core").await
}
#[tokio::test]
// NOTE: passing - takes about 258 seconds
// - embedded 74/74
async fn test_batch_ss_db() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ploke-db").await
}
#[tokio::test]
// NOTE: passing - takes about 30 seconds
// - embedded 10/10
async fn test_batch_ss_error() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ploke-error").await
}
#[tokio::test]
// NOTE: passing - takes about 159 seconds
// - embedded 33/33
async fn test_batch_ss_io() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ploke-io").await
}
#[tokio::test]
// NOTE: passing - takes about 1.06 seconds
// - embedded 2/2
async fn test_batch_ss_rag() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ploke-rag").await
}
#[tokio::test]
// NOTE: passing - takes about 413.18 seconds
// - embedded 132/132
async fn test_batch_ss_tui() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ploke-tui").await
}
#[tokio::test]
// NOTE: passing - takes about 43.71 seconds <-- probably wrong
// - embedded 9/9
async fn test_batch_ss_ty_mcp() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/ploke-ty-mcp").await
}
#[tokio::test]
// NOTE: passing - takes about 76.80 seconds
// - embedded 17/17
async fn test_batch_ss_test_utils() -> Result<(), Error> {
    let _guard = init_test_tracing(Level::INFO);
    test_next_batch_ss("crates/test-utils").await
}

