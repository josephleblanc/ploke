use serde_json::json;
use uuid::Uuid;

use ploke_tui::event_bus::EventBusCaps;
use ploke_tui::test_utils::mock::create_mock_app_state;
use ploke_tui::tools::{
    CanonicalEdit, CodeEdit, CodeEditInput, FunctionMarker, Tool, ToolDefinition, ToolFunctionDef,
    ToolName, GetFileMetadataTool, GetFileMetadataInput,
};

#[test]
fn code_edit_tool_def_serializes_expected_shape() {
    let def: ToolFunctionDef = <CodeEdit as Tool>::tool_def();
    let v = serde_json::to_value(&def).expect("serialize tool def");
    let func = v.as_object().expect("def obj");
    assert_eq!(func.get("name").and_then(|n| n.as_str()), Some("apply_code_edit"));
    assert_eq!(
        func.get("description").and_then(|d| d.as_str()),
        Some("Apply canonical code edits to one or more nodes identified by canonical path.")
    );
    let params = func.get("parameters").and_then(|p| p.as_object()).expect("params obj");
    let props = params.get("properties").and_then(|p| p.as_object()).expect("props obj");
    assert!(props.contains_key("edits"));
}

#[test]
fn code_edit_canonical_mapping_includes_mode_tag() {
    use ploke_tui::rag::utils::{ApplyCodeEditRequest, Edit};

    let input = CodeEditInput {
        edits: vec![CanonicalEdit {
            file: "src/lib.rs".to_string(),
            canon: "crate::foo::bar".to_string(),
            node_type: ploke_db::NodeType::Function,
            code: "fn bar(){}".to_string(),
        }],
        confidence: Some(0.7),
    };

    // Mirror the conversion performed in CodeEdit::run
    let typed = ApplyCodeEditRequest {
        confidence: input.confidence,
        edits: input
            .edits
            .into_iter()
            .map(|e| Edit::Canonical {
                file: e.file,
                canon: e.canon,
                node_type: e.node_type,
                code: e.code,
            })
            .collect(),
    };

    let payload = serde_json::to_value(&typed).expect("serialize typed request");
    let edits = payload.get("edits").and_then(|e| e.as_array()).expect("array");
    let first = edits.first().and_then(|e| e.as_object()).expect("obj");
    assert_eq!(first.get("mode").and_then(|m| m.as_str()), Some("canonical"));
}

#[test]
fn get_file_metadata_tool_def_serializes_expected_shape() {
    let def: ToolFunctionDef = <GetFileMetadataTool as Tool>::tool_def();
    let v = serde_json::to_value(&def).expect("serialize tool def");
    let func = v.as_object().expect("def obj");
    assert_eq!(func.get("name").and_then(|n| n.as_str()), Some("get_file_metadata"));
    let params = func.get("parameters").and_then(|p| p.as_object()).expect("params obj");
    let req = params.get("required").and_then(|r| r.as_array()).expect("required arr");
    assert!(req.iter().any(|s| s.as_str() == Some("file_path")));
}

#[tokio::test]
async fn get_file_metadata_run_happy_path() {
    use std::sync::Arc;
    use std::{fs, path::PathBuf};

    let state = Arc::new(create_mock_app_state());
    let event_bus = Arc::new(ploke_tui::EventBus::new(EventBusCaps::default()));

    // Create a temporary file in the current workspace target dir
    let tmp_dir = std::env::temp_dir();
    let file_path = tmp_dir.join(format!("ploke_tui_test_{}.txt", Uuid::new_v4()));
    fs::write(&file_path, b"hello world").expect("write temp file");

    let tool = GetFileMetadataTool {
        state,
        event_bus,
        parent_id: Uuid::new_v4(),
    };

    let res = tool
        .run(GetFileMetadataInput {
            file_path: file_path.display().to_string(),
        })
        .await
        .expect("tool run ok");

    assert!(res.ok);
    assert!(res.exists);
    assert_eq!(res.byte_len, 11);
    assert_eq!(res.file_path, file_path.display().to_string());
    assert_eq!(res.tracking_hash.len(), 36);
}
