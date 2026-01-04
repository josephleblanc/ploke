use super::*;

// Target module for path policy and security

#[derive(Debug, Clone)]
pub struct PathPolicy {
    pub roots: Vec<PathBuf>,
    pub symlink_policy: SymlinkPolicy,
    pub require_absolute: bool,
}

impl PathPolicy {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            symlink_policy: SymlinkPolicy::DenyCrossRoot,
            require_absolute: true,
        }
    }
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn lexical_normalize_abs(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::RootDir => out.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(seg) => out.push(seg),
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
        }
    }
    out
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
        .any(|root| path_canon.starts_with(canonicalize_best_effort(root)))
}

#[tracing::instrument]
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

pub fn normalize_target_path(path: &Path, policy: &PathPolicy) -> Result<PathBuf, IoError> {
    if policy.require_absolute && !path.is_absolute() {
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

    normalize_against_roots_with_policy(path, &policy.roots, policy.symlink_policy)
}

/// Normalize a path against configured roots without requiring the target to exist.
pub fn normalize_target_path_allow_missing(
    path: &Path,
    policy: &PathPolicy,
    operation: &'static str,
) -> Result<PathBuf, IoError> {
    if policy.require_absolute && !path.is_absolute() {
        return Err(IoError::FileOperation {
            operation,
            path: path.to_path_buf(),
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path must be absolute",
            )),
            kind: std::io::ErrorKind::InvalidInput,
        });
    }

    normalize_against_roots_allow_missing(path, &policy.roots, policy.symlink_policy, operation)
}

/// Normalize a path against configured roots using a symlink policy.
/// Currently enforces strict canonicalization, then checks containment using the provided policy.
///
/// Note: Until full policy is implemented, this defers to `path_within_roots_with_policy` which
/// currently delegates to `path_within_roots`.
pub(crate) fn normalize_against_roots_with_policy(
    path: &Path,
    roots: &[PathBuf],
    policy: SymlinkPolicy,
) -> Result<PathBuf, IoError> {
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

    match policy {
        SymlinkPolicy::DenyCrossRoot => {
            // Strict canonicalization first; containment check on canonical paths.
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
        SymlinkPolicy::Follow => {
            // Lexical containment (prevents '..' traversal) without following symlinks,
            // then strict canonicalization for consistent hashing/IO.
            let lex_path = lexical_normalize_abs(path);
            let within = roots.iter().any(|r| {
                let lex_root = lexical_normalize_abs(r);
                lex_path.starts_with(&lex_root)
            });

            if !within {
                return Err(IoError::FileOperation {
                    operation: "read",
                    path: path.to_path_buf(),
                    source: Arc::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "path outside configured roots",
                    )),
                    kind: std::io::ErrorKind::InvalidInput,
                });
            }

            let canon = canonicalize_strict(path, "read")?;
            Ok(canon)
        }
    }
}

fn normalize_against_roots_allow_missing(
    path: &Path,
    roots: &[PathBuf],
    policy: SymlinkPolicy,
    operation: &'static str,
) -> Result<PathBuf, IoError> {
    if !path.is_absolute() {
        return Err(IoError::FileOperation {
            operation,
            path: path.to_path_buf(),
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path must be absolute",
            )),
            kind: std::io::ErrorKind::InvalidInput,
        });
    }

    let lex_path = lexical_normalize_abs(path);

    let match_result = roots.iter().find_map(|root| {
        let lex_root = lexical_normalize_abs(root);
        if !lex_path.starts_with(&lex_root) {
            return None;
        }
        if matches!(policy, SymlinkPolicy::DenyCrossRoot) {
            return Some(validate_existing_prefixes_within_root(
                &lex_path,
                &lex_root,
                operation,
            )
            .map(|_| lex_root));
        }
        Some(Ok(lex_root))
    });

    match match_result {
        Some(Ok(_)) => Ok(lex_path),
        Some(Err(err)) => Err(err),
        None => Err(path_outside_roots_error(lex_path, operation)),
    }
}

fn validate_existing_prefixes_within_root(
    lex_path: &Path,
    lex_root: &Path,
    operation: &'static str,
) -> Result<(), IoError> {
    let canon_root = canonicalize_best_effort(lex_root);
    let mut prefix = lex_root.to_path_buf();

    if prefix.exists() {
        let canon = canonicalize_strict(&prefix, operation)?;
        if !canon.starts_with(&canon_root) {
            return Err(path_outside_roots_error(lex_path.to_path_buf(), operation));
        }
    }

    if let Ok(rel) = lex_path.strip_prefix(lex_root) {
        for comp in rel.components() {
            prefix.push(comp);
            if !prefix.exists() {
                continue;
            }
            let canon = canonicalize_strict(&prefix, operation)?;
            if !canon.starts_with(&canon_root) {
                return Err(path_outside_roots_error(lex_path.to_path_buf(), operation));
            }
        }
    }

    Ok(())
}

fn path_outside_roots_error(path: PathBuf, operation: &'static str) -> IoError {
    IoError::FileOperation {
        operation,
        path,
        source: Arc::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path outside configured roots",
        )),
        kind: std::io::ErrorKind::InvalidInput,
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
