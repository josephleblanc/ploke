//! High-performance text retrieval from code graph database
#![allow(unused_variables, unused_imports, dead_code)]

mod database;
mod error;
mod query;
mod result;
mod span;
mod index;

pub use database::{to_usize, Database, TypedEmbedData};
pub use error::DbError;
pub use query::{
    builder::FieldValue,
    builder::NodeType,
    callbacks::{Callback, CallbackManager},
    QueryBuilder,
};
pub use index::{ search_similar, create_index };
pub use result::{CodeSnippet, QueryResult, ResultFormatter};
pub use span::{CodeLocation, SpanChange, SpanTracker};
