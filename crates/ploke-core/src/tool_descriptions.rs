use crate::tool_types::ToolName;

pub type ToolDescription = &'static str;
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

#[cfg(test)]
mod tests {
    use super::{tool_description, tool_description_artifact_relpath};
    use crate::tool_types::ToolName;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    static TOOL_DESCRIPTION_FILE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct RestoreFileGuard {
        path: PathBuf,
        original: String,
    }

    impl Drop for RestoreFileGuard {
        fn drop(&mut self) {
            let _ = fs::write(&self.path, &self.original);
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate dir should have workspace parent")
            .parent()
            .expect("workspace dir should have parent")
            .to_path_buf()
    }

    #[test]
    #[ignore = "known regression: tool descriptions are baked in with include_str!, so runtime file edits are invisible until rebuild"]
    fn tool_description_reflects_runtime_file_edits() {
        let _guard = TOOL_DESCRIPTION_FILE_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("lock poisoned");

        let tool = ToolName::RequestCodeContext;
        let path = repo_root().join(tool_description_artifact_relpath(tool));
        let original = fs::read_to_string(&path).expect("read tool description artifact");
        let _restore = RestoreFileGuard {
            path: path.clone(),
            original: original.clone(),
        };

        let updated = format!("{original}\n\nTEST_SENTINEL_RUNTIME_RELOAD\n");
        fs::write(&path, &updated).expect("write modified tool description artifact");

        assert_eq!(
            tool_description(tool),
            updated,
            "tool descriptions used by the runtime must reflect on-disk edits without requiring a rebuild"
        );
    }
}
