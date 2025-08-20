#![allow(missing_docs)]
//! Error types for ploke-rag.
//!
//! [`RagError`] captures channel failures, database/actor errors, embedding failures, and
//! search state violations. A conversion into the workspace-wide error type is provided so
//! higher layers can uniformly handle failures.
use ploke_db::DbError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RagError {
    #[error("Database error: {0}")]
    Db(#[from] DbError),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Embedding error: {0}")]
    Embed(String),

    #[error("Search error: {0}")]
    Search(String),
}

impl From<RagError> for ploke_error::Error {
    fn from(value: RagError) -> ploke_error::Error {
        match value {
            RagError::Db(db_err) => {
                ploke_error::Error::Internal(ploke_error::internal::InternalError::CompilerError(
                    format!("DB error: {}", db_err),
                ))
            }
            RagError::Channel(msg) => {
                ploke_error::Error::Internal(ploke_error::internal::InternalError::CompilerError(
                    format!("Channel communication error: {}", msg),
                ))
            }
            RagError::Embed(msg) => {
                ploke_error::Error::Internal(ploke_error::internal::InternalError::NotImplemented(
                    format!("Embedding error: {}", msg),
                ))
            }
            RagError::Search(msg) => {
                ploke_error::Error::Internal(ploke_error::internal::InternalError::NotImplemented(
                    format!("Search error: {}", msg),
                ))
            }
        }
    }
}
