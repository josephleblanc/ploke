use std::{collections::HashMap, sync::Arc};

use crate::{Database, DbError};

use super::{Bm25Indexer, DocData, DocMeta};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum Bm25Status {
    Uninitialized,
    Building,
    Ready { docs: usize },
    Empty,
    Error(String),
}

#[derive(Debug)]
pub enum Bm25Cmd {
    /// Index a batch of (Uuid, DocMeta) documents with a tokenizer version tag.
    IndexBatch { docs: Vec<DocData> },
    /// Remove documents from the sparse index by id.
    Remove { ids: Vec<Uuid> },
    /// Rebuild the index from source of truth (placeholder; no-op for now).
    Rebuild,
    /// Finalize the seed/build phase; commit metadata and return ack.
    FinalizeSeed { resp: oneshot::Sender<Result<(), String>> },
    /// Search the index and respond via oneshot with (id, score) pairs.
    Search { query: String, top_k: usize, resp: oneshot::Sender<Vec<(Uuid, f32)>> },
    /// Get current lifecycle/status of the BM25 actor/index.
    Status { resp: oneshot::Sender<Result<Bm25Status, DbError>> },
    /// Persist lightweight index metadata to disk (placeholder; sidecar only).
    Save { path: std::path::PathBuf, resp: oneshot::Sender<Result<(), DbError>> },
    /// Load persisted state or rebuild from DB as a functional fallback.
    Load { path: std::path::PathBuf, resp: oneshot::Sender<Result<(), DbError>> },
}

/// Start the BM25 actor with a given avgdl parameter.
/// Returns an mpsc::Sender<Bm25Cmd> handle for issuing commands.
pub fn start(db: Arc<Database>, avgdl: f32) -> Result< mpsc::Sender<Bm25Cmd> , DbError> {
    let (tx, mut rx) = mpsc::channel::<Bm25Cmd>(128);
    let mut indexer = Bm25Indexer::new(avgdl);
    let mut status = Bm25Status::Uninitialized;

    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Bm25Cmd::IndexBatch { docs } => {
                    tracing::debug!("BM25 IndexBatch: {} docs", docs.len());
                    match indexer.upsert_batch_with_cozo(db.as_ref(), docs.into_iter()) {
                        Ok(_n) => {
                            let docs = indexer.doc_count();
                            status = if docs == 0 {
                                Bm25Status::Empty
                            } else {
                                Bm25Status::Ready { docs }
                            };
                        }
                        Err(e) => {
                            tracing::error!("Error upserting batch with cozo: {e}");
                            status = Bm25Status::Error(e.to_string());
                        }
                    }
                }
                Bm25Cmd::Remove { ids } => {
                    tracing::debug!("BM25 Remove: {} docs", ids.len());
                    for id in ids {
                        indexer.remove(&id);
                    }
                    let docs = indexer.doc_count();
                    status = if docs == 0 {
                        Bm25Status::Empty
                    } else {
                        Bm25Status::Ready { docs }
                    };
                }
                Bm25Cmd::Rebuild => {
                    tracing::info!("BM25 Rebuild: starting rebuild from database");
                    // status = Bm25Status::Building;
                    match Bm25Indexer::rebuild_from_db(db.as_ref()) {
                        Ok(new_indexer) => {
                            let docs = new_indexer.doc_count();
                            indexer = new_indexer;
                            status = if docs == 0 {
                                Bm25Status::Empty
                            } else {
                                Bm25Status::Ready { docs }
                            };
                            tracing::info!("BM25 Rebuild: completed successfully with {} docs", docs);
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            tracing::error!("BM25 Rebuild: failed with error: {msg}");
                            status = Bm25Status::Error(msg);
                        }
                    }
                }
                Bm25Cmd::FinalizeSeed { resp } => {
                    // Compute avgdl from staged metadata and drain it for persistence.
                    let avgdl = indexer.compute_avgdl_from_staged();
                    // let staged = indexer.upsert_batch_with_cozo(db);
                    let staged = indexer.drain_staged_meta();
                    tracing::info!(
                        "BM25 FinalizeSeed: {} docs staged, computed avgdl={}",
                        staged.len(),
                        avgdl
                    );
                    let _ = resp.send(Ok(()));
                }
                Bm25Cmd::Search { query, top_k, resp } => {
                    tracing::debug!("query: {query}, top_k: {top_k}, resp: {resp:?}");
                    let scored = indexer.search(&query, top_k);
                    tracing::trace!("scored: {scored:?}");
                    let results: Vec<(Uuid, f32)> =
                        scored.into_iter().map(|d| (d.id, d.score)).collect();
                    if resp.send(results).is_err() {
                        tracing::warn!("BM25 search response receiver dropped before sending results");
                    }
                }
                Bm25Cmd::Status { resp } => {
                    let _ = resp.send(Ok(status.clone()));
                }
                Bm25Cmd::Save { path, resp } => {
                    let docs = indexer.doc_count();
                    let content = format!(r#"{{"version":"{}","docs":{}}}"#, super::TOKENIZER_VERSION, docs);
                    let result = (|| -> Result<(), std::io::Error> {
                        if let Some(dir) = path.parent() {
                            if !dir.as_os_str().is_empty() {
                                std::fs::create_dir_all(dir)?;
                            }
                        }
                        std::fs::write(&path, content)?;
                        Ok(())
                    })().map_err(|e| DbError::Cozo(format!("bm25 save error: {}", e)));
                    let _ = resp.send(result);
                }
                Bm25Cmd::Load { path, resp } => {
                    tracing::info!("BM25 Load requested from {:?}", path);
                    // Placeholder implementation: perform a rebuild to ensure a ready index.
                    // status = Bm25Status::Building;
                    let res = match Bm25Indexer::rebuild_from_db(db.as_ref()) {
                        Ok(new_indexer) => {
                            let docs = new_indexer.doc_count();
                            indexer = new_indexer;
                            status = if docs == 0 {
                                Bm25Status::Empty
                            } else {
                                Bm25Status::Ready { docs }
                            };
                            Ok(())
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            status = Bm25Status::Error(msg.clone());
                            Err(DbError::Cozo(format!("bm25 load error: {}", msg)))
                        }
                    };
                    let _ = resp.send(res);
                }
            }
        }
        tracing::info!("BM25 service actor loop ended");
    });

    Ok( tx )
}


/// Convenience starter with a reasonable default avgdl.
pub fn start_default(db: Arc<Database>) -> Result< mpsc::Sender<Bm25Cmd>, DbError > {
    start(db, 10.0)
}

/// Start the BM25 actor by rebuilding the in-memory index from the Cozo database.
///
/// This scans all primary node relations, computes a fitted avgdl, and populates the in-memory
/// scorer so the service is immediately queryable.
///
/// Example (no_run):
/// ```
/// # use std::sync::Arc;
/// # use ploke_db::{Database, DbError};
/// # use ploke_db::bm25_index::bm25_service;
/// # async fn example(db: Arc<Database>) -> Result<(), DbError> {
/// let tx = bm25_service::start_rebuilt(db)?;
/// // Now you can issue search commands immediately without having to index first.
/// # Ok(())
/// # }
/// ```
pub fn start_rebuilt(db: Arc<Database>) -> Result<mpsc::Sender<Bm25Cmd>, DbError> {
    let (tx, mut rx) = mpsc::channel::<Bm25Cmd>(128);
    let indexer = Bm25Indexer::rebuild_from_db(db.as_ref())?;

    tokio::spawn(async move {
        let mut indexer = indexer;
        let mut status = {
            let docs = indexer.doc_count();
            if docs == 0 { Bm25Status::Empty } else { Bm25Status::Ready { docs } }
        };
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Bm25Cmd::IndexBatch { docs } => {
                    tracing::debug!("BM25 IndexBatch: {} docs", docs.len());
                    match indexer.upsert_batch_with_cozo(db.as_ref(), docs.into_iter()) {
                        Ok(_n) => {
                            let docs = indexer.doc_count();
                            status = if docs == 0 {
                                Bm25Status::Empty
                            } else {
                                Bm25Status::Ready { docs }
                            };
                        }
                        Err(e) => {
                            tracing::error!("Error upserting batch with cozo: {e}");
                            status = Bm25Status::Error(e.to_string());
                        }
                    }
                }
                Bm25Cmd::Remove { ids } => {
                    tracing::debug!("BM25 Remove: {} docs", ids.len());
                    for id in ids {
                        indexer.remove(&id);
                    }
                    let docs = indexer.doc_count();
                    status = if docs == 0 {
                        Bm25Status::Empty
                    } else {
                        Bm25Status::Ready { docs }
                    };
                }
                Bm25Cmd::Rebuild => {
                    tracing::info!("BM25 Rebuild: starting rebuild from database");
                    // status = Bm25Status::Building;
                    match Bm25Indexer::rebuild_from_db(db.as_ref()) {
                        Ok(new_indexer) => {
                            let docs = new_indexer.doc_count();
                            indexer = new_indexer;
                            status = if docs == 0 {
                                Bm25Status::Empty
                            } else {
                                Bm25Status::Ready { docs }
                            };
                            tracing::info!("BM25 Rebuild: completed successfully with {} docs", docs);
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            tracing::error!("BM25 Rebuild: failed with error: {msg}");
                            status = Bm25Status::Error(msg);
                        }
                    }
                }
                Bm25Cmd::FinalizeSeed { resp } => {
                    let avgdl = indexer.compute_avgdl_from_staged();
                    let staged = indexer.drain_staged_meta();
                    tracing::info!(
                        "BM25 FinalizeSeed: {} docs staged, computed avgdl={}",
                        staged.len(),
                        avgdl
                    );
                    let _ = resp.send(Ok(()));
                }
                Bm25Cmd::Search { query, top_k, resp } => {
                    tracing::debug!("query: {query}, top_k: {top_k}, resp: {resp:?}");
                    let scored = indexer.search(&query, top_k);
                    tracing::debug!("scored: {scored:?}");
                    let results: Vec<(uuid::Uuid, f32)> =
                        scored.into_iter().map(|d| (d.id, d.score)).collect();
                    if resp.send(results).is_err() {
                        tracing::warn!("BM25 search response receiver dropped before sending results");
                    }
                }
                Bm25Cmd::Status { resp } => {
                    let _ = resp.send(Ok(status.clone()));
                }
                Bm25Cmd::Save { path, resp } => {
                    let docs = indexer.doc_count();
                    let content = format!(r#"{{"version":"{}","docs":{}}}"#, super::TOKENIZER_VERSION, docs);
                    let result = (|| -> Result<(), std::io::Error> {
                        if let Some(dir) = path.parent() {
                            if !dir.as_os_str().is_empty() {
                                std::fs::create_dir_all(dir)?;
                            }
                        }
                        std::fs::write(&path, content)?;
                        Ok(())
                    })().map_err(|e| DbError::Cozo(format!("bm25 save error: {}", e)));
                    let _ = resp.send(result);
                }
                Bm25Cmd::Load { path, resp } => {
                    tracing::info!("BM25 Load requested from {:?}", path);
                    // Placeholder implementation: perform a rebuild to ensure a ready index.
                    // status = Bm25Status::Building;
                    let res = match Bm25Indexer::rebuild_from_db(db.as_ref()) {
                        Ok(new_indexer) => {
                            let docs = new_indexer.doc_count();
                            indexer = new_indexer;
                            status = if docs == 0 {
                                Bm25Status::Empty
                            } else {
                                Bm25Status::Ready { docs }
                            };
                            Ok(())
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            status = Bm25Status::Error(msg.clone());
                            Err(DbError::Cozo(format!("bm25 load error: {}", msg)))
                        }
                    };
                    let _ = resp.send(res);
                }
            }
        }
        tracing::info!("BM25 service actor loop ended");
    });

    Ok(tx)
}
