//! Error types for ploke-db

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum DbError {
    #[error("Database error: {0}")]
    Cozo(String),

    #[error("Query execution error: {0}")]
    QueryExecution(String),

    #[error("Invalid query construction: {0}")]
    QueryConstruction(String),
}

#[derive(Error, Debug)]
pub enum DbWarning {
    #[error("Invalid query build attempt: {0}")]
    QueryBuild(String),
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
