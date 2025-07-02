use ploke_error::{Error, InternalError};

#[derive(thiserror::Error, Debug, Clone)]
pub enum BatchError {
    #[error("Snippet fetch failed: {0}")]
    SnippetFetch(#[source] ploke_error::Error),

    #[error("Embedding computation failed: {0}")]
    Embedding(#[source] ploke_error::Error),

    #[error("Database update failed: {0}")]
    Database(#[source] ploke_db::DbError),

    #[error("Invalid vector dimension: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Generic catch-all error: {0}")]
    Generic(String),
}
// This is safe because ploke_embed already depends on ploke_error
impl From<BatchError> for ploke_error::Error {
    fn from(e: BatchError) -> Self {
        ploke_error::Error::Internal(InternalError::embedder_error(e))
    }
}
