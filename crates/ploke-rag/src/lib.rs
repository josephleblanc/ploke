#![allow(unused_variables, unused_imports, dead_code)]
use std::sync::Arc;

use ploke_db::{bm25_index::bm25_service::Bm25Cmd, Database, DbError};
use ploke_embed::indexer::IndexerTask;
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Error, Debug)]
pub enum RagError {
    #[error("Database error: {0}")]
    Db(#[from] DbError),
    // #[error("Embedding error: {0}")]
    // Embed(#[from] EmbeddingError),
}

pub struct RagService {
    db: Arc< Database >,
    dense_embedder: IndexerTask,
    bm_embedder: mpsc::Sender<Bm25Cmd>
}

impl RagService {
    pub fn new(db: Arc< Database >, dense_embedder: IndexerTask) -> Self {
        Self { db, dense_embedder: todo!(), bm_embedder: todo!() }
    }
}
