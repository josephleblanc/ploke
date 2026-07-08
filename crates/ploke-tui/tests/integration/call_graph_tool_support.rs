use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

use cozo::DataValue;
use ploke_core::{
    ArcStr,
    rag_types::{
        CallCalleeInfo, CallContextInfo, CallPathInfo, CallReceiverInfo, CallResolutionKind,
        CallSiteBucketInfo, CallSiteKind, CallStatusKind, CallTargetKind, ProofContextInfo,
    },
};
use ploke_db::{
    CallStatusKind as DbCallStatusKind, Database, ProofGraphStore,
    helpers::{
        graph_resolve_exact, graph_resolve_exact_call_body_owner,
        graph_resolve_exact_call_body_owner_for_parent,
    },
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
    to_uuid,
};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::{RagConfig, RagService, TokenBudget};
use ploke_test_utils::{
    AXUM_OPAQUE_FUTURE_SUMMARY_ID, AXUM_REQUEST_BUILDER_SUMMARY_ID, AXUM_ROUTING_POST_SUMMARY_ID,
    CORPUS_AXUM_CALL_GRAPH, CORPUS_CHRONO_CALL_GRAPH, CORPUS_MEMCHR_CALL_GRAPH,
    axum_body_empty_dependency_record, axum_dependency_record,
    axum_handler_async_block_poll_resume_blocker, axum_opaque_future_boundary_id,
    axum_opaque_future_macro_summary_records, axum_request_builder_summary_records,
    axum_routing_post_boundary_id, axum_routing_post_macro_summary_records,
    fresh_backup_fixture_db, setup_db_full_multi_embedding, workspace_root,
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
use serde_json::json;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

#[path = "call_graph_tool_support/real_corpus_remaining.rs"]
mod real_corpus_remaining;
pub(crate) use real_corpus_remaining::*;

#[path = "call_graph_tool_support/targetless.rs"]
mod targetless;
pub(crate) use targetless::*;

#[path = "call_graph_tool_support/shared_matrix.rs"]
mod shared_matrix;
pub(crate) use shared_matrix::*;

#[path = "call_graph_tool_support/fixture_callable.rs"]
mod fixture_callable;
pub(crate) use fixture_callable::*;

#[path = "call_graph_tool_support/fixture_receiver.rs"]
mod fixture_receiver;
pub(crate) use fixture_receiver::*;

pub(crate) struct CallGraphToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
}

pub(crate) struct LocalItemToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

pub(crate) struct AxumHandlerAsyncBlockToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

pub(crate) struct AxumBodyEmptyToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
    pub(crate) dependency_root_site: Uuid,
}

pub(crate) struct AxumParseAttrsToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
}

pub(crate) struct AxumJsonFromBytesToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
}

pub(crate) struct AxumBoxedIntoRouteToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
}

pub(crate) struct AxumRunUiTestsToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
}

pub(crate) struct ChronoAliasConstructorToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
}

pub(crate) struct ChronoNaiveUtcToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedMethodCallSite>,
}

pub(crate) struct AxumExpandWithToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
}

pub(crate) struct AxumErrorHandlingTraitsToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
}

pub(crate) struct AxumHandlerCallToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) caller: ExpectedCallSite,
}

pub(crate) struct AxumRequestExtractPathToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) start_file_path: PathBuf,
    pub(crate) start_module_path: Vec<String>,
    pub(crate) target_file_path: PathBuf,
    pub(crate) target_module_path: Vec<String>,
    pub(crate) start: Uuid,
    pub(crate) intermediate: Uuid,
    pub(crate) target: Uuid,
}

pub(crate) struct AxumFromRequestFreeFunctionPathToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) start: Uuid,
    pub(crate) intermediate: Uuid,
    pub(crate) target: Uuid,
}

pub(crate) struct AxumTaskSpawnEffectToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) spawn_owner: Uuid,
    pub(crate) spawn_site: Uuid,
}

pub(crate) struct AxumAwaitReceiverToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

pub(crate) struct ExpectedCallSite {
    pub(crate) owner: Uuid,
    pub(crate) site: Uuid,
    pub(crate) path: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct ExpectedMethodCallSite {
    pub(crate) owner: Uuid,
    pub(crate) site: Uuid,
    pub(crate) callee: CallCalleeInfo,
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
        ctx_for_state(&self.state, call_id)
    }

    pub(crate) fn seed_extern_c_abs_effect(&self) -> ExpectedCallSite {
        let module_path = vec!["crate".to_string()];
        let owner = graph_resolve_exact(
            self.state.db.as_ref(),
            "function",
            self.file_path.as_path(),
            &module_path,
            "call_extern_c_function",
        )
        .expect("resolve call_extern_c_function")
        .pop()
        .expect("call_extern_c_function row")
        .id;
        let context = self
            .state
            .db
            .call_context_for_owner(owner)
            .expect("call_extern_c_function call context");
        let row = context
            .iter()
            .find(|row| row.site.path.as_ref() == Some(&vec!["abs".to_string()]))
            .unwrap_or_else(|| {
                panic!("call_extern_c_function should expose abs(value): {context:#?}")
            });
        assert_eq!(row.status.status, DbCallStatusKind::External);
        assert!(
            row.targets.is_empty(),
            "abs(value) should stay targetless before tool execution: {row:#?}"
        );

        assert!(
            self.state
                .db
                .project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project extern C proof facts")
                >= 2,
            "call_extern_c_function should project targetless proof rows"
        );
        self.state
            .db
            .upsert_proof_fact_values(&[ploke_test_utils::fixture_extern_c_abs_effect_record(
                row.site.id,
            )])
            .expect("upsert extern C FFI effect seed");
        let effects = self
            .state
            .db
            .call_effects_reachable_from_owner(
                owner,
                ploke_db::CallPathOptions {
                    max_depth: 2,
                    max_paths: 16,
                },
            )
            .expect("reachable extern C FFI effect seed");
        assert!(
            effects
                .iter()
                .any(|effect| effect.effect_seed_id == "effect:fixture-extern-c-abs"),
            "fixture should prove the FFI effect is reachable before tool execution: {effects:#?}"
        );

        ExpectedCallSite {
            owner,
            site: row.site.id,
            path: vec!["abs".to_string()],
        }
    }
}

impl LocalItemToolFixture {
    pub(crate) async fn axum_path_deserialize_local_impl_method() -> Self {
        let db = axum_call_graph_db();
        let target = axum_call_body_owner_target_by_label(
            db.as_ref(),
            "LocalItem",
            "local_impl_method:deserialize",
            &["crate", "extract", "path"],
            "axum/src/extract/path/mod.rs",
        );
        let exact = graph_resolve_exact_call_body_owner(
            db.as_ref(),
            target.file_path.as_path(),
            &target.module_path,
            "local_impl_method:deserialize",
            "LocalItem",
        )
        .expect("resolve local_impl_method:deserialize owner");
        assert_eq!(
            exact.len(),
            1,
            "local_impl_method:deserialize should be exact-addressable through call_body_owner"
        );
        assert_eq!(exact[0].id, target.id);
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project local item proof facts")
                >= 2,
            "local_impl_method:deserialize should project node-scoped call/proof rows"
        );
        let state =
            axum_state_for_target(Arc::clone(&db), &target, "local_impl_method:deserialize").await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            owner: target.id,
        }
    }

    pub(crate) async fn axum_from_extractor_from_request_parts_local_impl_method() -> Self {
        let db = axum_call_graph_db();
        let item_name = "local_impl_method:from_request_parts";
        let target = axum_call_body_owner_target_by_label_and_parent(
            db.as_ref(),
            "LocalItem",
            item_name,
            "test_from_extractor",
            &["crate", "middleware", "from_extractor", "tests"],
            "axum/src/middleware/from_extractor.rs",
        );
        let unqualified = graph_resolve_exact_call_body_owner(
            db.as_ref(),
            target.file_path.as_path(),
            &target.module_path,
            item_name,
            "LocalItem",
        )
        .expect("resolve ambiguous local_impl_method:from_request_parts owners");
        assert!(
            unqualified.len() > 1,
            "fixture should prove parent_name is needed for repeated local impl labels"
        );
        let exact = graph_resolve_exact_call_body_owner_for_parent(
            db.as_ref(),
            target.file_path.as_path(),
            &target.module_path,
            item_name,
            "LocalItem",
            "test_from_extractor",
        )
        .expect("resolve parent-qualified local_impl_method:from_request_parts owner");
        assert_eq!(
            exact.len(),
            1,
            "parent_name should disambiguate the test_from_extractor local impl method"
        );
        assert!(
            db.project_call_proof_facts_for_node(exact[0].id, "bd:corpus-axum-call-graph")
                .expect("project local impl method proof facts")
                >= 2,
            "local_impl_method:from_request_parts should project node-scoped call/proof rows"
        );
        assert_eq!(exact[0].id, target.id);
        let state = axum_state_for_target(Arc::clone(&db), &target, item_name).await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            owner: target.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumBodyEmptyToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_body_empty_target(&db);
        let callers = db
            .callers_for_target(target.id)
            .expect("Body::empty incoming callers")
            .into_iter()
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .expect("Body::empty caller should carry a path"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            23,
            "current axum fixture should resolve exactly the twenty-three Body::empty caller sites"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum Body::empty proof facts")
                >= callers.len(),
            "Body::empty should project target-scoped proof rows for real-corpus callers"
        );
        let dependency_root_site =
            attach_body_empty_dependency_root_proof(&db, target.id, &callers);
        let state = axum_state_for_target(Arc::clone(&db), &target, "Body::empty").await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
            callers,
            dependency_root_site,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumParseAttrsToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_parse_attrs_target(&db);
        let callers = db
            .callers_for_target(target.id)
            .expect("parse_attrs incoming callers")
            .into_iter()
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .expect("parse_attrs caller should carry a path"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            11,
            "current axum fixture should resolve the eleven parse_attrs caller sites"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum parse_attrs proof facts")
                >= callers.len(),
            "parse_attrs should project target-scoped proof rows for real-corpus callers"
        );
        let state = axum_state_for_target(Arc::clone(&db), &target, "parse_attrs").await;

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
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumJsonFromBytesToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_json_from_bytes_target(&db);
        let callers = db
            .callers_for_target(target.id)
            .expect("Json::from_bytes incoming callers")
            .into_iter()
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .expect("Json::from_bytes caller should carry a path"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            2,
            "current axum fixture should resolve the two Json::from_bytes caller sites"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum Json::from_bytes proof facts")
                >= callers.len(),
            "Json::from_bytes should project target-scoped proof rows for real-corpus callers"
        );
        let state = axum_state_for_target(Arc::clone(&db), &target, "Json::from_bytes").await;

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
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumBoxedIntoRouteToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_boxed_into_route_target(&db);
        let callers = db
            .callers_for_target(target.id)
            .expect("BoxedIntoRoute incoming caller")
            .into_iter()
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .expect("BoxedIntoRoute caller should carry a path"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            3,
            "current axum fixture should resolve the three BoxedIntoRoute constructor callers"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum BoxedIntoRoute proof facts")
                >= callers.len(),
            "BoxedIntoRoute should project target-scoped proof rows for its real-corpus callers"
        );
        let state = axum_state_for_target(Arc::clone(&db), &target, "BoxedIntoRoute").await;

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
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumRunUiTestsToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_run_ui_tests_target(&db);
        let callers = db
            .callers_for_target(target.id)
            .expect("run_ui_tests incoming callers")
            .into_iter()
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .expect("run_ui_tests caller should carry a path"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            5,
            "current axum fixture should resolve the five run_ui_tests caller sites"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum run_ui_tests proof facts")
                >= callers.len(),
            "run_ui_tests should project target-scoped proof rows for real-corpus callers"
        );
        let state = axum_state_for_target(Arc::clone(&db), &target, "run_ui_tests").await;

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
        ctx_for_state(&self.state, call_id)
    }
}

impl ChronoAliasConstructorToolFixture {
    pub(crate) async fn new() -> Self {
        let db = chrono_call_graph_db();
        let target = chrono_local_result_single_target(&db);
        let callers = db
            .callers_for_target(target.id)
            .expect("LocalResult::Single incoming callers")
            .into_iter()
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .expect("MappedLocalTime::Single caller should carry a path"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            12,
            "chrono LocalResult::Single should expose all alias constructor caller rows"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-chrono-call-graph")
                .expect("project LocalResult::Single proof facts")
                >= callers.len(),
            "LocalResult::Single should project target-scoped proof rows for real-corpus callers"
        );
        let crate_root = target
            .file_path
            .parent()
            .and_then(|src_dir| src_dir.parent())
            .unwrap_or_else(|| panic!("chrono LocalResult::Single file should live under src"))
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
        ctx_for_state(&self.state, call_id)
    }
}

impl ChronoNaiveUtcToolFixture {
    pub(crate) async fn new() -> Self {
        let db = chrono_call_graph_db();
        let target = chrono_naive_utc_target(&db);
        let callers = db
            .callers_for_target(target.id)
            .expect("DateTime::naive_utc incoming callers")
            .into_iter()
            .map(|caller| {
                let receiver = match caller.site.receiver {
                    Some(ploke_db::CallReceiver::TryMethodCallResult { method_name }) => {
                        Some(CallReceiverInfo::TryMethodCallResult { method_name })
                    }
                    other => panic!(
                        "DateTime::naive_utc caller should carry TryMethodCallResult(ok_or), got {other:?}"
                    ),
                };
                ExpectedMethodCallSite {
                    owner: caller.site.owner_id,
                    site: caller.site.id,
                    callee: CallCalleeInfo::Method {
                        name: caller
                            .site
                            .method
                            .expect("DateTime::naive_utc caller should carry a method name"),
                        receiver,
                    },
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            2,
            "chrono DateTime::naive_utc should expose the two parsed.rs try-receiver caller rows"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-chrono-call-graph")
                .expect("project DateTime::naive_utc proof facts")
                >= callers.len(),
            "DateTime::naive_utc should project target-scoped proof rows for real-corpus callers"
        );
        let crate_root = target
            .file_path
            .parent()
            .and_then(|src_dir| src_dir.parent())
            .unwrap_or_else(|| panic!("chrono DateTime::naive_utc file should live under src"))
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
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumExpandWithToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target =
            axum_function_target_by_name_and_file(&db, "expand_with", "axum-macros/src/lib.rs");
        let report = db
            .call_impact_for_target(
                target.id,
                ploke_db::CallPathOptions {
                    max_depth: 2,
                    max_paths: 16,
                },
            )
            .expect("expand_with impact report");
        let expected_callers = [
            "derive_from_request",
            "derive_from_request_parts",
            "derive_typed_path",
            "derive_from_ref",
        ];
        assert_eq!(
            report.paths.len(),
            expected_callers.len(),
            "axum fixture should expose one-hop proc-macro impact paths to expand_with: {report:#?}"
        );
        assert_eq!(
            report.callers.len(),
            expected_callers.len(),
            "axum fixture should expose proc-macro callers for expand_with: {report:#?}"
        );
        assert_eq!(
            report.direct_callers.len(),
            expected_callers.len(),
            "axum fixture should expose direct proc-macro callers for expand_with: {report:#?}"
        );
        assert_eq!(
            report.direct_call_sites.len(),
            expected_callers.len(),
            "axum fixture should expose direct proc-macro call sites for expand_with: {report:#?}"
        );
        assert_eq!(
            report.public_callers.len(),
            expected_callers.len(),
            "axum fixture should expose public proc-macro callers for expand_with: {report:#?}"
        );
        for name in expected_callers {
            assert!(
                report.callers.iter().any(|caller| caller.name == name),
                "expand_with impact should include proc-macro caller {name}: {report:#?}"
            );
            assert!(
                report
                    .public_callers
                    .iter()
                    .any(|caller| caller.name == name),
                "expand_with impact should include public proc-macro caller {name}: {report:#?}"
            );
        }
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum expand_with proof facts")
                >= 1,
            "expand_with should project node-scoped proof rows"
        );
        let state = axum_state_for_target(Arc::clone(&db), &target, "expand_with").await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumErrorHandlingTraitsToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target =
            axum_function_target_by_name_and_file(&db, "traits", "axum/src/error_handling/mod.rs");
        let callers = db
            .callers_for_target(target.id)
            .expect("error_handling::traits incoming callers");
        assert!(
            callers.is_empty(),
            "error_handling::traits should have zero persisted source callers: {callers:#?}"
        );
        let uncalled = db.private_uncalled_nodes().expect("private uncalled nodes");
        assert!(
            uncalled.iter().any(|node| node.id == target.id),
            "private uncalled-node helper should include error_handling::traits: {uncalled:#?}"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum error_handling::traits proof facts")
                >= 3,
            "error_handling::traits should project node-scoped proof rows for its outgoing source calls"
        );
        db.upsert_proof_fact_values(&[ploke_test_utils::axum_entrypoint_record(
            "bd:corpus-axum-call-graph",
            target.id,
        )])
        .expect("admit generated test-harness entrypoint summary");
        let state = axum_state_for_target(Arc::clone(&db), &target, "error_handling::traits").await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumHandlerCallToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_handler_call_target(&db);
        let mut callers = db
            .callers_for_target(target.id)
            .expect("Handler::call incoming caller")
            .into_iter()
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .expect("Handler::call caller should carry a path"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            1,
            "current axum fixture should resolve one Handler::call trait-method caller"
        );
        assert_eq!(
            callers[0].path,
            vec!["Handler".to_string(), "call".to_string()],
            "Handler::call caller should preserve the associated path"
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum Handler::call proof facts")
                >= callers.len(),
            "Handler::call should project target-scoped proof rows for its real-corpus caller"
        );
        let state = axum_state_for_target(Arc::clone(&db), &target, "Handler::call").await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
            caller: callers.pop().expect("one Handler::call caller"),
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumRequestExtractPathToolFixture {
    pub(crate) async fn new() -> Self {
        Self::new_with_rag_config(RagConfig::default()).await
    }

    pub(crate) async fn new_with_rag_config(rag_config: RagConfig) -> Self {
        let db = axum_call_graph_db();
        let start = axum_method_target_by_body_and_file(
            &db,
            "extract",
            "self.extract_with_state(&())",
            "axum-core/src/ext_traits/request.rs",
        );
        let intermediate = axum_method_target_by_body_and_file(
            &db,
            "extract_with_state",
            "E::from_request(self, state)",
            "axum-core/src/ext_traits/request.rs",
        );
        let target = axum_trait_method_target_by_name_and_file(
            &db,
            "FromRequest",
            "from_request",
            "axum-core/src/extract/mod.rs",
        );
        let outgoing = db
            .call_paths_from_owner(
                start.id,
                ploke_db::CallPathOptions {
                    max_depth: 2,
                    max_paths: 16,
                },
            )
            .expect("RequestExt::extract outgoing call paths");
        assert!(
            outgoing
                .iter()
                .any(|path| path.end_id == target.id && path.depth == 2),
            "current axum fixture should expose RequestExt::extract -> FromRequest::from_request: {outgoing:#?}"
        );
        for node in [&start, &intermediate, &target] {
            assert!(
                db.project_call_proof_facts_for_node(node.id, "bd:corpus-axum-call-graph")
                    .unwrap_or_else(|err| panic!("project {} proof facts: {err}", node.id))
                    >= 1,
                "call path fixture node {} should project proof rows",
                node.id
            );
        }
        let state = axum_state_for_target_with_rag_config(
            Arc::clone(&db),
            &start,
            "RequestExt::extract",
            rag_config,
        )
        .await;

        Self {
            state,
            start_file_path: start.file_path,
            start_module_path: start.module_path,
            target_file_path: target.file_path,
            target_module_path: target.module_path,
            start: start.id,
            intermediate: intermediate.id,
            target: target.id,
        }
    }

    pub(crate) fn start_module_path_arg(&self) -> String {
        self.start_module_path.join("::")
    }

    pub(crate) fn target_module_path_arg(&self) -> String {
        self.target_module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumFromRequestFreeFunctionPathToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let file_suffix = "axum-macros/src/from_request/mod.rs";
        let start = axum_function_target_by_name_and_file(&db, "expand", file_suffix);
        let intermediate = axum_function_target_by_name_and_file(
            &db,
            "impl_struct_by_extracting_each_field",
            file_suffix,
        );
        let target = axum_function_target_by_name_and_file(&db, "extract_fields", file_suffix);
        let paths = db
            .call_paths_between(
                start.id,
                target.id,
                ploke_db::CallPathOptions {
                    max_depth: 2,
                    max_paths: 128,
                },
            )
            .expect("from_request::expand free-function call paths");
        assert!(
            paths.iter().any(|path| {
                path.start_id == start.id && path.end_id == target.id && path.depth == 2
            }),
            "current axum fixture should expose from_request::expand -> extract_fields: {paths:#?}"
        );
        for node in [&start, &intermediate, &target] {
            assert!(
                db.project_call_proof_facts_for_node(node.id, "bd:corpus-axum-call-graph")
                    .unwrap_or_else(|err| panic!("project {} proof facts: {err}", node.id))
                    >= 1,
                "free-function path fixture node {} should project proof rows",
                node.id
            );
        }
        let state = axum_state_for_target(Arc::clone(&db), &start, "from_request::expand").await;

        Self {
            state,
            file_path: start.file_path,
            module_path: start.module_path,
            start: start.id,
            intermediate: intermediate.id,
            target: target.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumHandlerAsyncBlockToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_call_body_owner_target_by_label_and_parent(
            db.as_ref(),
            "AsyncBlock",
            "async_block",
            "call",
            &["crate", "handler"],
            "axum/src/handler/mod.rs",
        );
        let exact = graph_resolve_exact_call_body_owner_for_parent(
            db.as_ref(),
            target.file_path.as_path(),
            &target.module_path,
            "async_block",
            "AsyncBlock",
            "call",
        )
        .expect("resolve Handler::call async block owner");
        assert_eq!(
            exact.len(),
            1,
            "parent_name should disambiguate the Handler::call async block"
        );
        assert_eq!(exact[0].id, target.id);
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project Handler::call async block proof facts")
                >= 4,
            "Handler::call async block should project self()/into_response() proof rows"
        );
        let context = db
            .call_context_for_owner(target.id)
            .expect("Handler::call async block call context");
        let self_call = context
            .iter()
            .find(|row| {
                row.site.kind == ploke_db::CallSiteKind::Path
                    && row.site.path.as_ref() == Some(&vec!["self".to_string()])
            })
            .unwrap_or_else(|| {
                panic!("Handler::call async block should expose self(): {context:#?}")
            });
        assert_eq!(self_call.status.status, DbCallStatusKind::Unsupported);
        assert!(
            self_call.targets.is_empty(),
            "Handler::call async-block self() should stay targetless: {self_call:#?}"
        );
        let into_response = context
            .iter()
            .find(|row| row.site.method.as_deref() == Some("into_response"))
            .unwrap_or_else(|| {
                panic!("Handler::call async block should expose into_response(): {context:#?}")
            });
        assert_eq!(into_response.status.status, DbCallStatusKind::Unsupported);
        assert!(
            into_response.targets.is_empty(),
            "Handler::call async-block into_response() should stay targetless: {into_response:#?}"
        );
        db.upsert_proof_fact_values(&[
            axum_handler_async_block_poll_resume_blocker(self_call.site.id, "self"),
            axum_handler_async_block_poll_resume_blocker(into_response.site.id, "into_response"),
        ])
        .expect("upsert Handler::call async-block poll/resume blockers");
        let state = axum_state_for_target(Arc::clone(&db), &target, "async_block").await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            owner: target.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumTaskSpawnEffectToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let start = axum_function_target_by_name_and_file(
            &db,
            "deserialize_error_status_codes",
            "axum/src/form.rs",
        );
        let spawn_owner = axum_function_target_by_name_and_file(
            &db,
            "spawn_service",
            "axum/src/test_helpers/test_client.rs",
        );
        let spawn_context = db
            .call_context_for_owner(spawn_owner.id)
            .expect("spawn_service call context");
        let spawn_row = spawn_context
            .iter()
            .find(|row| {
                row.site.path.as_ref().is_some_and(|call_path| {
                    call_path == &["tokio".to_string(), "spawn".to_string()]
                })
            })
            .unwrap_or_else(|| {
                panic!("spawn_service should expose tokio::spawn frontier: {spawn_context:#?}")
            });
        assert_eq!(spawn_row.status.status, DbCallStatusKind::External);
        assert!(
            spawn_row.targets.is_empty(),
            "tokio::spawn should stay targetless before tool execution: {spawn_row:#?}"
        );

        db.upsert_proof_fact_values(&[json!({
            "fact_kind": "effect_seed",
            "schema_version": "ploke-proof-facts.v1",
            "effect_seed_id": "effect:axum-tui-test-client-task-spawn",
            "call_site_id": spawn_row.site.id.to_string(),
            "effect_class": "async_task_spawn",
            "confidence": "source-oracle",
            "blocker_if_unresolved": false,
            "evidence_use": "proof_only"
        })])
        .expect("upsert axum task-spawn effect seed");
        let effects = db
            .call_effects_reachable_from_owner(
                start.id,
                ploke_db::CallPathOptions {
                    max_depth: 3,
                    max_paths: 16,
                },
            )
            .expect("reachable task-spawn effect seed");
        assert!(
            effects
                .iter()
                .any(|effect| effect.effect_seed_id == "effect:axum-tui-test-client-task-spawn"),
            "fixture should prove the task-spawn effect is reachable before tool execution: {effects:#?}"
        );

        let state =
            axum_state_for_target(Arc::clone(&db), &start, "deserialize_error_status_codes").await;

        Self {
            state,
            file_path: start.file_path,
            module_path: start.module_path,
            owner: start.id,
            spawn_owner: spawn_owner.id,
            spawn_site: spawn_row.site.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumAwaitReceiverToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let owner = axum_await_receiver_owner(&db);
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-axum-call-graph")
                .expect("project axum ConnLimiter::accept proof facts")
                >= 2,
            "ConnLimiter::accept should project targetless call-site proof rows"
        );
        let state = axum_state_for_target(Arc::clone(&db), &owner, "ConnLimiter::accept").await;

        Self {
            state,
            file_path: owner.file_path,
            module_path: owner.module_path,
            owner: owner.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

fn ctx_for_state(state: &Arc<AppState>, call_id: &'static str) -> Ctx {
    Ctx {
        state: Arc::clone(state),
        event_bus: Arc::new(EventBus::new(EventBusCaps::default())),
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from(call_id),
    }
}

fn axum_call_graph_db() -> Arc<Database> {
    let db = Arc::new(
        fresh_backup_fixture_db(&CORPUS_AXUM_CALL_GRAPH).expect("corpus axum call graph db"),
    );
    assert!(
        db.has_call_graph_relations()
            .expect("check call graph relations"),
        "corpus_axum_call_graph should expose call graph relations"
    );
    db
}

fn chrono_call_graph_db() -> Arc<Database> {
    let db = Arc::new(
        fresh_backup_fixture_db(&CORPUS_CHRONO_CALL_GRAPH).expect("corpus chrono call graph db"),
    );
    assert!(
        db.has_call_graph_relations()
            .expect("check call graph relations"),
        "corpus_chrono_call_graph should expose call graph relations"
    );
    db
}

fn memchr_call_graph_db() -> Arc<Database> {
    let db = Arc::new(
        fresh_backup_fixture_db(&CORPUS_MEMCHR_CALL_GRAPH).expect("corpus memchr call graph db"),
    );
    assert!(
        db.has_call_graph_relations()
            .expect("check call graph relations"),
        "corpus_memchr_call_graph should expose call graph relations"
    );
    db
}

async fn axum_state_for_target(
    db: Arc<Database>,
    target: &TargetInfo,
    label: &str,
) -> Arc<AppState> {
    axum_state_for_target_with_rag_config(db, target, label, RagConfig::default()).await
}

async fn axum_state_for_target_with_rag_config(
    db: Arc<Database>,
    target: &TargetInfo,
    label: &str,
    rag_config: RagConfig,
) -> Arc<AppState> {
    let crate_root = target
        .file_path
        .parent()
        .and_then(|src_dir| src_dir.parent())
        .unwrap_or_else(|| panic!("{label} file should live under a crate src directory"))
        .to_path_buf();
    app_state_with_rag_config(db, crate_root, rag_config).await
}

async fn app_state_with_rag(db: Arc<Database>, crate_root: PathBuf) -> Arc<AppState> {
    app_state_with_rag_config(db, crate_root, RagConfig::default()).await
}

async fn app_state_with_rag_config(
    db: Arc<Database>,
    crate_root: PathBuf,
    rag_config: RagConfig,
) -> Arc<AppState> {
    let user_cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(user_cfg.clone());
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        user_cfg.load_embedding_processor().expect("embedder"),
    ));
    let io_handle = IoManagerHandle::new();
    let rag = Arc::new(
        RagService::new_full(
            Arc::clone(&db),
            Arc::clone(&embedder),
            io_handle.clone(),
            rag_config,
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

struct TargetInfo {
    id: Uuid,
    file_path: PathBuf,
    module_path: Vec<String>,
}

fn axum_body_empty_target(db: &Database) -> TargetInfo {
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

    TargetInfo {
        id: to_uuid(&row[0]).expect("Body::empty uuid"),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn axum_parse_attrs_target(db: &Database) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from("parse_attrs"));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *function {{ id, name: $name, body, module_id @ 'NOW' }},
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query_params(&script, params)
        .expect("query axum parse_attrs target");
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            body_key(data_str(&row[1], "function body")).contains("parse_args::<T>()")
                && data_str(&row[2], "file_path").ends_with("axum-macros/src/attr_parsing.rs")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum-macros parse_attrs target; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("parse_attrs uuid"),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn axum_json_from_bytes_target(db: &Database) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from("from_bytes"));

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
        .expect("query axum Json::from_bytes target");
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            body_key(data_str(&row[1], "method body"))
                .contains("serde_json::Deserializer::from_slice(bytes)")
                && data_str(&row[2], "file_path").ends_with("axum/src/json.rs")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum Json::from_bytes target; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("Json::from_bytes uuid"),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn axum_boxed_into_route_target(db: &Database) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from("BoxedIntoRoute"));

    let rows = db
        .raw_query_params(
            r#"?[id, file_path, mod_path] :=
                *module { id: module_id, path: mod_path @ 'NOW' },
                *syntax_edge {
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                },
                *struct { id, name: $name @ 'NOW' },
                *file_mod { owner_id: module_id, file_path @ 'NOW' }"#,
            params,
        )
        .expect("query axum BoxedIntoRoute target");
    let matching = rows
        .rows
        .iter()
        .filter(|row| data_str(&row[1], "file_path").ends_with("axum/src/boxed.rs"))
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum BoxedIntoRoute target; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("BoxedIntoRoute uuid"),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn chrono_local_result_single_target(db: &Database) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *enum {{ id: enum_id, name: "LocalResult" @ 'NOW' }},
    *variant {{ id, name: "Single", owner_id: enum_id @ 'NOW' }},
    ancestor[enum_id, module_id],
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query(&script)
        .expect("query chrono LocalResult::Single target");
    let matching = rows
        .rows
        .iter()
        .filter(|row| data_str(&row[1], "file_path").ends_with("src/offset/mod.rs"))
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one chrono LocalResult::Single target; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("LocalResult::Single uuid"),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn chrono_naive_utc_target(db: &Database) -> TargetInfo {
    method_target_by_body_and_file(db, "naive_utc", "self.datetime", "src/datetime/mod.rs")
}

fn axum_run_ui_tests_target(db: &Database) -> TargetInfo {
    axum_function_target_by_name_and_file(db, "run_ui_tests", "axum-macros/src/lib.rs")
}

fn axum_function_target_by_name_and_file(
    db: &Database,
    name: &'static str,
    file_suffix: &'static str,
) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *function {{ id, name: $name, module_id @ 'NOW' }},
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query axum {name} target: {err}"));
    let matching = rows
        .rows
        .iter()
        .filter(|row| data_str(&row[1], "file_path").ends_with(file_suffix))
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum function target {name:?} in {file_suffix:?}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{name} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn axum_handler_call_target(db: &Database) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *method {{ id, name: "call", owner_id: trait_id @ 'NOW' }},
    *trait {{ id: trait_id, name: "Handler" @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query(&script)
        .expect("query axum Handler::call target");
    let matching = rows
        .rows
        .iter()
        .filter(|row| data_str(&row[1], "file_path").ends_with("axum/src/handler/mod.rs"))
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum Handler::call trait target; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("Handler::call uuid"),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn axum_method_target_by_body_and_file(
    db: &Database,
    method_name: &str,
    body_needle: &str,
    file_suffix: &str,
) -> TargetInfo {
    method_target_by_body_and_file(db, method_name, body_needle, file_suffix)
}

fn method_target_by_body_and_file(
    db: &Database,
    method_name: &str,
    body_needle: &str,
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *method {{ id, name: $method_name, body @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("method_name".to_string(), DataValue::from(method_name));

    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query method {method_name}: {err}"));
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            body_key(body).contains(&body_key(body_needle))
                && data_str(&row[2], "file_path").ends_with(file_suffix)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method {method_name:?} in {file_suffix:?} containing {body_needle:?}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("method uuid"),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn axum_trait_method_target_by_name_and_file(
    db: &Database,
    trait_name: &str,
    method_name: &str,
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *trait {{ id: trait_id, name: $trait_name @ 'NOW' }},
    *method {{ id, name: $method_name, owner_id: trait_id @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("trait_name".to_string(), DataValue::from(trait_name));
    params.insert("method_name".to_string(), DataValue::from(method_name));

    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query axum trait method {trait_name}::{method_name}: {err}"));
    let matching = rows
        .rows
        .iter()
        .filter(|row| data_str(&row[1], "file_path").ends_with(file_suffix))
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum trait method {trait_name}::{method_name} in {file_suffix:?}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("axum trait method uuid"),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn axum_await_receiver_owner(db: &Database) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from("accept"));

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
        .expect("query axum ConnLimiter::accept owner");
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            body_key(body).contains("self.sem.clone().acquire_owned().await.unwrap()")
                && data_str(&row[2], "file_path").ends_with("axum/src/serve/listener.rs")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum ConnLimiter::accept owner; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("ConnLimiter::accept uuid"),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn axum_call_body_owner_target_by_label(
    db: &Database,
    owner_kind: &str,
    label: &str,
    module_path: &[&str],
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

parent_anchor[parent_id, mod_id] := *function{{ id: parent_id @ 'NOW' }}, ancestor[parent_id, mod_id]
parent_anchor[parent_id, mod_id] := *macro{{ id: parent_id @ 'NOW' }}, ancestor[parent_id, mod_id]
parent_anchor[parent_id, mod_id] := *method{{ id: parent_id @ 'NOW' }}, ancestor[parent_id, mod_id]
parent_anchor[parent_id, mod_id] := *const{{ id: parent_id @ 'NOW' }}, ancestor[parent_id, mod_id]
parent_anchor[parent_id, mod_id] := *static{{ id: parent_id @ 'NOW' }}, ancestor[parent_id, mod_id]

?[id, file_path, mod_path] :=
    *call_body_owner {{ id, owner_kind: $owner_kind, parent_id, label: $label @ 'NOW' }},
    parent_anchor[parent_id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("owner_kind".to_string(), DataValue::from(owner_kind));
    params.insert("label".to_string(), DataValue::from(label));

    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query axum call_body_owner {label}: {err}"));
    let module_path = module_path
        .iter()
        .map(|part| (*part).to_string())
        .collect::<Vec<_>>();
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            data_str(&row[1], "file_path").ends_with(file_suffix)
                && data_path(&row[2], "module path") == module_path
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum call_body_owner {label:?} in {file_suffix:?} with module path {module_path:?}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn axum_call_body_owner_target_by_label_and_parent(
    db: &Database,
    owner_kind: &str,
    label: &str,
    parent_name: &str,
    module_path: &[&str],
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

parent_anchor[parent_id, parent_name, mod_id] := *function{{ id: parent_id, name: parent_name @ 'NOW' }}, ancestor[parent_id, mod_id]
parent_anchor[parent_id, parent_name, mod_id] := *macro{{ id: parent_id, name: parent_name @ 'NOW' }}, ancestor[parent_id, mod_id]
parent_anchor[parent_id, parent_name, mod_id] := *method{{ id: parent_id, name: parent_name @ 'NOW' }}, ancestor[parent_id, mod_id]
parent_anchor[parent_id, parent_name, mod_id] := *const{{ id: parent_id, name: parent_name @ 'NOW' }}, ancestor[parent_id, mod_id]
parent_anchor[parent_id, parent_name, mod_id] := *static{{ id: parent_id, name: parent_name @ 'NOW' }}, ancestor[parent_id, mod_id]

?[id, file_path, mod_path] :=
    *call_body_owner {{ id, owner_kind: $owner_kind, parent_id, label: $label @ 'NOW' }},
    parent_anchor[parent_id, $parent_name, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("owner_kind".to_string(), DataValue::from(owner_kind));
    params.insert("label".to_string(), DataValue::from(label));
    params.insert("parent_name".to_string(), DataValue::from(parent_name));

    let rows = db.raw_query_params(&script, params).unwrap_or_else(|err| {
        panic!("query axum call_body_owner {label} for parent {parent_name}: {err}")
    });
    let module_path = module_path
        .iter()
        .map(|part| (*part).to_string())
        .collect::<Vec<_>>();
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            data_str(&row[1], "file_path").ends_with(file_suffix)
                && data_path(&row[2], "module path") == module_path
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum call_body_owner {label:?} for parent {parent_name:?} in {file_suffix:?} with module path {module_path:?}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
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
    callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
) {
    assert_expected_path_incoming_context(calls, callers, target, label, "Body::empty");
}

pub(crate) fn assert_body_empty_impact_summary(
    impact: &serde_json::Map<String, serde_json::Value>,
    label: &str,
) {
    let callers = impact_array(impact, "callers", label);
    let test_callers = impact_array(impact, "test_callers", label);
    let non_test_callers = impact_array(impact, "non_test_callers", label);
    let direct_call_sites = impact_array(impact, "direct_call_sites", label);
    let callsite_buckets = impact_array(impact, "callsite_buckets", label);
    let source_files = impact_array(impact, "source_files", label);
    let source_crates = impact_array(impact, "source_crates", label);
    let source_modules = impact_array(impact, "source_modules", label);

    assert_eq!(
        direct_call_sites.len(),
        23,
        "{label} Body::empty impact should expose every current direct callsite: {direct_call_sites:#?}"
    );
    assert!(
        !test_callers.is_empty() && !non_test_callers.is_empty(),
        "{label} Body::empty impact should preserve both test and non-test caller buckets: {impact:#?}"
    );
    assert_eq!(
        test_callers.len() + non_test_callers.len(),
        callers.len(),
        "{label} Body::empty impact test/non-test buckets should partition eventual callers: {impact:#?}"
    );

    let buckets = callsite_buckets
        .iter()
        .map(|bucket| serde_json::from_value::<CallSiteBucketInfo>(bucket.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed Body::empty impact callsite buckets");
    assert!(
        buckets.iter().any(|bucket| {
            bucket.kind == CallSiteKind::Path
                && bucket.relation == CallTargetKind::AssociatedFunction
                && bucket.count == 23
        }),
        "{label} Body::empty impact should summarize the path/associated-function bucket: {buckets:#?}"
    );

    let calls = direct_call_sites
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed Body::empty impact direct callsites");
    let mut path_counts = BTreeMap::<Vec<String>, usize>::new();
    for call in calls {
        let CallCalleeInfo::Path { path } = call.callee else {
            panic!("{label} Body::empty impact callsite should be path-shaped: {call:#?}");
        };
        *path_counts.entry(path).or_default() += 1;
    }
    assert_eq!(
        path_counts,
        BTreeMap::from([
            (vec!["Body".to_string(), "empty".to_string()], 21),
            (vec!["Self".to_string(), "empty".to_string()], 2),
        ]),
        "{label} Body::empty impact should distinguish Body::empty and Self::empty path rows"
    );

    for suffix in [
        "axum-core/src/body.rs",
        "axum-core/src/ext_traits/request.rs",
        "axum/src/middleware/from_fn.rs",
        "axum/src/routing/tests/mod.rs",
    ] {
        assert_source_file_json(source_files, suffix, label);
    }
    assert_source_crate_json(source_crates, "axum-core", label);
    assert_source_crate_json(source_crates, "axum", label);
    for module in [
        &["crate", "body"][..],
        &["crate", "ext_traits", "request"][..],
        &["crate", "middleware", "from_fn"][..],
        &["crate", "routing", "tests"][..],
    ] {
        assert_source_module_json(source_modules, module, label);
    }
}

fn attach_body_empty_dependency_root_proof(
    db: &Database,
    target: Uuid,
    callers: &[ExpectedCallSite],
) -> Uuid {
    let mut records = Vec::new();
    let mut sites = Vec::new();
    for caller in callers {
        let site = caller.site.to_string();
        let source = db
            .proof_source_provenance(&site)
            .unwrap_or_else(|err| panic!("Body::empty source provenance: {err}"))
            .unwrap_or_else(|| panic!("Body::empty should have proof provenance for {site}"));
        if source.source_file.ends_with("axum/src/form.rs") {
            sites.push(caller.site);
            records.push(axum_body_empty_dependency_record(
                "bd:corpus-axum-call-graph",
                caller.site,
                caller.owner,
                target,
            ));
        }
    }
    assert_eq!(
        sites.len(),
        1,
        "Body::empty should identify the direct axum/src/form.rs dependency-root caller site"
    );
    db.upsert_proof_fact_values(&records)
        .expect("insert Body::empty dependency-root proof");
    sites[0]
}

pub(crate) fn assert_body_empty_dependency_root_proof(
    proofs: &[serde_json::Value],
    site: Uuid,
    target: Uuid,
    tool: &str,
) {
    let site = site.to_string();
    let target = target.to_string();
    assert!(
        proofs.iter().any(|proof| {
            proof.get("kind").and_then(serde_json::Value::as_str) == Some("dependency_root")
                && proof
                    .get("call_site_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(site.as_str())
                && proof
                    .get("resolved_def_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(target.as_str())
                && proof.get("target_kind").and_then(serde_json::Value::as_str)
                    == Some("workspace_inherent_method")
                && proof.get("target_name").and_then(serde_json::Value::as_str)
                    == Some("axum_core::body::Body::empty")
                && proof.get("target_root").and_then(serde_json::Value::as_str)
                    == Some("axum-core/src/body.rs")
                && proof.get("status").and_then(serde_json::Value::as_str) == Some("admitted")
        }),
        "{tool} should expose the axum/src/form.rs Body::empty dependency-root proof row: {proofs:#?}"
    );
}

pub(crate) fn assert_parse_attrs_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
) {
    assert_expected_path_incoming_context(calls, callers, target, label, "parse_attrs");
}

fn impact_array<'a>(
    impact: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    label: &str,
) -> &'a [serde_json::Value] {
    impact
        .get(field)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} Body::empty call_impact {field} array: {impact:#?}"))
}

fn assert_source_file_json(files: &[serde_json::Value], suffix: &str, label: &str) {
    assert!(
        files
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|path| path.ends_with(suffix)),
        "{label} Body::empty impact should include source file ending with {suffix:?}: {files:#?}"
    );
}

fn assert_source_crate_json(crates: &[serde_json::Value], expected: &str, label: &str) {
    assert!(
        crates
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|name| name == expected),
        "{label} Body::empty impact should include source crate {expected:?}: {crates:#?}"
    );
}

fn assert_source_module_json(modules: &[serde_json::Value], expected: &[&str], label: &str) {
    assert!(
        modules.iter().any(|module| {
            module.as_array().is_some_and(|actual| {
                actual
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .eq(expected.iter().copied())
            })
        }),
        "{label} Body::empty impact should include source module {expected:?}: {modules:#?}"
    );
}

pub(crate) fn assert_json_from_bytes_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
) {
    assert_expected_path_incoming_context(calls, callers, target, label, "Json::from_bytes");
}

pub(crate) fn assert_expected_path_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
    target_label: &str,
) {
    let target = target.to_string();
    let matching = calls
        .iter()
        .filter(|call| {
            call.get("kind").and_then(serde_json::Value::as_str) == Some("path")
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
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        callers.len(),
        "{label} should return all {target_label} incoming caller-site rows: {calls:#?}"
    );

    for expected in callers {
        let owner = expected.owner.to_string();
        let site = expected.site.to_string();
        assert!(
            matching.iter().any(|call| {
                call.get("owner_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                    && call.get("site_id").and_then(serde_json::Value::as_str)
                        == Some(site.as_str())
                    && call
                        .get("callee")
                        .and_then(|callee| callee.get("path"))
                        .and_then(|path_variant| path_variant.get("path"))
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|path| {
                            path.iter()
                                .filter_map(serde_json::Value::as_str)
                                .eq(expected.path.iter().map(String::as_str))
                        })
            }),
            "{label} should return {target_label} incoming caller site {site}: {calls:#?}"
        );
    }
}

pub(crate) fn assert_task_spawn_effects(
    effects: &[serde_json::Value],
    fixture: &AxumTaskSpawnEffectToolFixture,
    label: &str,
) {
    let spawn_owner = fixture.spawn_owner.to_string();
    let spawn_site = fixture.spawn_site.to_string();
    let effect = effects
        .iter()
        .find(|effect| {
            effect
                .get("effect_seed_id")
                .and_then(serde_json::Value::as_str)
                == Some("effect:axum-tui-test-client-task-spawn")
        })
        .unwrap_or_else(|| {
            panic!("{label} should include the axum task-spawn effect seed: {effects:#?}")
        });

    assert_eq!(
        effect
            .get("effect_class")
            .and_then(serde_json::Value::as_str),
        Some("async_task_spawn")
    );
    assert_eq!(
        effect.get("confidence").and_then(serde_json::Value::as_str),
        Some("source-oracle")
    );
    assert_eq!(
        effect
            .get("blocker_if_unresolved")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert!(
        effect
            .get("blocker_reasons")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|blockers| blockers.is_empty()),
        "{label} non-blocking effect seed should not add blockers: {effect:#?}"
    );
    let owner = fixture.owner.to_string();
    let paths = effect
        .get("paths_to_owner")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} effect should include paths_to_owner: {effect:#?}"));
    let effect_path = paths
        .iter()
        .find(|path| {
            path.get("start_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                && path.get("end_id").and_then(serde_json::Value::as_str)
                    == Some(spawn_owner.as_str())
                && path.get("depth").and_then(serde_json::Value::as_u64) == Some(2)
        })
        .unwrap_or_else(|| {
            panic!("{label} effect should include the resolved path to spawn_service: {effect:#?}")
        });
    let edges = effect_path
        .get("edges")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} effect path should include edges: {effect_path:#?}"));
    assert_eq!(edges.len(), 2, "{label} task-spawn path should be two hops");
    assert_eq!(
        edges[0]
            .get("caller_id")
            .and_then(serde_json::Value::as_str),
        Some(owner.as_str())
    );
    assert_eq!(
        edges[1]
            .get("callee_id")
            .and_then(serde_json::Value::as_str),
        Some(spawn_owner.as_str())
    );
    let call_site = effect
        .get("call_site")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{label} effect should include call_site object: {effect:#?}"));
    assert_eq!(
        call_site
            .get("owner_id")
            .and_then(serde_json::Value::as_str),
        Some(spawn_owner.as_str())
    );
    assert_eq!(
        call_site.get("site_id").and_then(serde_json::Value::as_str),
        Some(spawn_site.as_str())
    );
    assert_eq!(
        call_site.get("status").and_then(serde_json::Value::as_str),
        Some("external")
    );
    assert!(
        call_site
            .get("targets")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|targets| targets.is_empty()),
        "{label} effect callsite should remain targetless: {effect:#?}"
    );
    assert!(
        call_site
            .get("callee")
            .and_then(|callee| callee.get("path"))
            .and_then(|path_variant| path_variant.get("path"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|path| path
                .iter()
                .filter_map(serde_json::Value::as_str)
                .eq(["tokio", "spawn"])),
        "{label} effect callsite should preserve tokio::spawn callee path: {effect:#?}"
    );
}

pub(crate) fn assert_fixture_extern_c_abs_effects(
    effects: &[serde_json::Value],
    expected: &ExpectedCallSite,
    label: &str,
) {
    let owner = expected.owner.to_string();
    let site = expected.site.to_string();
    let effect = effects
        .iter()
        .find(|effect| {
            effect
                .get("effect_seed_id")
                .and_then(serde_json::Value::as_str)
                == Some("effect:fixture-extern-c-abs")
        })
        .unwrap_or_else(|| panic!("{label} should include the extern C FFI effect: {effects:#?}"));

    assert_eq!(
        effect
            .get("effect_class")
            .and_then(serde_json::Value::as_str),
        Some("ffi_boundary")
    );
    assert_eq!(
        effect.get("confidence").and_then(serde_json::Value::as_str),
        Some("fixture-source-oracle")
    );
    assert_eq!(
        effect
            .get("blocker_if_unresolved")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(
        effect
            .get("blocker_reasons")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|blockers| {
                blockers
                    .iter()
                    .any(|reason| reason.as_str() == Some("external_dependency_summary_missing"))
            }),
        "{label} should preserve the external-summary blocker on the FFI effect: {effect:#?}"
    );
    let paths = effect
        .get("paths_to_owner")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} effect should include paths_to_owner: {effect:#?}"));
    assert!(
        paths.is_empty(),
        "{label} direct FFI frontier should not invent a self path: {effect:#?}"
    );
    let call_site = effect
        .get("call_site")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{label} effect should include call_site object: {effect:#?}"));
    assert_eq!(
        call_site
            .get("owner_id")
            .and_then(serde_json::Value::as_str),
        Some(owner.as_str())
    );
    assert_eq!(
        call_site.get("site_id").and_then(serde_json::Value::as_str),
        Some(site.as_str())
    );
    assert_eq!(
        call_site.get("status").and_then(serde_json::Value::as_str),
        Some("external")
    );
    assert!(
        call_site
            .get("targets")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|targets| targets.is_empty()),
        "{label} effect callsite should remain targetless: {effect:#?}"
    );
    assert!(
        call_site
            .get("callee")
            .and_then(|callee| callee.get("path"))
            .and_then(|path_variant| path_variant.get("path"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|path| path
                .iter()
                .filter_map(serde_json::Value::as_str)
                .eq(["abs"])),
        "{label} effect callsite should preserve abs callee path: {effect:#?}"
    );
}

pub(crate) fn assert_chrono_naive_utc_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedMethodCallSite],
    target: Uuid,
    label: &str,
) {
    assert_expected_method_incoming_context(calls, callers, target, label, "DateTime::naive_utc");
}

pub(crate) fn assert_expected_method_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedMethodCallSite],
    target: Uuid,
    label: &str,
    target_label: &str,
) {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.kind == CallSiteKind::Method
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        callers.len(),
        "{label} should return all {target_label} incoming caller-site rows: {calls:#?}"
    );

    for expected in callers {
        let call = matching
            .iter()
            .find(|call| {
                call.owner_id == expected.owner
                    && call.site_id == expected.site
                    && call.callee == expected.callee
            })
            .unwrap_or_else(|| {
                panic!(
                    "{label} should return {target_label} incoming caller site {}: {calls:#?}",
                    expected.site
                )
            });
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1, "{call:#?}");
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Method);
    }
}

pub(crate) fn assert_boxed_into_route_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
) {
    let target = target.to_string();
    for caller in callers {
        let owner = caller.owner.to_string();
        let site = caller.site.to_string();
        let matching = calls
            .iter()
            .filter(|call| {
                call.get("owner_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                    && call.get("site_id").and_then(serde_json::Value::as_str)
                        == Some(site.as_str())
                    && call.get("kind").and_then(serde_json::Value::as_str) == Some("path")
                    && call
                        .get("callee")
                        .and_then(|callee| callee.get("path"))
                        .and_then(|path_variant| path_variant.get("path"))
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|path| {
                            path.iter()
                                .filter_map(serde_json::Value::as_str)
                                .eq(caller.path.iter().map(String::as_str))
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
            })
            .count();
        assert_eq!(
            matching, 1,
            "{label} should return the BoxedIntoRoute constructor incoming caller site {site}: {calls:#?}"
        );
    }
}

pub(crate) fn assert_run_ui_tests_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
) {
    assert_expected_path_incoming_context(calls, callers, target, label, "run_ui_tests");
}

pub(crate) fn assert_handler_call_incoming_context(
    calls: &[serde_json::Value],
    caller: &ExpectedCallSite,
    target: Uuid,
    label: &str,
) {
    assert_expected_path_incoming_context(
        calls,
        std::slice::from_ref(caller),
        target,
        label,
        "Handler::call",
    );
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

pub(crate) fn assert_resolved_callable_param_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    target: Uuid,
    site: Uuid,
    build_domain: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let target = target.to_string();
    let site = site.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.build_domain_id.as_deref() == Some(build_domain)
        }),
        "{tool} should return the callable parameter call_site proof row: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_edge"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.callee_def_id.as_deref() == Some(target.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
        }),
        "{tool} should return the resolved callable parameter call_edge proof row: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
                && proof.resolved_def_id.as_deref() == Some(target.as_str())
        }),
        "{tool} should return the resolved callable parameter call_resolution proof row: {proofs:#?}"
    );
}

pub(crate) fn assert_await_result_unwrap_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    label: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Method
                && call.callee
                    == (CallCalleeInfo::Method {
                        name: "unwrap".to_string(),
                        receiver: Some(CallReceiverInfo::AwaitResult),
                    })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{label} should return exactly one targetless AwaitResult unwrap row: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::Unsupported);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{label} should not fabricate a target for AwaitResult unwrap: {call:#?}"
    );
    call.site_id
}

pub(crate) fn assert_two_hop_call_path(
    paths: &[serde_json::Value],
    start: Uuid,
    intermediate: Uuid,
    target: Uuid,
    label: &str,
) {
    let paths = paths
        .iter()
        .filter_map(|path| serde_json::from_value::<CallPathInfo>(path.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        paths.iter().any(|path| {
            path.start_id == start
                && path.end_id == target
                && path.depth == 2
                && path.edges.len() == 2
                && path.edges[0].caller_id == start
                && path.edges[0].callee_id == intermediate
                && path.edges[1].caller_id == intermediate
                && path.edges[1].callee_id == target
        }),
        "{label} should expose the expected two-hop call path: {paths:#?}"
    );
}

pub(crate) fn assert_call_path_node(
    nodes: &[serde_json::Value],
    id: Uuid,
    canon_suffix: &str,
    file_suffix: &str,
    label: &str,
) {
    let id = id.to_string();
    assert!(
        nodes.iter().any(|node| {
            node.get("id").and_then(serde_json::Value::as_str) == Some(id.as_str())
                && node
                    .get("canon_path")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|canon| canon.ends_with(canon_suffix))
                && node
                    .get("file_path")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|file| file.ends_with(file_suffix))
        }),
        "{label} should include call path node {id} ending with {canon_suffix:?} in {file_suffix:?}: {nodes:#?}"
    );
}

pub(crate) fn assert_await_result_unwrap_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    label: &str,
) {
    let owner = owner.to_string();
    let site_id = site_id.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.build_domain_id.as_deref() == Some("bd:corpus-axum-call-graph")
        }),
        "{label} should return the AwaitResult unwrap call_site proof row: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some("blocked")
                && proof.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "{label} should return the AwaitResult unwrap blocked resolution proof row: {proofs:#?}"
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
