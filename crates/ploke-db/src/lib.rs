//! High-performance text retrieval from code graph database

mod database;
mod error;
mod query;
mod result;
mod span;

pub use database::Database;
pub use error::DbError;
pub use query::{ QueryBuilder, builder::NodeType, builder::FieldValue };
pub use result::{CodeSnippet, QueryResult, ResultFormatter};
pub use span::{CodeLocation, SpanChange, SpanTracker};
