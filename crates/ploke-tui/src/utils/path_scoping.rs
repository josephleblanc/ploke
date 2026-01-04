use std::path::{Path, PathBuf};

use ploke_io::path_policy::{PathPolicy, normalize_target_path_allow_missing};

/// Resolve a user-supplied path within a crate root.
///
/// Intended behavior (to be enforced in implementation):
/// - If `user_path` is relative, join to `crate_root` and normalize; reject if it escapes root.
/// - If `user_path` is absolute, accept only if contained within `crate_root`.
/// - Support common LLM outputs that redundantly include the root segment.
///
/// Current stub: naive join without validation (will be tightened in implementation).
pub fn resolve_in_crate_root<P: AsRef<Path>, R: AsRef<Path>>(
    user_path: P,
    crate_root: R,
) -> Result<PathBuf, String> {
    let p = user_path.as_ref();
    let root = crate_root.as_ref();
    if root.as_os_str().is_empty() {
        return Err("invalid crate_root".to_string());
    }

    let target = if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    };
    let policy = PathPolicy::new(vec![root.to_path_buf()]);
    normalize_target_path_allow_missing(&target, &policy, "read")
        .map_err(|err| err.to_string())
}
