//! Error types for ploke-db

//! Error types for ploke-db

use std::panic::Location;

use ploke_core::embeddings::{EmbRelName, HnswRelName};
use ploke_error::PrettyDebug;
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum DbError {
    #[error("Conversion Error: {0}")]
    UuidConv(#[from] uuid::Error),

    #[error("Vector Conversion Error")]
    VectorConv,

    #[error("Database error: {0}")]
    Cozo(String),

    #[error("Cozo query `{query_name}` failed at {file}:{line}:{column}: {message}")]
    CozoQuery {
        query_name: &'static str,
        message: String,
        file: &'static str,
        line: u32,
        column: u32,
    },

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

    #[error(
        "Experimental embedding script '{action}' failed for relation 'embedding_set': {details}"
    )]
    EmbeddingSetScriptFailure {
        action: &'static str,
        details: String,
    },

    #[error("Experimental embedding script '{action}' failed for relation {relation}: {details}")]
    HnswEmbeddingScriptFailure {
        action: &'static str,
        relation: HnswRelName,
        details: String,
    },

    #[error("Warning: Empty list of vectors passed as updates to database")]
    EmbeddingUpdateEmpty,
}

impl DbError {
    pub fn cozo_with_callsite(
        query_name: &'static str,
        message: String,
        caller: &'static Location<'static>,
    ) -> Self {
        Self::CozoQuery {
            query_name,
            message,
            file: caller.file(),
            line: caller.line(),
            column: caller.column(),
        }
    }

    /// Structured view for logging/pretty-printing the CozoQuery variant.
    pub fn cozo_query_fields(&self) -> Option<CozoQueryFields<'_>> {
        match self {
            DbError::CozoQuery {
                query_name,
                message,
                file,
                line,
                column,
            } => Some(CozoQueryFields {
                query_name,
                message,
                file,
                line: *line,
                column: *column,
            }),
            _ => None,
        }
    }
}

#[derive(Error, Debug)]
pub enum DbWarning {
    #[error("Invalid query build attempt: {0}")]
    QueryBuild(String),
}

#[derive(Debug, Serialize)]
pub struct CozoQueryFields<'a> {
    pub query_name: &'static str,
    pub message: &'a str,
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
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

impl PrettyDebug for DbError {
    type Fields<'a> = CozoQueryFields<'a>;

    fn fields(&self) -> Option<Self::Fields<'_>> {
        self.cozo_query_fields()
    }
}
