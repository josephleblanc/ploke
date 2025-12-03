/*!
Phase 5 — Minimal Write Path (splice + atomic rename)

This module implements the initial version of the write path:
- UTF-8 read of the current file content
- Compute actual TrackingHash and verify against the expected
- Byte-range splice with UTF-8 boundary validation
- Atomic temp-write + fsync + rename (+ best-effort parent fsync)
- Return new TrackingHash

Notes:
- Per-file locking and watcher-origin propagation will be added in subsequent steps.
- Cross-crate types (WriteSnippetData, WriteResult) are expected to live in ploke-core later.
*/

use super::*;
use crate::actor::read_and_compute_hash;
use crate::path_policy::{
    normalize_against_roots, normalize_against_roots_with_policy, SymlinkPolicy,
};
use dashmap::DashMap;
use lazy_static::lazy_static;
use mpatch::ApplyOptions;
use ploke_core::file_hash::{hash_file_blake3_bounded, LargeFilePolicy};
use ploke_core::{WriteResult, WriteSnippetData};
use serde::{Deserialize, Serialize};
use tracing::debug;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

// `WriteSnippetData` and `WriteResult` moved to ploke-core
// If changes are needed, share details in implementation-log and USER will propogate them to
// ploke-core (this is to save context while developing, as ploke-core is a large mono-file lib.rs)
//
// #[derive(Debug, Clone)]
// pub struct WriteSnippetData {
//     pub id: uuid::Uuid,
//     pub name: String,
//     pub file_path: PathBuf,
//     pub expected_file_hash: TrackingHash,
//     pub start_byte: usize,
//     pub end_byte: usize,
//     pub replacement: String,
//     pub namespace: uuid::Uuid,
// }
//
// #[derive(Debug, Clone)]
// pub struct WriteResult {
//     pub new_file_hash: TrackingHash,
// }
//
// impl WriteResult {
//     pub fn new(new_file_hash: TrackingHash) -> Self {
//         Self { new_file_hash }
//     }
// }

lazy_static! {
    static ref FILE_LOCKS: DashMap<PathBuf, Arc<Mutex<()>>> = DashMap::new();
}

fn get_file_lock(path: &Path) -> Arc<Mutex<()>> {
    FILE_LOCKS
        .entry(path.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Placeholder structures to be replaced by shared types in ploke-core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsWriteSnippetData {
    pub id: uuid::Uuid,
    pub file_path: PathBuf,
    pub expected_file_hash: Option<FileHash>,
    pub namespace: uuid::Uuid,
    pub diff: Diff,
    pub options: PatchApplyOptions,
    pub large_file_policy: LargeFilePolicy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsWriteResult {
    pub new_file_hash: FileHash,
}

impl NsWriteResult {
    fn new(new_file_hash: FileHash) -> Self {
        Self { new_file_hash }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PatchApplyOptions {
    pub dry_run: bool,
    pub fuzz_factor: f32,
}

impl Default for PatchApplyOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            fuzz_factor: 0.7,
        }
    }
}

impl From<PatchApplyOptions> for ApplyOptions {
    fn from(value: PatchApplyOptions) -> Self {
        Self {
            dry_run: value.dry_run,
            fuzz_factor: value.fuzz_factor,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diff(String);

impl AsRef<str> for Diff {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Diff {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[tracing::instrument]
async fn process_one_write_ns(
    req: NsWriteSnippetData,
    roots: Option<Arc<Vec<PathBuf>>>,
    symlink_policy: Option<SymlinkPolicy>,
    max_bytes: u64,
) -> Result<NsWriteResult, PlokeError> {
    let file_path = if let Some(roots) = roots.as_ref() {
        let roots_ref: &[PathBuf] = roots.as_ref();
        if let Some(policy) = symlink_policy {
            normalize_against_roots_with_policy(&req.file_path, roots_ref, policy)?
        } else {
            normalize_against_roots(&req.file_path, roots_ref)?
        }
    } else {
        if !req.file_path.is_absolute() {
            return Err(PlokeError::from(IoError::FileOperation {
                operation: "write",
                path: req.file_path.clone(),
                kind: std::io::ErrorKind::InvalidInput,
                source: Arc::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "path must be absolute",
                )),
            }));
        }
        req.file_path.clone()
    };

    // TODO: finish setting up with locks correctly
    // let lock = get_file_lock(&file_path);
    // let _write_lock_guard = lock.lock().await;

    let large_file_policy = req.large_file_policy;
    let hashed_result = read_and_compute_hash(&file_path, large_file_policy, max_bytes)?;
    let new_hash = hashed_result?;
    debug!("new_hash calculated: {:?}", new_hash);

    let NsWriteSnippetData {
        id,
        file_path,
        expected_file_hash,
        namespace,
        diff,
        options,
        ..
    } = req;
    if req.expected_file_hash.is_some_and(|h| h != new_hash) {
        return Err(PlokeError::from(IoError::NsContentMismatch {
            id,
            file_path,
            expected_file_hash,
            namespace,
            diff,
            options,
        }));
    };
    let parsed_patch = mpatch::parse_single_patch(diff.as_ref())
        .map_err(|e| { 
            tracing::error!("Error in parse_single_patch: {}", e.to_string());
            PlokeError::from(IoError::NsPatchError(e.to_string())) 
        })?;
    let patch_options = mpatch::ApplyOptions {
        dry_run: options.dry_run,
        fuzz_factor: options.fuzz_factor,
    };

    // TODO: Add a similar lock/read/write with atomic edits, similar to below `process_one_write`
    mpatch::apply_patch_to_file(&parsed_patch, &file_path, patch_options)
        .map_err(|e| { 
            tracing::error!("Error in parse_single_patch: {}", e.to_string());
            PlokeError::from(IoError::NsPatchError(e.to_string())) 
        })?;

    Ok(NsWriteResult::new(new_hash))
}

async fn process_one_write(
    req: WriteSnippetData,
    roots: Option<Arc<Vec<PathBuf>>>,
    symlink_policy: Option<SymlinkPolicy>,
) -> Result<WriteResult, IoError> {
    use crate::read::{parse_tokens_from_str, read_file_to_string_abs};

    // 0) Normalize/validate path against configured roots and policy (writes)
    let file_path = if let Some(roots) = roots.as_ref() {
        let roots_ref: &[PathBuf] = roots.as_ref();
        if let Some(policy) = symlink_policy {
            normalize_against_roots_with_policy(&req.file_path, roots_ref, policy)?
        } else {
            normalize_against_roots(&req.file_path, roots_ref)?
        }
    } else {
        if !req.file_path.is_absolute() {
            return Err(IoError::FileOperation {
                operation: "write",
                path: req.file_path.clone(),
                kind: std::io::ErrorKind::InvalidInput,
                source: Arc::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "path must be absolute",
                )),
            });
        }
        req.file_path.clone()
    };

    // Acquire per-file async lock to serialize writes to the same path.
    // Keep the Arc<Mutex<()>> binding alive as long as the guard to satisfy borrow checker.
    let lock = get_file_lock(&file_path);
    let _write_lock_guard = lock.lock().await;

    // 1) Read current content (absolute-path enforced by helper)
    let content = read_file_to_string_abs(&file_path).await?;

    // 2) Verify expected file hash
    let actual_hash = {
        let tokens = parse_tokens_from_str(&content, &file_path)?;
        TrackingHash::generate(req.namespace, &file_path, &tokens)
    };
    if actual_hash != req.expected_file_hash {
        return Err(IoError::ContentMismatch {
            path: file_path.clone(),
            name: Some(req.name.clone()),
            id: Some(req.id),
            file_tracking_hash: req.expected_file_hash.0,
            namespace: req.namespace,
        });
    }

    // 3) Validate range and UTF-8 boundaries
    let len = content.len();
    if req.start_byte > req.end_byte || req.end_byte > len {
        return Err(IoError::OutOfRange {
            path: file_path.clone(),
            start_byte: req.start_byte,
            end_byte: req.end_byte,
            file_len: len,
        });
    }
    if !content.is_char_boundary(req.start_byte) || !content.is_char_boundary(req.end_byte) {
        return Err(IoError::InvalidCharBoundary {
            path: file_path.clone(),
            start_byte: req.start_byte,
            end_byte: req.end_byte,
        });
    }

    // 4) Splice
    let head = &content[..req.start_byte];
    let tail = &content[req.end_byte..];
    let mut new_content = String::with_capacity(head.len() + req.replacement.len() + tail.len());
    new_content.push_str(head);
    new_content.push_str(&req.replacement);
    new_content.push_str(tail);

    // 5) Compute new hash from new content
    let new_hash = {
        let new_tokens = parse_tokens_from_str(&new_content, &file_path)?;
        TrackingHash::generate(req.namespace, &file_path, &new_tokens)
    };

    // 6) Atomic write (temp file in same directory)
    let parent = file_path
        .parent()
        .ok_or_else(|| IoError::FileOperation {
            operation: "write",
            path: req.file_path.clone(),
            kind: std::io::ErrorKind::InvalidInput,
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "file has no parent directory",
            )),
        })?
        .to_path_buf();

    let tmp_path = parent.join(format!(".plokeio-{}.tmp", uuid::Uuid::new_v4()));

    // Create and write
    {
        let mut f =
            tokio::fs::File::create(&tmp_path)
                .await
                .map_err(|e| IoError::FileOperation {
                    operation: "write",
                    path: tmp_path.clone(),
                    kind: e.kind(),
                    source: Arc::new(e),
                })?;
        f.write_all(new_content.as_bytes())
            .await
            .map_err(|e| IoError::FileOperation {
                operation: "write",
                path: tmp_path.clone(),
                kind: e.kind(),
                source: Arc::new(e),
            })?;
        f.sync_all().await.map_err(|e| IoError::FileOperation {
            operation: "sync",
            path: tmp_path.clone(),
            kind: e.kind(),
            source: Arc::new(e),
        })?;
    }

    // Rename over original
    tokio::fs::rename(&tmp_path, &file_path)
        .await
        .map_err(|e| IoError::FileOperation {
            operation: "rename",
            path: file_path.clone(),
            kind: e.kind(),
            source: Arc::new(e),
        })?;

    // Best-effort fsync parent directory to ensure durability
    {
        let parent_clone = parent.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(dir) = std::fs::File::open(&parent_clone) {
                match dir.sync_all() {
                    Ok(()) => {tracing::trace!(target: "file-edit", "Write successful for file: {}",
                        file_path.to_string_lossy())},
                    Err(e) => {tracing::error!(target: "file-edit", "Write failed for file {file_error} with error: {err}",
                        err = e.to_string(),
                        file_error = file_path.to_string_lossy(),
                    )},
                }
            }
        })
        .await;
    }

    Ok(WriteResult::new(new_hash))
}

/// Batch write entrypoint used by the IoManager.
pub(crate) async fn write_snippets_batch(
    requests: Vec<WriteSnippetData>,
    roots: Option<Arc<Vec<PathBuf>>>,
    symlink_policy: Option<SymlinkPolicy>,
    max_bytes: u64,
) -> Vec<Result<WriteResult, PlokeError>> {
    let mut out = Vec::with_capacity(requests.len());
    for req in requests {
        let res = process_one_write(req, roots.clone(), symlink_policy)
            .await
            .map_err(ploke_error::Error::from);
        out.push(res);
    }
    out
}

pub(crate) async fn write_snippets_batch_ns(
    requests: Vec<NsWriteSnippetData>,
    roots: Option<Arc<Vec<PathBuf>>>,
    symlink_policy: Option<SymlinkPolicy>,
    max_bytes: u64,
) -> Vec<Result<NsWriteResult, PlokeError>> {
    let mut out = Vec::with_capacity(requests.len());
    for req in requests {
        let res = process_one_write_ns(
            req, 
            roots.clone(), 
            symlink_policy,
            max_bytes
        ).await;
        out.push(res);
    }
    out
}
