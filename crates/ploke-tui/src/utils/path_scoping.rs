use std::path::{Path, PathBuf};

use ploke_io::path_policy::{PathPolicy, normalize_target_path_allow_missing};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteScope {
    denied_prefixes: Vec<PathBuf>,
    denied_filenames: Vec<String>,
}

impl WriteScope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn deny_prefixes(mut self, prefixes: impl IntoIterator<Item = PathBuf>) -> Self {
        self.denied_prefixes.extend(prefixes);
        self
    }

    pub fn deny_filenames(mut self, filenames: impl IntoIterator<Item = String>) -> Self {
        self.denied_filenames.extend(filenames);
        self
    }

    pub fn check(&self, primary_root: &Path, resolved_path: &Path) -> Result<(), String> {
        let rel = resolved_path.strip_prefix(primary_root).map_err(|_| {
            format!(
                "path '{}' is outside writable root '{}'",
                resolved_path.display(),
                primary_root.display()
            )
        })?;
        if self
            .denied_prefixes
            .iter()
            .any(|prefix| rel.starts_with(prefix))
        {
            return Err(format!("path '{}' is protected", rel.display()));
        }
        if rel
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| self.denied_filenames.iter().any(|denied| denied == name))
        {
            return Err(format!("path '{}' is protected", rel.display()));
        }
        Ok(())
    }
}

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

pub fn resolve_write_path(
    user_path: &Path,
    primary_root: &Path,
    policy: &PathPolicy,
    scope: Option<&WriteScope>,
) -> Result<PathBuf, String> {
    let resolved = resolve_tool_path(user_path, primary_root, policy)?;
    if let Some(scope) = scope {
        scope.check(primary_root, &resolved)?;
    }
    Ok(resolved)
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

    #[test]
    fn write_scope_rejects_paths_outside_primary_root() {
        let ws = PathBuf::from("/ws");
        let evidence = PathBuf::from("/evidence");
        let policy = PathPolicy::new(vec![ws.clone(), evidence.clone()]);
        let scope = WriteScope::new();

        let err = resolve_write_path(Path::new("/evidence/report.md"), &ws, &policy, Some(&scope))
            .expect_err("extra read roots are not write roots");

        assert!(err.contains("outside writable root"));
    }

    #[test]
    fn write_scope_rejects_denied_prefixes_and_filenames() {
        let ws = PathBuf::from("/ws");
        let policy = PathPolicy::new(vec![ws.clone()]);
        let scope = WriteScope::new()
            .deny_prefixes([PathBuf::from("crates/protected")])
            .deny_filenames(["Cargo.toml".to_string()]);

        let prefix_err = resolve_write_path(
            Path::new("crates/protected/src/lib.rs"),
            &ws,
            &policy,
            Some(&scope),
        )
        .expect_err("protected prefix should reject");
        assert!(prefix_err.contains("protected"));

        let filename_err = resolve_write_path(Path::new("Cargo.toml"), &ws, &policy, Some(&scope))
            .expect_err("protected filename should reject");
        assert!(filename_err.contains("protected"));
    }
}
