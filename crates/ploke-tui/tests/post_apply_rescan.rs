//! Post-apply rescan
//!
//! Purpose: after approving a proposal, the system should schedule a rescan and
//! surface a SysInfo message indicating so.
//!
//! Approach: stage a minimal proposal (no-op edits) and approve it, then assert
//! the chat history includes a SysInfo message about scheduling a rescan.

use std::sync::Arc;

use ploke_core::ArcStr;
use ploke_tui::{
    EventBus,
    app_state::core::{
        AppState, ChatState, ConfigState, EditProposal, EditProposalStatus, RuntimeConfig,
        SystemState,
    },
    event_bus::EventBusCaps,
};
use tokio::sync::RwLock;

#[tokio::test]
async fn approve_emits_rescan_sysinfo() {
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let cfg = ploke_tui::user_config::UserConfig::default();
    let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder"));
    let state = Arc::new(AppState {
        chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig::from(cfg.clone())),
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
        create_proposals: RwLock::new(std::collections::HashMap::new()),
    });
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Insert a no-op proposal (empty edits) to take the approve path
    let req_id = uuid::Uuid::new_v4();
    {
        let mut guard = state.proposals.write().await;
        guard.insert(
            req_id,
            EditProposal {
                request_id: req_id,
                parent_id: uuid::Uuid::new_v4(),
                call_id: ArcStr::from("test_tool_call:0"),
                proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                edits: vec![],
                edits_ns: vec![],
                files: vec![],
                preview: ploke_tui::app_state::core::DiffPreview::UnifiedDiff {
                    text: String::new(),
                },
                status: EditProposalStatus::Pending,
                is_semantic: true,
            },
        );
    }

    // Approve and wait briefly
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        ploke_tui::rag::editing::approve_edits(&state, &event_bus, req_id),
    )
    .await
    .expect("approve_edits timed out");

    // Inspect chat for a rescan SysInfo
    let chat_guard = state.chat.0.read().await;
    let path = chat_guard.get_full_path();
    let found = path.iter().any(|m| {
        m.kind == ploke_tui::chat_history::MessageKind::SysInfo
            && m.content.contains("Scheduled rescan of workspace")
    });
    assert!(
        found,
        "expected scheduled rescan SysInfo message in chat history"
    );
}
