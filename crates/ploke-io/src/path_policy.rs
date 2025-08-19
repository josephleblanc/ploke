use super::*;

// Target module for path policy and security

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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
    let canon = canonicalize_best_effort(path);
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

