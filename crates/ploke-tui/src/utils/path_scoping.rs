use std::path::{Component, Path, PathBuf};

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

    let root_norm = normalize(root);
    let target = if p.is_absolute() {
        normalize(p)
    } else {
        normalize(&root_norm.join(p))
    };

    if is_within(&target, &root_norm) {
        Ok(target)
    } else {
        Err("path resolves outside crate root".to_string())
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut res = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(prefix) => res.push(prefix.as_os_str()),
            Component::RootDir => res.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                // Do not pop past root
                let _ = res.pop();
            }
            Component::Normal(seg) => res.push(seg),
        }
    }
    res
}

fn is_within(target: &Path, root: &Path) -> bool {
    // Ensure root is a prefix of target on component boundary
    if root.as_os_str().is_empty() {
        return false;
    }
    // Fast path
    if target.starts_with(root) {
        return true;
    }
    false
}
