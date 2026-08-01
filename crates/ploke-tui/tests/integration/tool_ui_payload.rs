use ploke_core::ArcStr;
use ploke_core::tool_types::ToolName;
use ploke_tui::tools::{ToolError, ToolErrorCode, ToolUiPayload, ToolVerbosity};

#[test]
fn render_tool_payload_respects_verbosity() {
    let payload = ToolUiPayload::new(
        ToolName::ApplyCodeEdit,
        ArcStr::from("call-1"),
        "Staged 2 edits",
    )
    .with_field("files", "2")
    .with_details("Applied in preview mode.");

    let minimal = payload.render(ToolVerbosity::Minimal);
    assert!(minimal.contains("apply_code_edit"));
    assert!(!minimal.contains("Fields:"));

    let normal = payload.render(ToolVerbosity::Normal);
    assert!(normal.contains("Fields:"));
    assert!(normal.contains("files: 2"));

    let verbose = payload.render(ToolVerbosity::Verbose);
    assert!(verbose.contains("Details:"));
    assert!(verbose.contains("Applied in preview mode."));
}

#[test]
fn render_error_payload_includes_error_code() {
    let err = ToolError::new(
        ToolName::RequestCodeContext,
        ToolErrorCode::MissingField,
        "missing search_term",
    )
    .field("search_term");

    let payload = ToolUiPayload::from_error(ArcStr::from("call-err"), &err);
    let rendered = payload.render(ToolVerbosity::Normal);

    assert!(rendered.contains("missing search_term"));
    assert!(rendered.contains("search_term"));
    assert!(rendered.contains("missing_field"));
}
