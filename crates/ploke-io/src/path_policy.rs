use super::*;

// Target module for path policy and security

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Strict canonicalization that returns an IoError on failure.
/// This ensures the path exists and we avoid accidental traversal outside roots.
fn canonicalize_strict(path: &Path, operation: &'static str) -> Result<PathBuf, IoError> {
    match std::fs::canonicalize(path) {
        Ok(p) => Ok(p),
        Err(e) => Err(IoError::FileOperation {
            operation,
            path: path.to_path_buf(),
            kind: e.kind(),
            source: Arc::new(e),
        }),
    }
}

pub(crate) fn path_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    let path_canon = canonicalize_best_effort(path);
    roots
        .iter()
        .any(|root| path_canon.starts_with(&canonicalize_best_effort(root)))
}

pub(crate) fn normalize_against_roots(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, IoError> {
    if !path.is_absolute() {
        return Err(IoError::FileOperation {
            operation: "read",
            path: path.to_path_buf(),
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path must be absolute",
            )),
            kind: std::io::ErrorKind::InvalidInput,
        });
    }
    // Strictly canonicalize the file path; fail if it cannot be resolved.
    let canon = canonicalize_strict(path, "read")?;
    if path_within_roots(&canon, roots) {
        Ok(canon)
    } else {
        Err(IoError::FileOperation {
            operation: "read",
            path: canon,
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path outside configured roots",
            )),
            kind: std::io::ErrorKind::InvalidInput,
        })
    }
}

// Future enhancements (Phase 7):
// - canonicalize paths and compare against configured roots
// - symlink policy: follow or deny across root boundaries based on config
// - clear error mapping for permission/traversal attempts

/// Symlink handling policy placeholder (Phase 7).
#[derive(Debug, Clone, Copy)]
pub enum SymlinkPolicy {
    /// Follow symlinks during normalization.
    Follow,
    /// Deny traversals that would escape configured roots via symlinks.
    DenyCrossRoot,
}

/// Placeholder for future symlink-aware root checks.
/// Currently delegates to `path_within_roots` until policy is fully implemented.
pub(crate) fn path_within_roots_with_policy(
    path: &Path,
    roots: &[PathBuf],
    _policy: SymlinkPolicy,
) -> bool {
    // TODO(Phase 7): Implement strict symlink policy evaluation
    path_within_roots(path, roots)
}

