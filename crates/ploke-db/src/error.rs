//! Error types for ploke-db

use ploke_core::embeddings::EmbRelName;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum DbError {
    #[error("Conversion Error: {0}")]
    UuidConv(#[from] uuid::Error),

    #[error("Database error: {0}")]
    Cozo(String),

    #[error("Query execution error: {0}")]
    QueryExecution(String),

    #[error("Invalid query construction: {0}")]
    QueryConstruction(String),

    #[error("Item not found")]
    NotFound,

    #[error("Error encountered for callback construction")]
    CallbackErr,

    #[error("Do not change the max of the callback")]
    CallbackSetCheck,

    #[error("Invalid lifecycle transition: {0}")]
    InvalidLifecycle(String),

    #[error("Error receiving message: {0}")]
    CrossBeamSend(String),

    #[error("Experimental embedding script '{action}' failed for relation {relation}: {details}")]
    EmbeddingScriptFailure {
        action: &'static str,
        relation: EmbRelName,
        details: String,
    },

    #[error("Warning: Empty list of vectors passed as updates to database")]
    EmbeddingUpdateEmpty,
}

#[derive(Error, Debug)]
pub enum DbWarning {
    #[error("Invalid query build attempt: {0}")]
    QueryBuild(String),
}

impl From<cozo::Error> for DbError {
    fn from(value: cozo::Error) -> Self {
        let e = value.to_string();
        tracing::trace!("Cozo Error: {}", e);
        Self::Cozo(e)
    }
}

// impl std::fmt::Display for crate::Error {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let msg = match self {
//             Self::Cozo(s) => s,
//             Self::QueryExecution(s) => s,
//             Self::QueryConstruction(s) => s,
//         };
//         write!(f, "{}", msg)
//     }
// }

// TODO: Work on the error types here to make it more clear what the difference is between warnings
// and errors that should not be recoverable, such as internal state errors.
impl From<DbError> for ploke_error::Error {
    fn from(value: DbError) -> Self {
        ploke_error::WarningError::PlokeDb(value.to_string()).into()
    }
}

impl From<DbWarning> for ploke_error::WarningError {
    fn from(value: DbWarning) -> Self {
        ploke_error::WarningError::PlokeDb(value.to_string())
    }
}
