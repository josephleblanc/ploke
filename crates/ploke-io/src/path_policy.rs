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
        // AI: You are needlessly using the `&` borrowed expression which is automatically
        // dereferenced by the compiler. Add this to your mistakes
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
