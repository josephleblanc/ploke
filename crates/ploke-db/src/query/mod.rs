//! Query building and execution interface
//!
//! Main entry point for constructing and executing queries against the code graph database.
//! Organized into submodules for different query types and operations.

pub mod builder;
pub mod filters;
pub mod joins;
pub mod semantic;

pub use builder::QueryBuilder;

/// Result of a database query
#[derive(Debug)]
pub struct QueryResult {
    pub rows: Vec<Vec<cozo::DataValue>>,
    pub headers: Vec<String>,
}

impl From<cozo::NamedRows> for QueryResult {
    fn from(named_rows: cozo::NamedRows) -> Self {
        Self {
            rows: named_rows.rows,
            headers: named_rows.headers,
        }
    }
}
