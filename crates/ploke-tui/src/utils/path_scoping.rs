use std::path::{Path, PathBuf};

use ploke_io::path_policy::{PathPolicy, normalize_target_path_allow_missing};

/// Resolve a user path for tool execution: relative paths join to `primary_root` (typically the
/// workspace root), then containment is checked against all `policy.roots`.
pub fn resolve_tool_path(
    user_path: &Path,
    primary_root: &Path,
    policy: &PathPolicy,
) -> Result<PathBuf, String> {
    if primary_root.as_os_str().is_empty() {
        return Err("invalid workspace root".to_string());
    }

    let target = if user_path.is_absolute() {
        user_path.to_path_buf()
    } else {
        primary_root.join(user_path)
    };
    normalize_target_path_allow_missing(&target, policy, "read").map_err(|err| err.to_string())
}

/// Resolve a user-supplied path within a single crate root.
///
/// Prefer [`resolve_tool_path`] with `SystemStatus::tool_path_context` for workspace-aware tools.
pub fn resolve_in_crate_root<P: AsRef<Path>, R: AsRef<Path>>(
    user_path: P,
    crate_root: R,
) -> Result<PathBuf, String> {
    let root = crate_root.as_ref().to_path_buf();
    let policy = PathPolicy::new(vec![root.clone()]);
    resolve_tool_path(user_path.as_ref(), &root, &policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolve_tool_path_relative_under_workspace() {
        let ws = PathBuf::from("/ws");
        let member = ws.join("crate_a");
        let policy = PathPolicy::new(vec![ws.clone(), member.clone()]);
        let p = resolve_tool_path(Path::new("crate_a/src/lib.rs"), &ws, &policy).unwrap();
        assert_eq!(p, PathBuf::from("/ws/crate_a/src/lib.rs"));
    }

    #[test]
    fn resolve_tool_path_absolute_must_match_policy() {
        let ws = PathBuf::from("/ws");
        let policy = PathPolicy::new(vec![ws.clone()]);
        assert!(resolve_tool_path(Path::new("/other/x"), &ws, &policy).is_err());
        let ok = resolve_tool_path(Path::new("/ws/x"), &ws, &policy).unwrap();
        assert_eq!(ok, PathBuf::from("/ws/x"));
    }
}
