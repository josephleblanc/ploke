use super::*;

// Target module for path policy and security

pub(crate) fn path_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}


// Future enhancements (Phase 7):
// - canonicalize paths and compare against configured roots
// - symlink policy: follow or deny across root boundaries based on config
// - clear error mapping for permission/traversal attempts

