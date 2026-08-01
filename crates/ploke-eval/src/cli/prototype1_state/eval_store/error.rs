use std::path::PathBuf;

use ploke_db::DbError;
use thiserror::Error;

use super::super::journal::PrototypeJournalError;
use super::{api::EvalStorageMode, evidence::ParentStartedReceipt};

#[derive(Debug, Error)]
pub(crate) enum EvalStoreError {
    #[error("failed to append {phase} evidence to transition journal: {source}")]
    Journal {
        phase: &'static str,
        source: PrototypeJournalError,
    },
    #[error("eval-store db operation {phase} failed: {source}")]
    Db {
        phase: &'static str,
        source: DbError,
    },
    #[error("eval-store db setup {phase} failed: {detail}")]
    DbSetup { phase: &'static str, detail: String },
    #[error("eval-store filesystem operation {phase} failed for {path:?}: {source}")]
    Io {
        phase: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("eval-store validation failed for {field}: {detail}")]
    Validation { field: &'static str, detail: String },
    #[error(
        "semantic hash mismatch for eval transition event '{event_id}': existing {existing_semantic_hash}, attempted {attempted_semantic_hash}"
    )]
    SemanticConflict {
        event_id: String,
        existing_semantic_hash: String,
        attempted_semantic_hash: String,
    },
    #[error(
        "eval-store {backend} DB write failed after filesystem parent-start append: journal_path={journal_path}, parent_started_source_event_index={parent_started_source_event_index}, resource_source_event_index={resource_source_event_index}, parent_started_content_sha256={parent_started_content_sha256}, resource_content_sha256={resource_content_sha256}, expected_semantic_hash={expected_semantic_hash:?}, db_error_or_mismatch={db_error_or_mismatch}, suggested_recovery={suggested_recovery}"
    )]
    PostFsDb {
        backend: &'static str,
        journal_path: String,
        parent_started_source_event_index: usize,
        resource_source_event_index: usize,
        parent_started_content_sha256: String,
        resource_content_sha256: String,
        expected_semantic_hash: Option<String>,
        db_error_or_mismatch: String,
        suggested_recovery: &'static str,
    },
}

impl EvalStoreError {
    pub(super) fn post_fs_db(
        mode: EvalStorageMode,
        receipt: &ParentStartedReceipt,
        expected_semantic_hash: Option<String>,
        db_error_or_mismatch: String,
    ) -> Self {
        Self::PostFsDb {
            backend: mode.as_str(),
            journal_path: receipt.parent.path.display().to_string(),
            parent_started_source_event_index: receipt.parent.source_event_index,
            resource_source_event_index: receipt.resource.source_event_index,
            parent_started_content_sha256: receipt.parent.content_sha256.clone(),
            resource_content_sha256: receipt.resource.content_sha256.clone(),
            expected_semantic_hash,
            db_error_or_mismatch,
            suggested_recovery: "re-run deterministic import for these source indices",
        }
    }
}
