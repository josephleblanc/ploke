use std::sync::Arc;

use ploke_tui::app_state::core::{AppState, ChatState, ConfigState, EditProposal, EditProposalStatus, RuntimeConfig, SystemState};
use ploke_tui::EventBus;
use ploke_tui::event_bus::EventBusCaps;
use tokio::sync::RwLock;

#[tokio::test]
async fn proposals_save_and_load_roundtrip() {
    // Use temp path for proposals persistence
    let dir = tempfile::tempdir().expect("tempdir");
    let proposals_path = dir.path().join("proposals.json");

    // Minimal state
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
    });

    // Insert a dummy proposal
    let req_id = uuid::Uuid::new_v4();
    let proposal = EditProposal {
        request_id: req_id,
        parent_id: uuid::Uuid::new_v4(),
        call_id: uuid::Uuid::new_v4().to_string(),
        proposed_at_ms: chrono::Utc::now().timestamp_millis(),
        edits: vec![],
        files: vec![std::env::current_dir().unwrap().join("Cargo.toml")],
        preview: ploke_tui::app_state::core::DiffPreview::UnifiedDiff { text: "diff".into() },
        status: EditProposalStatus::Pending,
    };
    {
        let mut guard = state.proposals.write().await;
        guard.insert(req_id, proposal);
    }

    // Save
    ploke_tui::app_state::handlers::proposals::save_proposals_to_path(&state, &proposals_path).await;
    assert!(proposals_path.exists(), "proposals file should exist");

    // Clear and load
    {
        let mut guard = state.proposals.write().await;
        guard.clear();
    }
    ploke_tui::app_state::handlers::proposals::load_proposals_from_path(&state, &proposals_path).await;
    let guard = state.proposals.read().await;
    assert_eq!(guard.len(), 1, "should reload one proposal");
    assert!(guard.get(&req_id).is_some(), "reloaded proposal should match id");
}
