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
