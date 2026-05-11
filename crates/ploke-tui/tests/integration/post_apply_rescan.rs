//! Post-apply rescan
//!
//! Purpose: after approving a proposal, the system should schedule a rescan and
//! surface a SysInfo message indicating so.
//!
//! Approach: stage a minimal real patch proposal and approve it, then assert
//! the chat history includes a SysInfo message about scheduling a rescan.

use std::sync::Arc;

use ploke_core::{ArcStr, PROJECT_NAMESPACE_UUID, file_hash::LargeFilePolicy};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::{Diff, NsWriteSnippetData, PatchApplyOptions};
use ploke_tui::{
    EventBus,
    app_state::core::{
        AppState, ChatState, ConfigState, EditProposal, EditProposalStatus, RuntimeConfig,
        SystemState,
    },
    event_bus::EventBusCaps,
    user_config::MessageVerbosityProfile,
};
use tempfile::TempDir;
use tokio::sync::RwLock;

async fn build_state(profile: MessageVerbosityProfile) -> (Arc<AppState>, Arc<EventBus>) {
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let mut cfg = ploke_tui::user_config::UserConfig::default();
    cfg.default_verbosity = profile;
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
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
    (state, event_bus)
}

async fn seed_and_approve_ns_proposal(state: &Arc<AppState>, event_bus: &Arc<EventBus>) -> TempDir {
    let temp_dir = tempfile::tempdir().expect("temp workspace");
    let file_path = temp_dir.path().join("notes.txt");
    std::fs::write(&file_path, "alpha\nbeta\ngamma\n").expect("seed file");

    let req_id = uuid::Uuid::new_v4();
    let edit = NsWriteSnippetData {
        id: uuid::Uuid::new_v4(),
        file_path: file_path.clone(),
        expected_file_hash: None,
        namespace: PROJECT_NAMESPACE_UUID,
        diff: Diff::from(
            "--- a/notes.txt\n+++ b/notes.txt\n@@ -1,3 +1,3 @@\n alpha\n-beta\n+delta\n gamma\n"
                .to_string(),
        ),
        options: PatchApplyOptions::default(),
        large_file_policy: LargeFilePolicy::Skip,
    };
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
                edits_ns: vec![edit],
                files: vec![file_path],
                preview: ploke_tui::app_state::core::DiffPreview::UnifiedDiff {
                    text: "patch notes.txt".to_string(),
                },
                status: EditProposalStatus::Pending,
                is_semantic: false,
            },
        );
    }

    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        ploke_tui::rag::editing::approve_edits(&state, &event_bus, req_id),
    )
    .await
    .expect("approve_edits timed out");

    temp_dir
}

fn has_scheduled_rescan_message(chat: &ploke_tui::chat_history::ChatHistory) -> bool {
    chat.messages.values().any(|m| {
        m.kind == ploke_tui::chat_history::MessageKind::SysInfo
            && m.content.contains("Scheduled rescan of workspace")
    })
}

#[tokio::test]
async fn approve_emits_rescan_sysinfo_under_default_profile() {
    let (state, event_bus) = build_state(MessageVerbosityProfile::Minimal).await;
    let _temp_dir = seed_and_approve_ns_proposal(&state, &event_bus).await;

    let chat_guard = state.chat.0.read().await;
    let found = has_scheduled_rescan_message(&chat_guard);
    assert!(
        found,
        "expected scheduled rescan SysInfo message to be emitted in chat storage"
    );
}

#[tokio::test]
async fn approve_emits_rescan_sysinfo_under_verbose_profile() {
    let (state, event_bus) = build_state(MessageVerbosityProfile::Verbose).await;
    let _temp_dir = seed_and_approve_ns_proposal(&state, &event_bus).await;

    let chat_guard = state.chat.0.read().await;
    let found = has_scheduled_rescan_message(&chat_guard);
    assert!(
        found,
        "expected scheduled rescan SysInfo message to be emitted in chat storage"
    );
}
