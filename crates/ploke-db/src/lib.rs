//! High-performance text retrieval from code graph database
#![allow(unused_variables, unused_imports, dead_code)]

extern crate self as ploke_db;

pub mod bm25_index;
#[cfg(feature = "call_graph")]
pub mod call_graph;
mod database;
mod error;
pub mod get_by_id;
pub mod helpers;
mod index;
pub mod observability;
pub mod proof_graph;
mod query;
mod result;
pub(crate) mod utils;

pub mod tool_query;
pub mod type_graph;

pub mod multi_embedding;

#[cfg(feature = "call_graph")]
pub use call_graph::{
    CallCallerRow, CallContextCandidate, CallContextOptions, CallContextRelation, CallContextRow,
    CallContextSeed, CallReceiver, CallRelationKind, CallResolutionKind, CallResolutionRow,
    CallSiteKind, CallSiteRow, CallStatusKind, CallTargetKind, CallTargetRow,
    call_target_endpoint_relation, valid_call_target_family,
};
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
pub use proof_graph::{
    ProofBlockerRow, ProofCheckerEdgeRow, ProofGraphContextRow, ProofGraphStore,
    ProofInvariantFinding, ProofInvariantStatus, ProofSourceProvenanceRow,
};
pub use query::{
    QueryBuilder,
    builder::FieldValue,
    builder::NodeType,
    callbacks::{Callback, CallbackManager},
};

pub use result::typed_rows;

pub use result::{CodeSnippet, QueryResult, ResultFormatter};
pub use type_graph::{
    TypeContainmentEdge, TypeContainmentKind, TypeContextCandidate, TypeContextOptions,
    TypeContextRelation, TypeContextSeed, TypeRelationKind, TypeTargetPath, TypeUseCoordinate,
    TypeUseRole, TypeUseRoot,
};
