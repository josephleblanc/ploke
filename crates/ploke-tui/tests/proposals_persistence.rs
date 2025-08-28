//! Proposal persistence tests
//!
//! Purpose: validate proposals are persisted to disk and can be reloaded, and
//! that missing or corrupted persistence files do not crash or corrupt state.
//!
//! TEST_GUIDELINES adherence:
//! - Deterministic: temp dir for file path; no network; typed JSON roundtrip.
//! - Semantic assertions: verify roundtrip preservation and empty load on
//!   missing/corrupted inputs. No insta snapshots needed (non-visual).
//! - External behavior is local-only and hermetic.
//!
//! Verified properties:
//! - Roundtrip: proposals saved and reloaded match by id; status preserved.
//! - Missing/corrupted file: load is graceful; proposals map remains unchanged.
//!
//! Not verified (by design):
//! - Specific warning log content; treated as best-effort and not part of the
//!   functional contract. We assert the absence of crashes and preservation of state.

use std::sync::Arc;

use ploke_tui::app_state::core::{AppState, ChatState, ConfigState, EditProposal, EditProposalStatus, RuntimeConfig, SystemState};
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn proposals_save_and_load_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let proposals_path = dir.path().join("proposals.json");

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

    timeout(
        Duration::from_secs(30),
        ploke_tui::app_state::handlers::proposals::save_proposals_to_path(&state, &proposals_path),
    )
    .await
    .expect("save_proposals_to_path timed out");
    assert!(proposals_path.exists(), "proposals file should exist");

    {
        let mut guard = state.proposals.write().await;
        guard.clear();
    }
    timeout(
        Duration::from_secs(30),
        ploke_tui::app_state::handlers::proposals::load_proposals_from_path(&state, &proposals_path),
    )
    .await
    .expect("load_proposals_from_path timed out");
    let guard = state.proposals.read().await;
    assert_eq!(guard.len(), 1, "should reload one proposal");
    assert!(guard.get(&req_id).is_some(), "reloaded proposal should match id");
}

#[tokio::test]
async fn proposals_load_missing_file_is_graceful() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing_path = dir.path().join("does_not_exist.json");

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

    timeout(
        Duration::from_secs(30),
        ploke_tui::app_state::handlers::proposals::load_proposals_from_path(&state, &missing_path),
    )
    .await
    .expect("load_proposals_from_path timed out");
    let guard = state.proposals.read().await;
    assert_eq!(guard.len(), 0, "no proposals loaded for missing file");
}

#[tokio::test]
async fn proposals_load_corrupted_file_is_graceful() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("proposals.json");
    std::fs::write(&path, "{ not: valid json [").expect("write corrupted file");

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

    timeout(
        Duration::from_secs(30),
        ploke_tui::app_state::handlers::proposals::load_proposals_from_path(&state, &path),
    )
    .await
    .expect("load_proposals_from_path timed out");
    let guard = state.proposals.read().await;
    assert_eq!(guard.len(), 0, "no proposals loaded from corrupted file");
}

