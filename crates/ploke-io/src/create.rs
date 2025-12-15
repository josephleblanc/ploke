/*!
Create-file path (atomic, root-enforced)

Implements an IO-level API to create (or overwrite) a file with full safety:
- Enforces absolute path and root containment (via parent directory normalization).
- Supports creating missing parent directories when `create_parents` is true.
- Restricts to Rust source files (`.rs`).
- Writes content atomically: temp file in parent dir + fsync + rename.
- Computes and returns a `TrackingHash` based on the final content.
*/

use super::*;
use crate::path_policy::{normalize_against_roots_with_policy, path_within_roots, SymlinkPolicy};
use ploke_core::{CreateFileData, CreateFileResult, OnExists};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

fn ensure_rust_extension(path: &Path) -> Result<(), IoError> {
    if path.extension().and_then(|e| e.to_str()) == Some("rs") {
        Ok(())
    } else {
        Err(IoError::FileOperation {
            operation: "create",
            path: path.to_path_buf(),
            kind: std::io::ErrorKind::InvalidInput,
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "only .rs files are supported for creation",
            )),
        })
    }
}

fn normalize_parent_for_create(
    file_path: &Path,
    roots: Option<Arc<Vec<PathBuf>>>,
    symlink_policy: Option<SymlinkPolicy>,
) -> Result<PathBuf, IoError> {
    if !file_path.is_absolute() {
        return Err(IoError::FileOperation {
            operation: "create",
            path: file_path.to_path_buf(),
            kind: std::io::ErrorKind::InvalidInput,
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path must be absolute",
            )),
        });
    }
    let parent = file_path.parent().ok_or_else(|| IoError::FileOperation {
        operation: "create",
        path: file_path.to_path_buf(),
        kind: std::io::ErrorKind::InvalidInput,
        source: Arc::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "file has no parent directory",
        )),
    })?;

    // For creation, the file itself may not exist. Normalize and enforce roots on the parent dir.
    let parent_canon = if let Some(roots) = roots.as_ref() {
        // Use symlink policy when provided
        match symlink_policy {
            Some(policy) => normalize_against_roots_with_policy(parent, roots.as_ref(), policy),
            None => normalize_against_roots_with_policy(
                parent,
                roots.as_ref(),
                SymlinkPolicy::DenyCrossRoot,
            ),
        }
    } else {
        // No configured roots; accept absolute parent as-is (best effort canonicalization not needed)
        Ok(parent.to_path_buf())
    }?;

    Ok(parent_canon)
}

pub(crate) async fn create_file(
    req: CreateFileData,
    roots: Option<Arc<Vec<PathBuf>>>,
    symlink_policy: Option<SymlinkPolicy>,
) -> Result<CreateFileResult, IoError> {
    use crate::read::parse_tokens_from_str;

    ensure_rust_extension(&req.file_path)?;
    let parent_canon = normalize_parent_for_create(&req.file_path, roots.clone(), symlink_policy)?;
    let target_path =
        parent_canon.join(
            req.file_path
                .file_name()
                .ok_or_else(|| IoError::FileOperation {
                    operation: "create",
                    path: req.file_path.clone(),
                    kind: std::io::ErrorKind::InvalidInput,
                    source: Arc::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "invalid file name",
                    )),
                })?,
        );

    // Enforce root containment on the resulting full path when roots configured
    if let Some(roots) = roots.as_ref() {
        if !path_within_roots(&target_path, roots.as_ref()) {
            return Err(IoError::FileOperation {
                operation: "create",
                path: target_path.clone(),
                kind: std::io::ErrorKind::InvalidInput,
                source: Arc::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "path outside configured roots",
                )),
            });
        }
    }

    // Parents handling
    if req.create_parents {
        if let Some(p) = target_path.parent() {
            tokio::fs::create_dir_all(p)
                .await
                .map_err(|e| IoError::FileOperation {
                    operation: "mkdirs",
                    path: p.to_path_buf(),
                    kind: e.kind(),
                    source: Arc::new(e),
                })?;
        }
    } else {
        // Ensure parent exists
        if let Some(p) = target_path.parent() {
            if tokio::fs::metadata(p).await.is_err() {
                return Err(IoError::FileOperation {
                    operation: "create",
                    path: target_path.clone(),
                    kind: std::io::ErrorKind::NotFound,
                    source: Arc::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "parent directory does not exist",
                    )),
                });
            }
        }
    }

    // Existence policy
    let exists = tokio::fs::metadata(&target_path).await.is_ok();
    if let (true, OnExists::Error) = (exists, req.on_exists) {
        return Err(IoError::FileOperation {
            operation: "create",
            path: target_path.clone(),
            kind: std::io::ErrorKind::AlreadyExists,
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "file already exists",
            )),
        });
    }

    // Compute hash from intended content
    let new_hash = {
        let tokens = parse_tokens_from_str(&req.content, &target_path)?;
        TrackingHash::generate(req.namespace, &target_path, &tokens)
    };

    // Atomic write via temp file then rename
    let tmp_path = target_path
        .parent()
        .ok_or_else(|| IoError::FileOperation {
            operation: "create",
            path: target_path.clone(),
            kind: std::io::ErrorKind::InvalidInput,
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "file has no parent directory",
            )),
        })?
        .join(format!(".plokeio-{}.tmp", uuid::Uuid::new_v4()));

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
        f.write_all(req.content.as_bytes())
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

    tokio::fs::rename(&tmp_path, &target_path)
        .await
        .map_err(|e| IoError::FileOperation {
            operation: "rename",
            path: target_path.clone(),
            kind: e.kind(),
            source: Arc::new(e),
        })?;

    // Best-effort fsync parent directory
    let parent = target_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(dir) = std::fs::File::open(&parent) {
            let _ = dir.sync_all();
        }
    })
    .await;

    Ok(CreateFileResult {
        new_file_hash: new_hash,
    })
}
