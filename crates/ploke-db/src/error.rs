//! Error types for ploke-db

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Cozo(String),

    #[error("Query execution error: {0}")]
    QueryExecution(String),

    #[error("Invalid query construction: {0}")]
    QueryConstruction(String),
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

impl From<crate::Error> for ploke_error::Error {
    fn from(value: crate::Error) -> Self {
        ploke_error::WarningError::PlokeDb(value.to_string()).into()
    }
}
