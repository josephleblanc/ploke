use crate::tool_types::ToolName;
use std::{borrow::Cow, fs, path::Path};

pub type ToolDescription = &'static str;
pub type RuntimeToolDescription = Cow<'static, str>;
pub type ToolDescriptionArtifactRelPath = &'static str;

pub fn tool_description(name: ToolName) -> ToolDescription {
    match name {
        ToolName::RequestCodeContext => include_str!("../tool_text/request_code_context.md"),
        ToolName::ApplyCodeEdit => include_str!("../tool_text/apply_code_edit.md"),
        ToolName::InsertRustItem => include_str!("../tool_text/insert_rust_item.md"),
        ToolName::CreateFile => include_str!("../tool_text/create_file.md"),
        ToolName::NsPatch => include_str!("../tool_text/non_semantic_patch.md"),
        ToolName::NsRead => include_str!("../tool_text/read_file.md"),
        ToolName::CodeItemLookup => include_str!("../tool_text/code_item_lookup.md"),
        ToolName::CodeItemEdges => include_str!("../tool_text/code_item_edges.md"),
        ToolName::Cargo => include_str!("../tool_text/cargo.md"),
        ToolName::ListDir => include_str!("../tool_text/list_dir.md"),
    }
}

pub fn tool_description_artifact_relpath(name: ToolName) -> ToolDescriptionArtifactRelPath {
    match name {
        ToolName::RequestCodeContext => "crates/ploke-core/tool_text/request_code_context.md",
        ToolName::ApplyCodeEdit => "crates/ploke-core/tool_text/apply_code_edit.md",
        ToolName::InsertRustItem => "crates/ploke-core/tool_text/insert_rust_item.md",
        ToolName::CreateFile => "crates/ploke-core/tool_text/create_file.md",
        ToolName::NsPatch => "crates/ploke-core/tool_text/non_semantic_patch.md",
        ToolName::NsRead => "crates/ploke-core/tool_text/read_file.md",
        ToolName::CodeItemLookup => "crates/ploke-core/tool_text/code_item_lookup.md",
        ToolName::CodeItemEdges => "crates/ploke-core/tool_text/code_item_edges.md",
        ToolName::Cargo => "crates/ploke-core/tool_text/cargo.md",
        ToolName::ListDir => "crates/ploke-core/tool_text/list_dir.md",
    }
}

/// Loads the live on-disk tool description when a checkout-local artifact is
/// available, otherwise falls back to the build-time `include_str!` copy.
///
/// Product contract: tool text under `crates/ploke-core/tool_text/*.md` is part
/// of the mutable runtime surface for dev/eval checkouts, so prompt edits made
/// on disk should affect newly built tool definitions without rebuilding the
/// binary. The fallback keeps installed binaries and non-checkout working
/// directories usable, at the cost of those environments retaining baked
/// descriptions until they run from a checkout or rebuild.
pub fn runtime_tool_description(name: ToolName) -> RuntimeToolDescription {
    if let Ok(current_dir) = std::env::current_dir() {
        if let Some(description) = runtime_tool_description_from_root(name, &current_dir) {
            return Cow::Owned(description);
        }
    }

    let build_workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent);
    if let Some(root) = build_workspace_root {
        if let Some(description) = runtime_tool_description_from_root(name, root) {
            return Cow::Owned(description);
        }
    }

    Cow::Borrowed(tool_description(name))
}

fn runtime_tool_description_from_root(name: ToolName, root: &Path) -> Option<String> {
    fs::read_to_string(root.join(tool_description_artifact_relpath(name))).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        runtime_tool_description_from_root, tool_description, tool_description_artifact_relpath,
    };
    use crate::tool_types::ToolName;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct RemoveDirGuard {
        path: PathBuf,
    }

    impl Drop for RemoveDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn temp_repo_root() -> (PathBuf, RemoveDirGuard) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ploke-core-tool-descriptions-{}-{unique}",
            std::process::id()
        ));
        let guard = RemoveDirGuard { path: root.clone() };
        (root, guard)
    }

    #[test]
    #[ignore = "runtime reload contract uses a temp checkout fixture"]
    fn tool_description_reflects_runtime_file_edits() {
        let tool = ToolName::RequestCodeContext;
        let (root, _guard) = temp_repo_root();
        let path = root.join(tool_description_artifact_relpath(tool));
        fs::create_dir_all(
            path.parent()
                .expect("tool description has parent directory"),
        )
        .expect("create temp tool_text directory");

        let original = "original temp tool description\n";
        fs::write(&path, original).expect("write temp tool description artifact");
        assert_eq!(
            runtime_tool_description_from_root(tool, &root).as_deref(),
            Some(original)
        );

        let updated = format!("{original}\n\nTEST_SENTINEL_RUNTIME_RELOAD\n");
        fs::write(&path, &updated).expect("write modified tool description artifact");

        assert_eq!(
            runtime_tool_description_from_root(tool, &root).as_deref(),
            Some(updated.as_str()),
            "tool descriptions used by the runtime must reflect on-disk edits without requiring a rebuild"
        );
    }

    #[test]
    fn runtime_tool_description_falls_back_to_baked_content_when_artifact_is_missing() {
        let tool = ToolName::RequestCodeContext;
        let (root, _guard) = temp_repo_root();

        assert!(runtime_tool_description_from_root(tool, &root).is_none());
        assert_eq!(
            tool_description(tool),
            include_str!("../tool_text/request_code_context.md")
        );
    }
}
