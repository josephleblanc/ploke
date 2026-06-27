use std::{collections::HashMap, path::PathBuf, sync::Arc};

use ploke_core::ArcStr;
use ploke_db::{Database, helpers::graph_resolve_exact};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::{RagConfig, RagService, TokenBudget};
use ploke_test_utils::{setup_db_full_multi_embedding, workspace_root};
use ploke_tui::{
    EventBus,
    app_state::{
        SystemStatus,
        core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState},
    },
    chat_history::ChatHistory,
    event_bus::EventBusCaps,
    tools::Ctx,
    user_config::UserConfig,
};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub(crate) struct CallGraphToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner: Uuid,
}

impl CallGraphToolFixture {
    pub(crate) async fn new() -> Self {
        let db = Arc::new(Database::new(
            setup_db_full_multi_embedding("fixture_call_graph").expect("fixture_call_graph db"),
        ));
        let crate_root = workspace_root().join("tests/fixture_crates/fixture_call_graph");
        let module_path = vec!["crate".to_string()];
        let file_path = crate_root.join("src/lib.rs");
        let owner = graph_resolve_exact(
            db.as_ref(),
            "function",
            file_path.as_path(),
            &module_path,
            "call_crate_local_target",
        )
        .expect("resolve call_crate_local_target")
        .pop()
        .expect("call_crate_local_target row")
        .id;
        assert!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project node proof facts")
                >= 3,
            "call_crate_local_target should project node-scoped proof rows"
        );

        let cfg = UserConfig::default();
        let runtime_cfg = RuntimeConfig::from(cfg.clone());
        let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            cfg.load_embedding_processor().expect("embedder"),
        ));
        let io_handle = IoManagerHandle::new();
        let rag = Arc::new(
            RagService::new_full(
                Arc::clone(&db),
                Arc::clone(&embedder),
                io_handle.clone(),
                RagConfig::default(),
            )
            .expect("rag service"),
        );
        assert!(
            !rag.call_context_degraded(),
            "fixture_call_graph should expose call context"
        );
        assert!(
            !rag.proof_context_degraded(),
            "projected fixture_call_graph facts should expose proof context"
        );

        let state = Arc::new(AppState {
            chat: ChatState::new(ChatHistory::new()),
            config: ConfigState::new(runtime_cfg),
            system: SystemState::new(SystemStatus::new(None)),
            indexing_state: RwLock::new(None),
            indexer_task: None,
            indexing_control: Arc::new(Mutex::new(None)),
            db,
            embedder,
            io_handle,
            proposals: RwLock::new(HashMap::new()),
            create_proposals: RwLock::new(HashMap::new()),
            rag: Some(rag),
            budget: TokenBudget::default(),
        });
        state
            .system
            .set_crate_focus_for_test(crate_root.clone())
            .await;

        Self {
            state,
            file_path,
            owner,
        }
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        Ctx {
            state: Arc::clone(&self.state),
            event_bus: Arc::new(EventBus::new(EventBusCaps::default())),
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: ArcStr::from(call_id),
        }
    }
}
