use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::{Mutex as StdMutex, OnceLock};

use cozo::DataValue;
use ploke_db::Database;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_db::multi_embedding::hnsw_ext::HnswExt;
use ploke_embed::indexer::EmbeddingProcessor;
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;
use ploke_test_utils::workspace_root;
use ploke_tui as tui;
use tokio::sync::{Mutex, RwLock};

use tui::app_state::handlers::indexing::index_workspace;
use tui::app_state::{
    AppState, ChatState, ConfigState, RuntimeConfig, SystemState, workspace_remove_for_test,
};
use tui::chat_history::ChatHistory;
use tui::event_bus::{EventBus, EventBusCaps};
use tui::user_config::WorkspaceRegistry;

fn fixture_lock() -> &'static StdMutex<()> {
    static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| StdMutex::new(()))
}

fn config_home_lock() -> &'static StdMutex<()> {
    static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| StdMutex::new(()))
}

struct XdgConfigHomeGuard {
    old_xdg: Option<String>,
}

impl XdgConfigHomeGuard {
    fn set_to(path: &Path) -> Self {
        let old_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", path);
        }
        Self { old_xdg }
    }
}

impl Drop for XdgConfigHomeGuard {
    fn drop(&mut self) {
        if let Some(old_xdg) = self.old_xdg.take() {
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", old_xdg);
            }
        } else {
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }
}

fn build_state() -> Arc<AppState> {
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

/// A pass here proves the first TUI/runtime subset remove flow reuses the
/// namespace-scoped DB primitive and republishes one coherent post-mutation
/// workspace state: surviving membership, valid focus, narrowed path roots,
/// invalidated search readiness, and rewritten registry/snapshot metadata.
#[tokio::test]
async fn workspace_remove_updates_runtime_membership_focus_and_snapshot_metadata() {
    let _fixture_lock = fixture_lock().lock().unwrap_or_else(|e| e.into_inner());
    let _config_lock = config_home_lock().lock().unwrap_or_else(|e| e.into_inner());
    let xdg_dir = tempfile::tempdir().expect("temp xdg dir");
    let _xdg_guard = XdgConfigHomeGuard::set_to(xdg_dir.path());

    let repo_root = workspace_root();
    let workspace_root = repo_root.join("tests/fixture_workspace/ws_fixture_01");
    let member_root = workspace_root.join("member_root");
    let nested_root = workspace_root.join("nested/member_nested");

    let state = build_state();
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    index_workspace(
        &state,
        &event_bus,
        workspace_root.display().to_string(),
        true,
    )
    .await;

    {
        let mut system_guard = state.system.write().await;
        system_guard.set_loaded_workspace(
            workspace_root.clone(),
            vec![member_root.clone(), nested_root.clone()],
            Some(member_root.clone()),
        );
    }

    let active_set = state
        .db
        .with_active_set(|set| set.clone())
        .expect("active embedding set");
    state.db.setup_multi_embedding().expect("setup multi embedding");
    let nested_value_id = function_node_id(&state.db, "nested_value");
    let dims = state.embedder.dimensions().expect("mock embedder dims");
    state
        .db
        .update_embeddings_batch(vec![(nested_value_id, vec![0.25; dims])])
        .expect("seed embedding");
    state
        .db
        .create_embedding_index(&active_set)
        .expect("create hnsw");
    assert!(
        state.db.is_hnsw_index_registered(&active_set).expect("hnsw state"),
        "test setup should start with an active HNSW registration"
    );

    workspace_remove_for_test(&state, &event_bus, member_root.display().to_string())
        .await
        .expect("workspace remove should succeed");

    assert_eq!(
        state.system.loaded_workspace_member_roots_for_test().await,
        vec![nested_root.clone()]
    );
    assert_eq!(state.system.crate_focus_for_test().await, Some(nested_root.clone()));
    assert_eq!(
        state.system.loaded_workspace_root_for_test().await,
        Some(workspace_root.clone())
    );
    assert_eq!(
        state
            .system
            .read()
            .await
            .derive_path_policy(&[])
            .expect("path policy after remove")
            .roots,
        vec![nested_root.clone()]
    );
    assert!(
        !state.db.is_hnsw_index_registered(&active_set).expect("post-remove hnsw state"),
        "workspace remove should leave search availability explicitly invalidated"
    );

    let context_rows = state.db.list_crate_context_rows().expect("crate_context rows");
    assert_eq!(context_rows.len(), 1);
    assert_eq!(context_rows[0].root_path, nested_root.display().to_string());

    let registry = WorkspaceRegistry::load_from_path(&WorkspaceRegistry::default_registry_path())
        .expect("load workspace registry");
    let entry = registry.entries.first().expect("workspace registry entry");
    assert_eq!(entry.workspace_root, workspace_root);
    assert_eq!(entry.member_roots, vec![nested_root.clone()]);
    assert_eq!(entry.focused_root, Some(nested_root.clone()));
    assert!(
        entry.snapshot_file.exists(),
        "subset remove should rewrite the current workspace snapshot"
    );

    let snapshot_db = Database::init_with_schema().expect("staging snapshot db");
    snapshot_db
        .import_backup_with_embeddings(&entry.snapshot_file)
        .expect("import rewritten workspace snapshot");
    let workspace_rows = snapshot_db
        .raw_query(
            r#"?[members] := *workspace_metadata { id, namespace, root_path, resolver, members, exclude, package_version @ 'NOW' }"#,
        )
        .expect("workspace metadata query");
    let members = match workspace_rows
        .rows
        .first()
        .and_then(|row| row.first())
        .expect("workspace metadata row present")
    {
        DataValue::List(values) => values
            .iter()
            .map(|value| {
                value
                    .get_str()
                    .expect("workspace member path should be string")
                    .to_string()
            })
            .collect::<Vec<_>>(),
        other => panic!("expected workspace_metadata.members list, got {other:?}"),
    };
    assert_eq!(members, vec![nested_root.display().to_string()]);

    let chat = state.chat.read().await;
    let rendered = chat
        .messages
        .values()
        .map(|msg| msg.content.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("invalidated active search state"),
        "subset remove should report search invalidation explicitly: {rendered}"
    );
}
