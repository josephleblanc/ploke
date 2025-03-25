//! Error types for ploke-db

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Cozo(#[from] cozo::Error),

    #[error("Query execution error: {0}")]
    QueryExecution(String),

    #[error("Invalid query construction: {0}")]
    QueryConstruction(String),
}
