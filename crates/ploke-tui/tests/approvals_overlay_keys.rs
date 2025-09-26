//! Approvals overlay key handling tests
//!
//! Purpose: ensure overlay key inputs emit the correct `StateCommand`s and
//! provide user guidance when the editor is not configured.
//!
//! TEST_GUIDELINES adherence:
//! - Deterministic: no network; minimal in‑memory AppState; overlay opened programmatically.
//! - Semantic assertions: verify `ApproveEdits`/`DenyEdits` and SysInfo message.
//! - Visuals covered elsewhere; focus here is input→command behavior.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_core::ArcStr;
use ploke_tui::app::App;
use ploke_tui::app::types::Mode;
use ploke_tui::app_state::core::{AppState, ChatState, ConfigState, DiffPreview, EditProposal, EditProposalStatus, RuntimeConfig, SystemState};
use ploke_tui::event_bus::EventBusCaps;
use ploke_tui::EventBus;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{timeout, Duration};

async fn make_app_with_proposals() -> (App, mpsc::Receiver<ploke_tui::app_state::StateCommand>, uuid::Uuid) {
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
        create_proposals: RwLock::new(std::collections::HashMap::new()),
    });

    // Insert a dummy proposal with one file
    let req_id = uuid::Uuid::new_v4();
    let proposal = EditProposal {
        request_id: req_id,
        parent_id: uuid::Uuid::new_v4(),
        call_id: ArcStr::from("example_tool_call:0"),
        proposed_at_ms: chrono::Utc::now().timestamp_millis(),
        edits: vec![],
        files: vec![std::env::current_dir().unwrap().join("Cargo.toml")],
        preview: DiffPreview::UnifiedDiff { text: "diff".into() },
        status: EditProposalStatus::Pending,
    };
    {
        let mut guard = state.proposals.write().await;
        guard.insert(req_id, proposal);
    }

    // Event bus and command channel
    let event_bus = EventBus::new(EventBusCaps::default());
    let (cmd_tx, cmd_rx) = mpsc::channel(64);

    let mut app = App::new(
        cfg.command_style,
        state,
        cmd_tx,
        &event_bus,
        "openai/gpt-4o".to_string(),
    );
    // Open overlay and set mode (mode is irrelevant for overlay keys)
    app.mode = Mode::Insert;
    app.approvals_open();

    (app, cmd_rx, req_id)
}

async fn make_app_with_proposals_and_editor(editor: Option<&str>) -> (App, mpsc::Receiver<ploke_tui::app_state::StateCommand>, uuid::Uuid) {
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
        create_proposals: RwLock::new(std::collections::HashMap::new()),
    });

    if let Some(cmd) = editor {
        let mut guard = state.config.write().await;
        guard.ploke_editor = Some(cmd.to_string());
    }

    // Insert a dummy proposal with one file
    let req_id = uuid::Uuid::new_v4();
    let proposal = EditProposal {
        request_id: req_id,
        parent_id: uuid::Uuid::new_v4(),
        call_id: ArcStr::from("example_tool_call:0"),
        proposed_at_ms: chrono::Utc::now().timestamp_millis(),
        edits: vec![],
        files: vec![std::env::current_dir().unwrap().join("Cargo.toml")],
        preview: DiffPreview::UnifiedDiff { text: "diff".into() },
        status: EditProposalStatus::Pending,
    };
    {
        let mut guard = state.proposals.write().await;
        guard.insert(req_id, proposal);
    }

    // Event bus and command channel
    let event_bus = EventBus::new(EventBusCaps::default());
    let (cmd_tx, cmd_rx) = mpsc::channel(64);

    let mut app = App::new(
        cfg.command_style,
        state,
        cmd_tx,
        &event_bus,
        "openai/gpt-4o".to_string(),
    );
    // Open overlay and set mode (mode is irrelevant for overlay keys)
    app.mode = Mode::Insert;
    app.approvals_open();

    (app, cmd_rx, req_id)
}

#[tokio::test]
async fn approvals_overlay_approve_and_deny_send_commands() {
    let (mut app, mut rx, req_id) = make_app_with_proposals().await;

    // Approve via Enter
    app.push_test_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    // Expect an ApproveEdits command
    let cmd = timeout(Duration::from_secs(30), rx.recv())
        .await
        .expect("approve command timed out")
        .expect("expected a command");
    match cmd {
        ploke_tui::app_state::StateCommand::ApproveEdits { request_id } => {
            assert_eq!(request_id, req_id);
        }
        other => panic!("unexpected command: {:?}", other),
    }

    // Reopen overlay and Deny via 'n'
    app.approvals_open();
    app.push_test_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    let cmd = timeout(Duration::from_secs(30), rx.recv())
        .await
        .expect("deny command timed out")
        .expect("expected a command");
    match cmd {
        ploke_tui::app_state::StateCommand::DenyEdits { request_id } => {
            assert_eq!(request_id, req_id);
        }
        other => panic!("unexpected command: {:?}", other),
    }
}

#[tokio::test]
async fn approvals_overlay_open_in_editor_without_editor_emits_sysinfo() {
    let (mut app, mut rx, _req_id) = make_app_with_proposals().await;

    // Trigger open-in-editor key 'o'
    app.approvals_open();
    app.push_test_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));

    // Expect an AddMessageImmediate SysInfo command as guidance
    let cmd = rx.recv().await.expect("expected a command");
    match cmd {
        ploke_tui::app_state::StateCommand::AddMessageImmediate { msg, kind, .. } => {
            assert!(msg.contains("No editor configured"));
            assert_eq!(kind, ploke_tui::chat_history::MessageKind::SysInfo);
        }
        other => panic!("unexpected command: {:?}", other),
    }
}

/// When an editor is configured by the user, pressing 'o' in the Approvals overlay
/// should attempt to spawn the editor non-blockingly, and should not emit the
/// "No editor configured" SysInfo guidance message.
///
/// Notes on behavior:
/// - The editor command is resolved from config `ploke_editor` or env `PLOKE_EDITOR`.
/// - The spawn is best-effort and non-blocking; failures are currently silent to avoid blocking UI.
/// - Args are formatted as `{path}` or `{path}:{line}` when line info is available in future.
#[tokio::test]
async fn approvals_overlay_open_in_editor_with_editor_smoke() {
    let (mut app, mut rx, _req_id) = make_app_with_proposals_and_editor(Some("true")).await;

    app.push_test_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));

    // Expect no guidance SysInfo ("No editor configured...") message.
    // Use a small timeout; the spawn is non-blocking and this should not emit any command.
    let res = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
    if let Ok(Some(ploke_tui::app_state::StateCommand::AddMessageImmediate { msg, .. })) = res {
        assert!(
            !msg.contains("No editor configured"),
            "unexpected guidance message with editor configured"
        );
    }
}
