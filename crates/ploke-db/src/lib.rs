//! High-performance text retrieval from code graph database
#![allow(unused_variables, unused_imports, dead_code)]

pub mod bm25_index;
mod database;
mod error;
pub mod get_by_id;
pub mod helpers;
mod index;
pub mod observability;
mod query;
mod result;
mod span;
pub(crate) mod utils;

#[cfg(feature = "multi_embedding_db")]
pub mod multi_embedding;

pub use database::{to_usize, to_uuid, Database, TypedEmbedData};
pub use error::DbError;
pub use index::hnsw::{
    create_index, create_index_primary, create_index_primary_with_index, create_index_warn,
    hnsw_all_types, hnsw_of_type, replace_index_warn, search_similar, search_similar_args,
    EmbedDataVerbose, SimilarArgs,
};
pub use observability::{
    CodeEditProposal, ConversationTurn, ObservabilityStore, ToolCallDone, ToolCallReq, ToolStatus,
    Validity,
};
pub use query::{
    builder::FieldValue,
    builder::NodeType,
    callbacks::{Callback, CallbackManager},
    QueryBuilder,
};
pub use result::{CodeSnippet, QueryResult, ResultFormatter};
pub use span::{CodeLocation, SpanChange, SpanTracker};
