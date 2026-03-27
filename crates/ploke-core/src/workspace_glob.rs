use std::fs;
use std::path::{Path, PathBuf};

/// Expand a Cargo workspace member path with minimal `*` wildcard support.
///
/// This is intentionally **not** a full glob implementation. It matches the current,
/// conservative behavior used by Ploke:
/// - If the joined path exists, it is returned as-is.
/// - Only a single `*` wildcard within the final path segment is expanded by scanning
///   the parent directory for matching subdirectories.
/// - If expansion yields no matches (or the directory cannot be read), the original
///   joined path is returned (so callers can keep surfacing "path not found" errors).
///
/// Examples of patterns this supports:
/// - `axum-*` (workspace root siblings)
/// - `crates/*` (immediate children under `crates/`)
pub fn expand_simple_workspace_member(workspace_root: &Path, member_path: &Path) -> Vec<PathBuf> {
    let joined = workspace_root.join(member_path);
    if joined.exists() {
        return vec![joined];
    }

    let file_name = match member_path.file_name().and_then(|name| name.to_str()) {
        Some(name) if name.contains('*') => name,
        _ => return vec![joined],
    };

    let Some((prefix, suffix)) = file_name.split_once('*') else {
        return vec![joined];
    };

    let parent = joined.parent().unwrap_or(workspace_root);
    let Ok(read_dir) = fs::read_dir(parent) else {
        return vec![joined];
    };

    let mut expanded = read_dir
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix) && name.ends_with(suffix))
        })
        .collect::<Vec<_>>();

    if expanded.is_empty() {
        vec![joined]
    } else {
        expanded.sort();
        expanded
    }
}

