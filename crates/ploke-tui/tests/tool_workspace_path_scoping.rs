//! `list_dir` / `read_file` path resolution against a loaded multi-member workspace.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use ploke_core::ArcStr;
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;
use ploke_test_utils::{WS_FIXTURE_01_CANONICAL, fresh_backup_fixture_db, workspace_root};
use ploke_tui::{
    EventBus,
    app_state::core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState},
    chat_history::ChatHistory,
    event_bus::EventBusCaps,
    tools::{
        Ctx, Tool,
        list_dir::{ListDir, ListDirParams, ListDirResult},
        ns_read::{NsRead, NsReadParams, NsReadResult},
    },
    user_config::UserConfig,
};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

async fn workspace_fixture_app_state() -> Arc<AppState> {
    let db = Arc::new(fresh_backup_fixture_db(&WS_FIXTURE_01_CANONICAL).expect("fixture db"));
    let cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(cfg.clone());
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));

    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(runtime_cfg),
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
    });

    let ws = workspace_root().join("tests/fixture_workspace/ws_fixture_01");
    let member_a = ws.join("member_root");
    let member_b = ws.join("nested/member_nested");

    {
        let mut g = state.system.write().await;
        g.set_loaded_workspace(
            ws.clone(),
            vec![member_a.clone(), member_b.clone()],
            Some(member_a.clone()),
        );
    }

    let policy = state
        .system
        .read()
        .await
        .derive_path_policy(&[])
        .expect("path policy after load");
    state
        .io_handle
        .update_roots(Some(policy.roots.clone()), Some(policy.symlink_policy))
        .await;

    state
}

fn tool_ctx(state: Arc<AppState>) -> Ctx {
    Ctx {
        state,
        event_bus: Arc::new(EventBus::new(EventBusCaps::default())),
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("ws-path-test"),
    }
}

#[tokio::test]
async fn list_dir_workspace_relative_nested_parent_succeeds() {
    let state = workspace_fixture_app_state().await;

    let params = ListDirParams {
        dir: Cow::Borrowed("nested"),
        include_hidden: Some(false),
        sort: Some(Cow::Borrowed("name")),
        max_entries: None,
    };
    let result = ListDir::execute(params, tool_ctx(state))
        .await
        .expect("list_dir");
    let parsed: ListDirResult = serde_json::from_str(&result.content).expect("parse");
    assert!(parsed.exists);
    assert!(parsed.entries.iter().any(|e| e.name == "member_nested"));
}

#[tokio::test]
async fn list_dir_absolute_non_focused_member_parent_succeeds() {
    let state = workspace_fixture_app_state().await;
    let nested = workspace_root().join("tests/fixture_workspace/ws_fixture_01/nested");

    let params = ListDirParams {
        dir: Cow::Owned(nested.display().to_string()),
        include_hidden: Some(false),
        sort: Some(Cow::Borrowed("name")),
        max_entries: None,
    };
    let result = ListDir::execute(params, tool_ctx(state))
        .await
        .expect("list_dir");
    let parsed: ListDirResult = serde_json::from_str(&result.content).expect("parse");
    assert!(parsed.exists);
    assert!(parsed.entries.iter().any(|e| e.name == "member_nested"));
}

#[tokio::test]
async fn read_file_cross_member_absolute_succeeds() {
    let state = workspace_fixture_app_state().await;
    let lib = workspace_root()
        .join("tests/fixture_workspace/ws_fixture_01/nested/member_nested/src/lib.rs");

    let params = NsReadParams {
        file: Cow::Owned(lib.display().to_string()),
        start_line: None,
        end_line: None,
        max_bytes: Some(4096),
    };
    let result = NsRead::execute(params, tool_ctx(state))
        .await
        .expect("read");
    let parsed: NsReadResult = serde_json::from_str(&result.content).expect("parse");
    assert!(parsed.exists);
    assert!(parsed.content.is_some());
    assert!(!parsed.content.as_ref().unwrap().is_empty());
}

#[tokio::test]
async fn read_file_workspace_relative_member_file_succeeds() {
    let state = workspace_fixture_app_state().await;

    let params = NsReadParams {
        file: Cow::Borrowed("nested/member_nested/src/lib.rs"),
        start_line: None,
        end_line: None,
        max_bytes: Some(4096),
    };
    let result = NsRead::execute(params, tool_ctx(state))
        .await
        .expect("read");
    let parsed: NsReadResult = serde_json::from_str(&result.content).expect("parse");
    assert!(parsed.exists);
}

#[tokio::test]
async fn read_file_outside_workspace_rejects() {
    let state = workspace_fixture_app_state().await;
    let outside = std::env::temp_dir().join("ploke_tool_path_outside_xyz/nope.rs");

    let params = NsReadParams {
        file: Cow::Owned(outside.display().to_string()),
        start_line: None,
        end_line: None,
        max_bytes: Some(1024),
    };
    let err = NsRead::execute(params, tool_ctx(state))
        .await
        .expect_err("should reject");
    let msg = err.to_string();
    assert!(
        msg.contains("outside") || msg.contains("invalid path"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn list_dir_outside_workspace_rejects() {
    let state = workspace_fixture_app_state().await;
    let outside = std::env::temp_dir().join("ploke_list_dir_outside_only");

    let params = ListDirParams {
        dir: Cow::Owned(outside.display().to_string()),
        include_hidden: Some(false),
        sort: Some(Cow::Borrowed("name")),
        max_entries: None,
    };
    let err = ListDir::execute(params, tool_ctx(state))
        .await
        .expect_err("should reject");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid path") || msg.contains("outside"),
        "{msg}"
    );
}
