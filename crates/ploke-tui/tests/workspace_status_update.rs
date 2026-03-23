use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex as StdMutex, OnceLock};

use cozo::DataValue;
use ploke_db::Database;
use ploke_embed::cancel_token::CancellationToken;
use ploke_embed::indexer::{EmbeddingProcessor, IndexerTask};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;
use ploke_test_utils::workspace_root;
use ploke_tui as tui;
use ploke_tui::app_state::IndexTargetDir;
use tokio::sync::{Mutex, RwLock};

use tui::app_state::handlers::indexing::index_workspace;
use tui::app_state::{
    AppState, ChatState, ConfigState, RuntimeConfig, SystemState, workspace_status_for_test,
    workspace_update_for_test,
};
use tui::chat_history::ChatHistory;
use tui::event_bus::{EventBus, EventBusCaps};

fn fixture_lock() -> &'static StdMutex<()> {
    static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| StdMutex::new(()))
}

struct FileRestoreGuard {
    path: PathBuf,
    original: String,
}

impl FileRestoreGuard {
    fn new(path: PathBuf) -> Self {
        let original = std::fs::read_to_string(&path).expect("read original fixture file");
        Self { path, original }
    }
}

impl Drop for FileRestoreGuard {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.path, &self.original);
    }
}

fn build_state() -> Arc<AppState> {
    let db = Arc::new(Database::init_with_schema().expect("init db"));
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        EmbeddingProcessor::new_mock(),
    ));
    let io_handle = IoManagerHandle::new();
    let (index_cancellation_token, index_cancel_handle) = CancellationToken::new();
    let indexer_task = IndexerTask::new(
        Arc::clone(&db),
        io_handle.clone(),
        Arc::clone(&embedder),
        index_cancellation_token,
        index_cancel_handle,
        None,
    );
    Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig::default()),
        system: SystemState::default(),
        indexing_state: RwLock::new(None),
        indexer_task: Some(Arc::new(indexer_task)),
        indexing_control: Arc::new(Mutex::new(None)),
        db,
        embedder,
        io_handle,
        proposals: RwLock::new(HashMap::new()),
        create_proposals: RwLock::new(HashMap::new()),
        rag: None,
        budget: TokenBudget::default(),
    })
}

fn function_has_embedding(
    db: &Database,
    embedding_set: &ploke_core::embeddings::EmbeddingSet,
    function_name: &str,
) -> bool {
    let vec_rel = embedding_set.rel_name.clone();
    let script = format!(
        r#"?[name] := *function {{ id, name @ 'NOW' }},
            *{vec_rel} {{ node_id: id @ 'NOW' }},
            name = "{function_name}""#
    );
    let query = db.raw_query(&script).expect("query function embeddings");
    !query.rows.is_empty()
}

fn function_node_id(db: &Database, function_name: &str) -> uuid::Uuid {
    let script = format!(r#"?[id] := *function {{ id, name @ 'NOW' }}, name = "{function_name}""#);
    let query = db.raw_query(&script).expect("query function id");
    match query
        .rows
        .first()
        .and_then(|row| row.first())
        .expect("function row present")
    {
        DataValue::Uuid(wrapper) => wrapper.0,
        other => panic!("expected uuid for function id, got {other:?}"),
    }
}

/// A pass here proves workspace status computes freshness over all loaded
/// member crates, not just the focused crate, and that workspace update
/// refreshes stale members without dropping embeddings from untouched members.
#[tokio::test]
async fn workspace_status_and_update_operate_per_loaded_crate() {
    let _lock = fixture_lock().lock().unwrap_or_else(|e| e.into_inner());
    let repo_root = workspace_root();
    let workspace_root = repo_root.join("tests/fixture_workspace/ws_fixture_01");
    let changed_member = workspace_root.join("member_root/src/lib.rs");
    let _restore_guard = FileRestoreGuard::new(changed_member.clone());

    let state = build_state();
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    index_workspace(
        &state,
        &event_bus,
        Some(IndexTargetDir::new(workspace_root.clone())),
        true,
    )
    .await;

    let active_set = state
        .db
        .with_active_set(|set| set.clone())
        .expect("active embedding set");
    let nested_value_id = function_node_id(&state.db, "nested_value");
    let dims = state.embedder.dimensions().expect("mock embedder dims");
    state
        .db
        .update_embeddings_batch(vec![(nested_value_id, vec![0.25; dims])])
        .expect("seed unchanged member embedding");
    assert!(
        function_has_embedding(&state.db, &active_set, "nested_value"),
        "seeded unchanged member embedding should be present before workspace update"
    );

    let changed = std::fs::read_to_string(&changed_member)
        .expect("read changed member file")
        .replace("41", "42");
    std::fs::write(&changed_member, changed).expect("write changed member file");

    workspace_status_for_test(&state, &event_bus)
        .await
        .expect("workspace status");

    let mut freshness = state.system.workspace_freshness_for_test().await;
    freshness.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        freshness.len(),
        2,
        "both loaded workspace members must be tracked"
    );
    assert_eq!(
        freshness,
        vec![
            (
                workspace_root.join("member_root"),
                tui::app_state::core::WorkspaceFreshness::Stale,
            ),
            (
                workspace_root.join("nested/member_nested"),
                tui::app_state::core::WorkspaceFreshness::Fresh,
            ),
        ]
    );

    workspace_update_for_test(&state, &event_bus)
        .await
        .expect("workspace update");

    let mut refreshed = state.system.workspace_freshness_for_test().await;
    refreshed.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        refreshed,
        vec![
            (
                workspace_root.join("member_root"),
                tui::app_state::core::WorkspaceFreshness::Fresh,
            ),
            (
                workspace_root.join("nested/member_nested"),
                tui::app_state::core::WorkspaceFreshness::Fresh,
            ),
        ]
    );
    assert!(
        function_has_embedding(&state.db, &active_set, "nested_value"),
        "workspace update must preserve embeddings for unchanged member crates"
    );
}

/// A pass here proves workspace status surfaces manifest drift instead of
/// silently absorbing added or removed members.
#[tokio::test]
async fn workspace_status_reports_workspace_member_drift() {
    let _lock = fixture_lock().lock().unwrap_or_else(|e| e.into_inner());
    let repo_root = workspace_root();
    let workspace_root = repo_root.join("tests/fixture_workspace/ws_fixture_01");
    let manifest_path = workspace_root.join("Cargo.toml");
    let _restore_guard = FileRestoreGuard::new(manifest_path.clone());

    let state = build_state();
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    index_workspace(
        &state,
        &event_bus,
        Some(IndexTargetDir::new(workspace_root.clone())),
        true,
    )
    .await;

    let manifest = std::fs::read_to_string(&manifest_path).expect("read workspace manifest");
    let changed = manifest.replace(
        "members = [\"member_root\", \"nested/member_nested\"]",
        "members = [\"member_root\"]",
    );
    std::fs::write(&manifest_path, changed).expect("write workspace manifest drift");

    workspace_status_for_test(&state, &event_bus)
        .await
        .expect("workspace status with drift");

    let chat = state.chat.read().await;
    let rendered = chat
        .messages
        .values()
        .map(|msg| msg.content.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Drift: removed workspace members require re-index"),
        "workspace status output should surface member drift: {rendered}"
    );
    assert!(
        rendered.contains("nested/member_nested"),
        "workspace status output should identify the drifted member: {rendered}"
    );
}
