#![allow(unused_variables, unused_imports, dead_code)]
use ploke_db::{Database, DbError};
use ploke_embed::indexer::IndexerTask;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RagError {
    #[error("Database error: {0}")]
    Db(#[from] DbError),
    // #[error("Embedding error: {0}")]
    // Embed(#[from] EmbeddingError),
}

pub struct RagService {
    db: Database,
    embedder: IndexerTask
}

impl RagService {
    pub fn new(db: Database, embedder: IndexerTask) -> Self {
        Self { db, embedder }
    }

    pub async fn query(&self, _question: &str) -> Result<Vec<ploke_db::CodeSnippet>, RagError> {
        // 1. Embed the question
        // 2. Query the database
        // 3. Return the results
        Ok(vec![])
    }
}
