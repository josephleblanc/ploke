//! High-performance text retrieval from code graph database

mod database;
mod error;
mod query;
mod result;
mod span;

pub use database::Database;
pub use error::Error;
pub use query::QueryBuilder;
pub use result::{CodeSnippet, QueryResult, ResultFormatter};
pub use span::{CodeLocation, SpanChange, SpanTracker};
