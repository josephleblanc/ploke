use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

use cozo::DataValue;
use ploke_core::ArcStr;
use ploke_db::{
    Database,
    helpers::graph_resolve_exact,
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
    to_uuid,
};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::{RagConfig, RagService, TokenBudget};
use ploke_test_utils::{
    CORPUS_AXUM_CALL_GRAPH, fresh_backup_fixture_db, setup_db_full_multi_embedding, workspace_root,
};
use ploke_tui::{
    EventBus,
    app_state::{
        SystemStatus,
        core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState},
    },
    chat_history::ChatHistory,
    event_bus::EventBusCaps,
    tools::{Ctx, ToolUiPayload},
    user_config::UserConfig,
};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub(crate) struct CallGraphToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
}

pub(crate) struct AxumBodyEmptyToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<Uuid>,
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
        let target = graph_resolve_exact(
            db.as_ref(),
            "function",
            file_path.as_path(),
            &module_path,
            "local_target",
        )
        .expect("resolve local_target")
        .pop()
        .expect("local_target row")
        .id;
        assert!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project node proof facts")
                >= 3,
            "call_crate_local_target should project node-scoped proof rows"
        );
        assert!(
            db.project_call_proof_facts_for_node(target, "bd:fixture-call-graph")
                .expect("project target proof facts")
                >= 3,
            "local_target should project target-scoped proof rows"
        );

        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner,
            target,
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

impl AxumBodyEmptyToolFixture {
    pub(crate) async fn new() -> Self {
        let db = Arc::new(
            fresh_backup_fixture_db(&CORPUS_AXUM_CALL_GRAPH).expect("corpus axum call graph db"),
        );
        assert!(
            db.has_call_graph_relations()
                .expect("check call graph relations"),
            "corpus_axum_call_graph should expose call graph relations"
        );

        let target = axum_body_empty_target(&db);
        let callers = db
            .callers_for_target(target.id)
            .expect("Body::empty incoming callers")
            .into_iter()
            .map(|caller| caller.site.owner_id)
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            2,
            "current axum fixture should resolve exactly the two Body::empty caller owners"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum Body::empty proof facts")
                >= callers.len(),
            "Body::empty should project target-scoped proof rows for real-corpus callers"
        );

        let crate_root = target
            .file_path
            .parent()
            .and_then(|src_dir| src_dir.parent())
            .expect("Body::empty file should live under axum-core/src")
            .to_path_buf();
        let state = app_state_with_rag(Arc::clone(&db), crate_root).await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
            callers,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
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

async fn app_state_with_rag(db: Arc<Database>, crate_root: PathBuf) -> Arc<AppState> {
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
        "call graph fixture should expose call context"
    );
    assert!(
        !rag.proof_context_degraded(),
        "projected call graph facts should expose proof context"
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
    state.system.set_crate_focus_for_test(crate_root).await;
    state
}

struct MethodTarget {
    id: Uuid,
    file_path: PathBuf,
    module_path: Vec<String>,
}

fn axum_body_empty_target(db: &Database) -> MethodTarget {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from("empty"));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *method {{ id, name: $name, body @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query_params(&script, params)
        .expect("query axum Body::empty target");
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            body_key(data_str(&row[1], "method body")).contains("Empty::new()")
                && data_str(&row[2], "file_path").ends_with("axum-core/src/body.rs")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum-core Body::empty target; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    MethodTarget {
        id: to_uuid(&row[0]).expect("Body::empty uuid"),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn data_str<'a>(value: &'a DataValue, label: &str) -> &'a str {
    match value {
        DataValue::Str(value) => value.as_str(),
        other => panic!("expected {label} string, found {other:?}"),
    }
}

fn data_path(value: &DataValue, label: &str) -> Vec<String> {
    let DataValue::List(parts) = value else {
        panic!("expected {label} list, found {value:?}");
    };
    parts
        .iter()
        .map(|part| data_str(part, label).to_string())
        .collect()
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}

pub(crate) fn assert_incoming_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    target: Uuid,
    label: &str,
) {
    let owner = owner.to_string();
    let target = target.to_string();

    assert!(
        calls.iter().any(|call| {
            call.get("owner_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                && call.get("kind").and_then(serde_json::Value::as_str) == Some("path")
                && call
                    .get("targets")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|targets| {
                        targets.iter().any(|candidate| {
                            candidate
                                .get("target_id")
                                .and_then(serde_json::Value::as_str)
                                == Some(target.as_str())
                        })
                    })
        }),
        "{label} should return incoming caller context for local_target: {calls:#?}"
    );
}

pub(crate) fn assert_body_empty_incoming_context(
    calls: &[serde_json::Value],
    callers: &[Uuid],
    target: Uuid,
    label: &str,
) {
    let target = target.to_string();
    for owner in callers {
        let owner = owner.to_string();
        assert!(
            calls.iter().any(|call| {
                call.get("owner_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                    && call.get("kind").and_then(serde_json::Value::as_str) == Some("path")
                    && call
                        .get("callee")
                        .and_then(|callee| callee.get("path"))
                        .and_then(|path_variant| path_variant.get("path"))
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|path| {
                            path.iter()
                                .filter_map(serde_json::Value::as_str)
                                .eq(["Body", "empty"])
                        })
                    && call
                        .get("targets")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|targets| {
                            targets.iter().any(|candidate| {
                                candidate
                                    .get("target_id")
                                    .and_then(serde_json::Value::as_str)
                                    == Some(target.as_str())
                            })
                        })
            }),
            "{label} should return Body::empty incoming caller context for {owner}: {calls:#?}"
        );
    }
}

pub(crate) fn assert_target_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    target: Uuid,
    label: &str,
) {
    let owner = owner.to_string();
    let target = target.to_string();

    assert!(
        proofs.iter().any(|proof| {
            proof.get("kind").and_then(serde_json::Value::as_str) == Some("call_edge")
                && proof
                    .get("caller_def_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(owner.as_str())
                && proof
                    .get("callee_def_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(target.as_str())
        }),
        "{label} should return target-centered proof rows for local_target callers: {proofs:#?}"
    );
}

pub(crate) fn ui_field<'a>(payload: &'a ToolUiPayload, name: &str) -> &'a str {
    payload
        .fields
        .iter()
        .find(|field| field.name.as_ref() == name)
        .unwrap_or_else(|| panic!("missing UI field {name}: {payload:#?}"))
        .value
        .as_ref()
}
