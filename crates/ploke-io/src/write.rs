/*!
Phase 5 — Write Path Scaffolding (internal stubs)

This module provides initial scaffolding for the future write path:
- Per-file locking
- Splice-in-memory with UTF-8 guarantees
- Atomic temp-write + fsync + rename + parent fsync
- New tracking hash computation and watcher origin propagation

Note:
- Cross-crate types (WriteSnippetData, WriteResult) are expected to live in ploke-core.
- Until then, we keep local placeholders behind the crate boundary.
*/

use super::*;
use std::path::PathBuf;

/// Placeholder structures to be replaced by shared types in ploke-core.
#[derive(Debug, Clone)]
pub(crate) struct WriteSnippetData {
    pub file_path: PathBuf,
    pub expected_file_hash: TrackingHash,
    pub start_byte: usize,
    pub end_byte: usize,
    pub replacement: String,
    pub namespace: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub(crate) struct WriteResult {
    pub new_file_hash: TrackingHash,
}

impl WriteResult {
    #[allow(dead_code)]
    pub fn new(new_file_hash: TrackingHash) -> Self {
        Self { new_file_hash }
    }
}

/// Stub entrypoint for a future batch write API.
/// Not wired into IoManager yet; left as scaffolding.
#[allow(dead_code)]
pub(crate) async fn write_snippets_batch(
    _requests: Vec<WriteSnippetData>,
) -> Vec<Result<WriteResult, PlokeError>> {
    // TODO(Phase 5): implement per-file locking, splice, atomic rename workflow
    Vec::new()
}
