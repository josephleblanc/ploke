#![allow(unused_variables, unused_imports, dead_code)]
use std::sync::Arc;

use ploke_db::{
    bm25_index::bm25_service,
    bm25_index::bm25_service::Bm25Cmd,
    Database, DbError,
};
use ploke_embed::indexer::IndexerTask;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum RagError {
    #[error("Database error: {0}")]
    Db(#[from] DbError),

    #[error("Channel error: {0}")]
    Channel(String),
    // #[error("Embedding error: {0}")]
    // Embed(#[from] EmbeddingError),
}

/// RAG orchestration service.
/// Holds handles to the database, dense embedder, and BM25 service actor.
#[derive(Debug)]
pub struct RagService {
    db: Arc<Database>,
    dense_embedder: Arc<IndexerTask>,
    bm_embedder: mpsc::Sender<Bm25Cmd>,
}

impl RagService {
    /// Construct a new RAG service, starting the BM25 service actor.
    pub fn new(db: Arc<Database>, dense_embedder: Arc<IndexerTask>) -> Result<Self, RagError> {
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Ok(Self { db, dense_embedder, bm_embedder })
    }

    /// Execute a BM25 search against the in-memory sparse index.
    /// Returns a Vec of (document_id, score) pairs sorted by relevance.
    pub async fn search_bm25(&self, query: &str, top_k: usize) -> Result<Vec<(Uuid, f32)>, RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Search {
                query: query.to_string(),
                top_k,
                resp: tx,
            })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 search command: {}", e)))?;

        rx.await
            .map_err(|e| RagError::Channel(format!("BM25 search response channel closed: {}", e)))
    }
}
