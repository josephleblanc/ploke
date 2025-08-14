use std::collections::HashMap;

use super::{Bm25Indexer, DocMeta};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Debug)]
pub enum Bm25Cmd {
    /// Index a batch of (Uuid, DocMeta) documents with a tokenizer version tag.
    IndexBatch { docs: Vec<(Uuid, DocMeta)> },
    /// Remove documents from the sparse index by id.
    Remove { ids: Vec<Uuid> },
    /// Rebuild the index from source of truth (placeholder; no-op for now).
    Rebuild,
    /// Finalize the seed/build phase; commit metadata and return ack.
    FinalizeSeed { resp: oneshot::Sender<Result<(), String>> },
    /// Search the index and respond via oneshot with (id, score) pairs.
    Search { query: String, top_k: usize, resp: oneshot::Sender<Vec<(Uuid, f32)>> },
}

/// Start the BM25 actor with a given avgdl parameter.
/// Returns an mpsc::Sender<Bm25Cmd> handle for issuing commands.
pub fn start(avgdl: f32) -> mpsc::Sender<Bm25Cmd> {
    // AI: Change this function to accept an Option<f32>, and propogate those changes into the `bm25_index/mod.rs` file. AI!
    let (tx, mut rx) = mpsc::channel::<Bm25Cmd>(128);
    let mut indexer = Bm25Indexer::new(avgdl);

    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Bm25Cmd::IndexBatch { docs } => {
                    tracing::debug!(
                        "BM25 IndexBatch: {} docs",
                        docs.len(),
                    );
                    indexer.extend_staged(docs.into_iter());
                }
                Bm25Cmd::Remove { ids } => {
                    tracing::debug!("BM25 Remove: {} docs", ids.len());
                    for id in ids {
                        indexer.remove(&id);
                    }
                }
                Bm25Cmd::Rebuild => {
                    tracing::warn!("BM25 Rebuild requested but not implemented yet; ignoring");
                    // Placeholder: real rebuild will stream docs, compute avgdl, and re-index.
                }
                Bm25Cmd::FinalizeSeed { resp } => {
                    // Compute avgdl from staged metadata and drain it for persistence.
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
                    let scored = indexer.search(&query, top_k);
                    let results: Vec<(Uuid, f32)> =
                        scored.into_iter().map(|d| (d.id, d.score)).collect();
                    let _ = resp.send(results);
                }
            }
        }
        tracing::info!("BM25 service actor loop ended");
    });

    tx
}

/// Convenience starter with a reasonable default avgdl.
pub fn start_default() -> mpsc::Sender<Bm25Cmd> {
    start(10.0)
}
