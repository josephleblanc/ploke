#![cfg(feature = "test_harness")]

use std::sync::Arc;

use serde_json::json;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use ploke_tui::app_state::core::PreviewMode;
use ploke_tui::event_bus::EventBusCaps;
use ploke_tui::{AppEvent, EventBus};
use ploke_tui::system::SystemEvent;
use ploke_tui::rag::utils::ToolCallParams;
use ploke_test_utils::workspace_root;

#[tokio::test]
async fn e2e_get_file_metadata_and_apply_code_edit_splice() {
    // Realistic state from TEST_APP with fixture DB loaded
    let state = ploke_tui::test_harness::get_state().await;
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Create a temp file and write content
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("demo.rs");
    std::fs::write(&file_path, "fn demo() { let x = 1; }\n").expect("write");

    // 1) Call get_file_metadata via dispatcher
    let request_id_meta = Uuid::new_v4();
    let call_id_meta = Uuid::new_v4().to_string();
    let mut rx = event_bus.realtime_tx.subscribe();
    let args_meta = json!({"file_path": file_path.display().to_string()});
    let params_meta = ToolCallParams {
        state: Arc::clone(&state),
        event_bus: Arc::clone(&event_bus),
        request_id: request_id_meta,
        parent_id: Uuid::new_v4(),
        name: "get_file_metadata".to_string(),
        arguments: args_meta,
        call_id: call_id_meta.clone(),
    };
    tokio::spawn(async move {
        ploke_tui::rag::dispatcher::handle_tool_call_requested(params_meta).await;
    });

    let content_meta = timeout(Duration::from_secs(5), async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::System(SystemEvent::ToolCallCompleted { request_id, call_id, content, .. }))
                    if request_id == request_id_meta && call_id == call_id_meta => break Ok(content),
                Ok(AppEvent::System(SystemEvent::ToolCallFailed { request_id, call_id, error, .. }))
                    if request_id == request_id_meta && call_id == call_id_meta => break Err(error),
                Ok(_) => continue,
                Err(e) => break Err(format!("event error: {}", e)),
            }
        }
    }).await.expect("timeout waiting for get_file_metadata").expect("get_file_metadata failed");

    let meta: ploke_core::rag_types::GetFileMetadataResult = serde_json::from_str(&content_meta).expect("deserialize meta");
    assert!(meta.ok && meta.exists);
    assert_eq!(meta.file_path, file_path.display().to_string());
    let on_disk = std::fs::metadata(&file_path).unwrap().len();
    assert_eq!(meta.byte_len, on_disk);

    // 2) Call apply_code_edit with a splice to rename `demo` -> `demo_ok`
    let initial = std::fs::read_to_string(&file_path).unwrap();
    let start = initial.find("demo").unwrap();
    let end = start + "demo".len();

    let request_id_edit = Uuid::new_v4();
    let call_id_edit = Uuid::new_v4().to_string();
    let mut rx2 = event_bus.realtime_tx.subscribe();
    // Compute tracking hash compatible with IoManager (token-based)
    let ast = syn::parse_file(&initial).expect("parse rust file");
    let tokens = quote::ToTokens::to_token_stream(&ast);
    let expected = ploke_core::TrackingHash::generate(
        ploke_core::PROJECT_NAMESPACE_UUID,
        &file_path,
        &tokens,
    )
    .0
    .to_string();

    let args_edit = json!({
        "edits": [{
            "file_path": file_path.display().to_string(),
            "expected_file_hash": expected,
            "start_byte": start as u64,
            "end_byte": end as u64,
            "replacement": "demo_ok"
        }]
    });
    let params_edit = ToolCallParams {
        state: Arc::clone(&state),
        event_bus: Arc::clone(&event_bus),
        request_id: request_id_edit,
        parent_id: Uuid::new_v4(),
        name: "apply_code_edit".to_string(),
        arguments: args_edit,
        call_id: call_id_edit.clone(),
    };
    tokio::spawn(async move {
        ploke_tui::rag::dispatcher::handle_tool_call_requested(params_edit).await;
    });

    let content_edit = timeout(Duration::from_secs(5), async move {
        loop {
            match rx2.recv().await {
                Ok(AppEvent::System(SystemEvent::ToolCallCompleted { request_id, call_id, content, .. }))
                    if request_id == request_id_edit && call_id == call_id_edit => break Ok(content),
                Ok(AppEvent::System(SystemEvent::ToolCallFailed { request_id, call_id, error, .. }))
                    if request_id == request_id_edit && call_id == call_id_edit => break Err(error),
                Ok(_) => continue,
                Err(e) => break Err(format!("event error: {}", e)),
            }
        }
    }).await.expect("timeout waiting for apply_code_edit").expect("apply_code_edit failed");

    let result: ploke_core::rag_types::ApplyCodeEditResult = serde_json::from_str(&content_edit).expect("deserialize result");
    assert!(result.ok && result.staged == 1);
    // Preview mode depends on config; just check it is either diff or codeblock
    assert!(result.preview_mode == "diff" || result.preview_mode == "codeblock");

    // Approve edits to actually apply
    ploke_tui::rag::editing::approve_edits(&state, &event_bus, request_id_edit)
        .await;
    let updated = std::fs::read_to_string(&file_path).unwrap();
    assert!(updated.contains("demo_ok"));
}

#[tokio::test]
async fn e2e_apply_code_edit_canonical_on_fixture() {
    // Use the fixture_nodes crate file and restore it after test
    let state = ploke_tui::test_harness::get_state().await;
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Set crate_focus to the fixture root so canonical resolution uses the same root as DB
    let crate_root = workspace_root().join("tests/fixture_crates/fixture_nodes");
    {
        use ploke_tui::app_state::core::SystemStatus;
        let mut sys = state.system.write().await;
        *sys = SystemStatus::new(Some(crate_root.clone()));
    }
    let rel_file = std::path::PathBuf::from("src/imports.rs");
    let abs_file = crate_root.join(&rel_file);
    let original = std::fs::read_to_string(&abs_file).expect("read fixture");

    // Stage canonical edit: replace function body with a tiny marker
    let request_id = Uuid::new_v4();
    let call_id = Uuid::new_v4().to_string();
    let mut rx = event_bus.realtime_tx.subscribe();
    let args = json!({
        "edits": [{
            "mode": "canonical",
            "file": rel_file.display().to_string(),
            "canon": "crate::imports::use_imported_items",
            "node_type": "function",
            "code": "pub fn use_imported_items() { let _e2e_marker = 7; }"
        }]
    });
    let params = ToolCallParams {
        state: Arc::clone(&state),
        event_bus: Arc::clone(&event_bus),
        request_id,
        parent_id: Uuid::new_v4(),
        name: "apply_code_edit".to_string(),
        arguments: args,
        call_id: call_id.clone(),
    };
    tokio::spawn(async move {
        ploke_tui::rag::dispatcher::handle_tool_call_requested(params).await;
    });

    let content = timeout(Duration::from_secs(10), async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::System(SystemEvent::ToolCallCompleted { request_id: rid, call_id: cid, content, .. }))
                    if rid == request_id && cid == call_id => break Ok(content),
                Ok(AppEvent::System(SystemEvent::ToolCallFailed { request_id: rid, call_id: cid, error, .. }))
                    if rid == request_id && cid == call_id => break Err(error),
                Ok(_) => continue,
                Err(e) => break Err(format!("event error: {}", e)),
            }
        }
    })
    .await
    .expect("timeout waiting for canonical apply")
    .expect("canonical apply failed to stage");

    // Apply it
    ploke_tui::rag::editing::approve_edits(&state, &event_bus, request_id).await;
    let updated = std::fs::read_to_string(&abs_file).unwrap();
    assert!(updated.contains("_e2e_marker"));

    // Restore original file to avoid test side effects
    std::fs::write(&abs_file, original).expect("restore original");
}
