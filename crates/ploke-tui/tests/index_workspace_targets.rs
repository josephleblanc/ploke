use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::{Mutex as StdMutex, OnceLock};
use std::time::Duration;

use tokio::sync::{Mutex, RwLock};

use ploke_db::Database;
use ploke_embed::indexer::EmbeddingProcessor;
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;
use ploke_test_utils::workspace_root;
use ploke_tui as tui;
use tui::app_state::handlers::indexing::index_workspace;
use tui::app_state::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState};
use tui::chat_history::ChatHistory;
use tui::event_bus::{EventBus, EventBusCaps};
use tui::parser::{IndexTargetKind, resolve_index_target};

fn cwd_lock() -> &'static StdMutex<()> {
    static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| StdMutex::new(()))
}

struct CwdGuard {
    prev: PathBuf,
}

impl CwdGuard {
    fn set_to(path: &Path) -> Self {
        let prev = std::env::current_dir().expect("current dir");
        std::env::set_current_dir(path).expect("set current dir");
        Self { prev }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

fn test_state() -> Arc<AppState> {
    let db = Arc::new(Database::init_with_schema().expect("init db"));
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        EmbeddingProcessor::new_mock(),
    ));
    Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig::default()),
        system: SystemState::default(),
        indexing_state: RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(Mutex::new(None)),
        db,
        embedder,
        io_handle: IoManagerHandle::new(),
        proposals: RwLock::new(HashMap::new()),
        create_proposals: RwLock::new(HashMap::new()),
        rag: None,
        budget: TokenBudget::default(),
    })
}

#[tokio::test]
async fn index_workspace_resolves_ancestor_workspace_from_nested_path() {
    let state = test_state();
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_01");
    let nested_src = fixture_workspace_root.join("nested/member_nested/src");

    index_workspace(
        &state,
        &event_bus,
        nested_src.display().to_string(),
        true,
    )
    .await;

    let nested_failure = state.system.read().await.last_parse_failure().cloned();
    assert!(
        nested_failure.is_none(),
        "unexpected parse failure: {:?}",
        nested_failure
    );

    assert_eq!(
        state.system.loaded_workspace_root_for_test().await,
        Some(fixture_workspace_root.clone())
    );
    assert_eq!(
        state.system.loaded_workspace_member_roots_for_test().await,
        vec![
            fixture_workspace_root.join("member_root"),
            fixture_workspace_root.join("nested/member_nested"),
        ]
    );

    let policy_roots = state
        .system
        .read()
        .await
        .derive_path_policy(&[])
        .expect("path policy after workspace index")
        .roots;
    assert_eq!(
        policy_roots,
        vec![
            fixture_workspace_root.join("member_root"),
            fixture_workspace_root.join("nested/member_nested"),
        ]
    );
}

#[tokio::test]
async fn index_workspace_failure_keeps_previous_loaded_workspace_state() {
    let state = test_state();
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let existing_root = workspace_root().join("tests/fixture_crates/fixture_nodes");
    let existing_members = vec![existing_root.clone()];
    let missing_target = tempfile::tempdir()
        .expect("tempdir")
        .path()
        .join("not_a_cargo_target");
    std::fs::create_dir_all(&missing_target).expect("create missing target dir");

    {
        let mut system_guard = state.system.write().await;
        system_guard.set_loaded_workspace(
            existing_root.clone(),
            existing_members.clone(),
            Some(existing_root.clone()),
        );
    }

    index_workspace(
        &state,
        &event_bus,
        missing_target.display().to_string(),
        true,
    )
    .await;

    assert_eq!(
        state.system.loaded_workspace_root_for_test().await,
        Some(existing_root.clone())
    );
    assert_eq!(
        state.system.loaded_workspace_member_roots_for_test().await,
        existing_members
    );

    let last_failure = state
        .system
        .read()
        .await
        .last_parse_failure()
        .cloned()
        .expect("parse failure recorded");
    assert!(last_failure.message.contains("No crate root or workspace root was found"));
}

#[test]
fn resolve_index_target_relative_fixture_path_fails_from_ploke_tui_crate_dir() {
    let _lock = cwd_lock().lock().unwrap_or_else(|e| e.into_inner());
    let repo_root = workspace_root();
    let _guard = CwdGuard::set_to(&repo_root.join("crates/ploke-tui"));

    let err = resolve_index_target(Some(PathBuf::from(
        "tests/fixture_crates/fixture_update_embed",
    )))
    .expect_err("relative path should fail from crate dir");

    let msg = err.to_string();
    assert!(
        msg.contains("Failed to normalize target path"),
        "unexpected error: {msg}"
    );
    assert!(
        msg.contains("crates/ploke-tui/tests/fixture_crates/fixture_update_embed"),
        "error should show the mis-resolved path: {msg}"
    );
}

#[test]
fn resolve_index_target_absolute_fixture_path_succeeds_from_ploke_tui_crate_dir() {
    let _lock = cwd_lock().lock().unwrap_or_else(|e| e.into_inner());
    let repo_root = workspace_root();
    let _guard = CwdGuard::set_to(&repo_root.join("crates/ploke-tui"));
    let fixture_root = repo_root.join("tests/fixture_crates/fixture_update_embed");

    let resolved = resolve_index_target(Some(fixture_root.clone())).expect("absolute path resolves");

    assert_eq!(resolved.kind, IndexTargetKind::Crate);
    assert_eq!(resolved.requested_path, fixture_root);
    assert_eq!(resolved.focused_root, resolved.workspace_root);
    assert_eq!(resolved.member_roots, vec![resolved.focused_root.clone()]);
}

#[tokio::test]
async fn index_workspace_resolution_failure_emits_no_index_status_and_records_parse_failure() {
    let _lock = cwd_lock().lock().unwrap_or_else(|e| e.into_inner());
    let repo_root = workspace_root();
    let _guard = CwdGuard::set_to(&repo_root.join("crates/ploke-tui"));

    let state = test_state();
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let fixture_root = repo_root.join("tests/fixture_crates/fixture_update_embed");
    {
        let mut system_guard = state.system.write().await;
        system_guard.set_focus_from_root(fixture_root.clone());
    }
    let mut index_rx = event_bus.index_subscriber();

    index_workspace(
        &state,
        &event_bus,
        "tests/fixture_crates/fixture_update_embed".to_string(),
        false,
    )
    .await;

    assert!(
        tokio::time::timeout(Duration::from_millis(200), index_rx.recv())
            .await
            .is_err(),
        "resolution failure should not publish indexing status"
    );

    let failure = state
        .system
        .read()
        .await
        .last_parse_failure()
        .cloned()
        .expect("parse failure recorded");
    assert!(failure.message.contains("Failed to normalize target path"));
    assert_eq!(
        state.system.loaded_workspace_root_for_test().await,
        Some(fixture_root)
    );
}
