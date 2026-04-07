//! High-performance text retrieval from code graph database
#![allow(unused_variables, unused_imports, dead_code)]

extern crate self as ploke_db;

pub mod bm25_index;
mod database;
mod error;
pub mod get_by_id;
pub mod helpers;
mod index;
pub mod observability;
mod query;
mod result;
pub(crate) mod utils;

pub mod tool_query;

pub mod multi_embedding;

pub use database::RestoredEmbeddingSet;
pub use database::{
    CrateContextRow, Database, NamespaceExportArtifact, NamespaceImportConflictReport,
    NamespaceImportError, NamespaceImportResult, NamespaceRemovalResult, QueryContext,
    TypedEmbedData, to_usize, to_uuid,
};
pub use error::DbError;
pub use index::hnsw::{
    EmbedDataVerbose, SimilarArgs, create_index, create_index_for_set, create_index_primary,
    create_index_primary_with_index, create_index_warn, hnsw_all_types, hnsw_of_type,
    replace_index_warn, search_similar, search_similar_args,
};
pub use observability::{
    CodeEditProposal, ConversationTurn, ObservabilityStore, ToolCallDone, ToolCallReq, ToolStatus,
    Validity,
};
pub use ploke_error::PrettyDebug;
pub use query::{
    QueryBuilder,
    builder::FieldValue,
    builder::NodeType,
    callbacks::{Callback, CallbackManager},
};

pub use result::typed_rows;

pub use result::{CodeSnippet, QueryResult, ResultFormatter};
