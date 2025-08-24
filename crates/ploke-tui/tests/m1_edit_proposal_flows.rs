use std::sync::Arc;

use ploke_core::{PROJECT_NAMESPACE_UUID, TrackingHash};
use ploke_tui::{
    EventBus,
    app_state::{
        RuntimeConfig,
        core::{AppState, ChatState, ConfigState, EditProposalStatus, SystemState},
        handlers::rag::{ToolCallParams, approve_edits, deny_edits, handle_tool_call_requested},
    },
    event_bus::EventBusCaps,
    user_config::UserConfig,
};
use quote::ToTokens;
use tokio::sync::RwLock;
use uuid::Uuid;

fn make_temp_file_with(content: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("file.rs");
    std::fs::write(&file_path, content).expect("write temp file");
    dir
}

fn tracking_hash_for(path: &std::path::Path, content: &str) -> String {
    let ast = syn::parse_file(content).expect("parse rust file");
    let tokens = ast.into_token_stream();
    let hash = TrackingHash::generate(PROJECT_NAMESPACE_UUID, path, &tokens);
    hash.0.to_string()
}

async fn make_min_state() -> Arc<AppState> {
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();

    // Minimal provider registry via user config default
    let cfg = UserConfig::default();
    let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder init"));

    Arc::new(AppState {
        chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig {
            provider_registry: cfg.registry.clone(),
            ..Default::default()
        }),
        system: SystemState::default(),
        indexing_state: RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
        db,
        embedder,
        io_handle,
        rag: None,
        budget: ploke_rag::TokenBudget::default(),
        proposals: RwLock::new(std::collections::HashMap::new()),
    })
}

#[tokio::test]
async fn stage_proposal_creates_pending_entry_and_preview() {
    let state = make_min_state().await;
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    let dir = make_temp_file_with("fn demo() { let x = 1; }\n");
    let file_path = dir.path().join("file.rs");
    let initial = std::fs::read_to_string(&file_path).unwrap();

    // splice "demo" -> "demo2"
    let start = initial.find("demo").unwrap();
    let end = start + "demo".len();

    let expected_file_hash = tracking_hash_for(&file_path, &initial);

    let args = serde_json::json!({
        "confidence": 0.5,
        "namespace": PROJECT_NAMESPACE_UUID.to_string(),
        "edits": [{
            "file_path": file_path.display().to_string(),
            "expected_file_hash": expected_file_hash,
            "start_byte": start as u64,
            "end_byte": end as u64,
            "replacement": "demo2"
        }]
    });

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call_id = Uuid::new_v4().to_string();

    handle_tool_call_requested(ToolCallParams {
        state: &state,
        event_bus: &event_bus,
        request_id,
        parent_id,
        vendor: ploke_tui::llm::ToolVendor::OpenAI,
        name: "apply_code_edit".to_string(),
        arguments: args,
        call_id,
    })
    .await;

    // Verify proposal exists and is Pending with a preview
    let reg = state.proposals.read().await;
    let proposal = reg.get(&request_id).expect("proposal not found");
    match &proposal.status {
        EditProposalStatus::Pending => {}
        other => panic!("expected Pending, got {:?}", other),
    }
    match &proposal.preview {
        ploke_tui::app_state::core::DiffPreview::CodeBlocks { per_file }
            if !per_file.is_empty() => {}
        ploke_tui::app_state::core::DiffPreview::UnifiedDiff { text } if !text.is_empty() => {}
        _ => panic!("preview not populated"),
    }
}

#[tokio::test]
async fn approve_applies_edits_and_updates_status() {
    let state = make_min_state().await;
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    let dir = make_temp_file_with("fn demo() { let x = 1; }\n");
    let file_path = dir.path().join("file.rs");
    let initial = std::fs::read_to_string(&file_path).unwrap();

    // splice "demo" -> "demo_ok"
    let start = initial.find("demo").unwrap();
    let end = start + "demo".len();
    let expected_file_hash = tracking_hash_for(&file_path, &initial);

    let args = serde_json::json!({
        "edits": [{
            "file_path": file_path.display().to_string(),
            "expected_file_hash": expected_file_hash,
            "start_byte": start as u64,
            "end_byte": end as u64,
            "replacement": "demo_ok"
        }]
    });

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call_id = Uuid::new_v4().to_string();

    handle_tool_call_requested(ToolCallParams {
        state: &state,
        event_bus: &event_bus,
        request_id,
        parent_id,
        vendor: ploke_tui::llm::ToolVendor::OpenAI,
        name: "apply_code_edit".to_string(),
        arguments: args,
        call_id,
    })
    .await;

    approve_edits(&state, &event_bus, request_id).await;

    let updated = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        updated.contains("demo_ok"),
        "file did not contain applied replacement: {updated}"
    );

    let reg = state.proposals.read().await;
    let status = &reg.get(&request_id).expect("proposal not found").status;
    match status {
        EditProposalStatus::Applied => {}
        s => panic!("expected Applied, got {:?}", s),
    }
}

#[tokio::test]
async fn deny_marks_denied_and_does_not_change_file() {
    let state = make_min_state().await;
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    let dir = make_temp_file_with("fn demo() { let x = 1; }\n");
    let file_path = dir.path().join("file.rs");
    let initial = std::fs::read_to_string(&file_path).unwrap();

    // splice "demo" -> "denied_change"
    let start = initial.find("demo").unwrap();
    let end = start + "demo".len();
    let expected_file_hash = tracking_hash_for(&file_path, &initial);

    let args = serde_json::json!({
        "edits": [{
            "file_path": file_path.display().to_string(),
            "expected_file_hash": expected_file_hash,
            "start_byte": start as u64,
            "end_byte": end as u64,
            "replacement": "denied_change"
        }]
    });

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call_id = Uuid::new_v4().to_string();

    handle_tool_call_requested(ToolCallParams {
        state: &state,
        event_bus: &event_bus,
        request_id,
        parent_id,
        vendor: ploke_tui::llm::ToolVendor::OpenAI,
        name: "apply_code_edit".to_string(),
        arguments: args,
        call_id,
    })
    .await;

    deny_edits(&state, &event_bus, request_id).await;

    // File unchanged
    let updated = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(updated, initial, "file should be unchanged after denial");

    let reg = state.proposals.read().await;
    let status = &reg.get(&request_id).expect("proposal not found").status;
    match status {
        EditProposalStatus::Denied => {}
        s => panic!("expected Denied, got {:?}", s),
    }
}
