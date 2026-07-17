use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

use cozo::DataValue;
use ploke_core::{
    ArcStr,
    rag_types::{
        AwaitedCallSiteInfo, CallCalleeInfo, CallContextInfo, CallEndpointKind, CallPathInfo,
        CallReachEffectInfo, CallReceiverInfo, CallResolutionKind, CallSiteBucketInfo,
        CallSiteKind, CallStatusKind, CallTargetKind, CrateBoundaryEdgeInfo,
        FuturePollFieldProducerFlowInfo, LocalBindingEdgeInfo, LocalBindingInfo,
        LocalBindingRelationKind, ProofContextInfo, ReturnedCallBindingFlowInfo,
        ReturnedCallSourceKind, ReturnedFutureExecutionFlowInfo, ReturnedFutureFlowInfo,
        SelfFieldAssignmentFlowInfo, SelfFieldParameterFlowInfo, UnsafeBlockCallInfo,
    },
};
use ploke_db::{
    CallReceiver, CallRelationKind as DbCallRelationKind,
    CallResolutionKind as DbCallResolutionKind, CallSiteKind as DbCallSiteKind,
    CallStatusKind as DbCallStatusKind, CallTargetKind as DbCallTargetKind, Database,
    ProofGraphStore,
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
    AXUM_BODY_SIZE_HINT_SUMMARY_ID, AXUM_OPAQUE_FUTURE_SUMMARY_ID, AXUM_REQUEST_BUILDER_SUMMARY_ID,
    AXUM_ROUTE_ONESHOT_SUMMARY_ID, AXUM_ROUTING_GET_SERVICE_SUMMARY_ID,
    AXUM_ROUTING_POST_SUMMARY_ID, AXUM_SERDE_JSON_FROM_SLICE_SUMMARY_ID,
    AXUM_STD_MEM_REPLACE_SUMMARY_ID, CORPUS_AXUM_CALL_GRAPH, CORPUS_CHRONO_CALL_GRAPH,
    CORPUS_MEMCHR_CALL_GRAPH, axum_body_empty_dependency_record,
    axum_body_size_hint_summary_records, axum_dependency_record,
    axum_handler_async_block_poll_resume_blocker, axum_opaque_future_boundary_id,
    axum_opaque_future_macro_summary_records, axum_request_builder_summary_records,
    axum_route_oneshot_summary_records, axum_router_new_dependency_record,
    axum_routing_get_service_boundary_id, axum_routing_get_service_macro_summary_records,
    axum_routing_post_boundary_id, axum_routing_post_macro_summary_records,
    axum_serde_json_from_slice_summary_records, axum_std_mem_replace_summary_records,
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

#[path = "call_graph_tool_support/ifunc.rs"]
mod ifunc;
pub(crate) use ifunc::*;

#[path = "call_graph_tool_support/shared_matrix.rs"]
mod shared_matrix;
pub(crate) use shared_matrix::*;

#[path = "call_graph_tool_support/fixture_callable.rs"]
mod fixture_callable;
pub(crate) use fixture_callable::*;

#[path = "call_graph_tool_support/fixture_receiver.rs"]
mod fixture_receiver;
pub(crate) use fixture_receiver::*;

#[path = "call_graph_tool_support/target_fixtures.rs"]
mod target_fixtures;
pub(crate) use target_fixtures::*;

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

pub(crate) struct MacroExprFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
}

pub(crate) struct AxumHandlerAsyncBlockToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

pub(crate) struct AsyncFutureToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner_name: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) closure: Uuid,
}

pub(crate) struct ReturnedClosureToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner_name: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) closure: Uuid,
}

struct ClosureFixtureParts {
    state: Arc<AppState>,
    file_path: PathBuf,
    module_path: Vec<String>,
    owner: Uuid,
    closure: Uuid,
}

pub(crate) struct AxumCallbackClosureToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
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

pub(crate) struct AxumTapIoConstructorToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

pub(crate) struct AxumTapIoAcceptToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

pub(crate) struct MemchrRunnerSetterToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) field_name: &'static str,
}

pub(crate) struct MemchrRunnerRunToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

#[derive(Debug)]
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

#[derive(Clone)]
pub(crate) struct ExpectedMethodEdge {
    pub(crate) owner: Uuid,
    pub(crate) site: Uuid,
    pub(crate) target: Uuid,
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

    fn extern_c_abs_callsite(&self) -> ExpectedCallSite {
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

        ExpectedCallSite {
            owner,
            site: row.site.id,
            path: vec!["abs".to_string()],
        }
    }

    pub(crate) fn seed_extern_c_abs_effect(&self) -> ExpectedCallSite {
        let expected = self.extern_c_abs_callsite();
        self.state
            .db
            .upsert_proof_fact_values(&[ploke_test_utils::fixture_extern_c_abs_effect_record(
                expected.site,
            )])
            .expect("upsert extern C FFI effect seed");
        let effects = self
            .state
            .db
            .call_effects_reachable_from_owner(
                expected.owner,
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

        expected
    }

    pub(crate) fn seed_extern_c_process_invariant(&self) -> ExpectedCallSite {
        let expected = self.extern_c_abs_callsite();
        self.state
            .db
            .upsert_proof_fact_values(&[json!({
                "fact_kind": "effect_seed",
                "schema_version": "ploke-proof-facts.v1",
                "effect_seed_id": "effect:fixture-tui-extern-c-process-create",
                "call_site_id": expected.site.to_string(),
                "effect_class": "operating_system_process_create",
                "confidence": "fixture-source-oracle",
                "blocker_if_unresolved": true,
                "evidence_use": "proof_only"
            })])
            .expect("upsert extern C process effect seed");
        let findings = self
            .state
            .db
            .call_proof_invariant_findings_for_owner(
                expected.owner,
                ploke_db::CallPathOptions {
                    max_depth: 2,
                    max_paths: 16,
                },
            )
            .expect("reachable extern C process invariant finding");
        assert!(
            findings.iter().any(|finding| {
                finding.invariant == "detached_process_successor_handoff"
                    && finding.status == "blocked"
                    && finding.call_site_id.as_deref() == Some(expected.site.to_string().as_str())
            }),
            "fixture should prove the process invariant is reachable before tool execution: {findings:#?}"
        );

        expected
    }

    pub(crate) fn async_closure_blocker(&self, owner_name: &'static str) -> ExpectedCallSite {
        let module_path = vec!["crate".to_string()];
        let owner = graph_resolve_exact(
            self.state.db.as_ref(),
            "function",
            self.file_path.as_path(),
            &module_path,
            owner_name,
        )
        .unwrap_or_else(|err| panic!("resolve {owner_name}: {err}"))
        .pop()
        .unwrap_or_else(|| panic!("{owner_name} row"))
        .id;
        let context = self
            .state
            .db
            .call_context_for_owner(owner)
            .unwrap_or_else(|err| panic!("{owner_name} call context: {err}"));
        let closure_path = vec!["closure".to_string()];
        let row = context
            .iter()
            .find(|row| row.site.path.as_ref() == Some(&closure_path))
            .unwrap_or_else(|| panic!("{owner_name} should expose closure(): {context:#?}"));
        assert_eq!(row.status.status, DbCallStatusKind::Unsupported);
        assert!(
            row.targets.is_empty(),
            "{owner_name} closure() should stay targetless before async poll/resume proof exists: {row:#?}"
        );

        assert!(
            self.state
                .db
                .project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .unwrap_or_else(|err| panic!("project {owner_name} proof facts: {err}"))
                >= 3,
            "{owner_name} should project targetless proof rows plus the async poll/resume blocker"
        );

        ExpectedCallSite {
            owner,
            site: row.site.id,
            path: closure_path,
        }
    }
}

impl LocalItemToolFixture {
    pub(crate) async fn fixture_macro_generated_local_const() -> Self {
        Self::fixture_macro_local_item(
            "call_const_item_macro_generated_const_initializer",
            "local_const",
        )
        .await
    }

    pub(crate) async fn fixture_macro_generated_local_static() -> Self {
        Self::fixture_macro_local_item(
            "call_static_item_macro_generated_static_initializer",
            "local_static",
        )
        .await
    }

    async fn fixture_macro_local_item(parent_name: &str, item_name: &str) -> Self {
        let db = Arc::new(Database::new(
            setup_db_full_multi_embedding("fixture_call_graph").expect("fixture_call_graph db"),
        ));
        let crate_root = workspace_root().join("tests/fixture_crates/fixture_call_graph");
        let module_path = vec!["crate".to_string()];
        let file_path = crate_root.join("src/lib.rs");
        let exact = graph_resolve_exact_call_body_owner_for_parent(
            db.as_ref(),
            file_path.as_path(),
            &module_path,
            item_name,
            "LocalItem",
            parent_name,
        )
        .unwrap_or_else(|err| panic!("resolve macro-generated {item_name} owner: {err}"));
        assert_eq!(
            exact.len(),
            1,
            "parent_name should disambiguate the macro-generated {item_name} owner"
        );
        assert!(
            db.project_call_proof_facts_for_node(exact[0].id, "bd:fixture-call-graph")
                .unwrap_or_else(|err| {
                    panic!("project macro-generated {item_name} proof facts: {err}")
                })
                >= 1,
            "macro-generated {item_name} should project node-scoped proof rows"
        );
        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            module_path,
            owner: exact[0].id,
        }
    }

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

impl MacroExprFixture {
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
            "call_expr_macro_generated_path_call",
        )
        .expect("resolve call_expr_macro_generated_path_call")
        .pop()
        .expect("call_expr_macro_generated_path_call row")
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
                .expect("project macro expression owner proof facts")
                >= 4,
            "call_expr_macro_generated_path_call should project macro and generated path-call proof rows"
        );
        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            module_path,
            owner,
            target,
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

impl AsyncFutureToolFixture {
    pub(crate) async fn tuple_field() -> Self {
        Self::for_owner("call_awaited_async_closure_future_tuple_field_with_body_call").await
    }

    pub(crate) async fn named_field() -> Self {
        Self::for_owner("call_awaited_async_closure_future_named_field_with_body_call").await
    }

    pub(crate) async fn named_field_alias() -> Self {
        Self::for_owner("call_awaited_async_closure_future_named_field_alias_with_body_call").await
    }

    pub(crate) async fn indexed_array() -> Self {
        Self::for_owner("call_awaited_async_closure_future_indexed_array_with_body_call").await
    }

    pub(crate) async fn returned_async_closure() -> Self {
        Self::for_owner_with_closure_parent(
            "call_awaited_returned_async_closure",
            "make_returned_async_closure",
        )
        .await
    }

    pub(crate) async fn stored_returned_async_closure() -> Self {
        Self::for_owner_with_closure_parent(
            "call_stored_returned_async_closure",
            "make_returned_async_closure",
        )
        .await
    }

    async fn for_owner(owner_name: &'static str) -> Self {
        Self::for_owner_with_closure_parent(owner_name, owner_name).await
    }

    async fn for_owner_with_closure_parent(
        owner_name: &'static str,
        closure_parent_name: &'static str,
    ) -> Self {
        let parts = closure_fixture_parts(
            owner_name,
            closure_parent_name,
            "async_closure",
            "async closure",
        )
        .await;

        Self {
            state: parts.state,
            file_path: parts.file_path,
            module_path: parts.module_path,
            owner_name,
            owner: parts.owner,
            closure: parts.closure,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl ReturnedClosureToolFixture {
    pub(crate) async fn forwarded_returned_closure() -> Self {
        let owner_name = "call_forwarded_returned_closure";
        let parts = closure_fixture_parts(
            owner_name,
            "make_target_closure",
            "closure",
            "forwarded returned closure",
        )
        .await;

        Self {
            state: parts.state,
            file_path: parts.file_path,
            module_path: parts.module_path,
            owner_name,
            owner: parts.owner,
            closure: parts.closure,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

async fn closure_fixture_parts(
    owner_name: &'static str,
    closure_parent_name: &'static str,
    closure_label: &'static str,
    detail: &'static str,
) -> ClosureFixtureParts {
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
        owner_name,
    )
    .unwrap_or_else(|err| panic!("resolve {detail} owner {owner_name}: {err}"))
    .pop()
    .unwrap_or_else(|| panic!("{detail} owner {owner_name}"));
    let exact = graph_resolve_exact_call_body_owner_for_parent(
        db.as_ref(),
        file_path.as_path(),
        &module_path,
        closure_label,
        "Closure",
        closure_parent_name,
    )
    .unwrap_or_else(|err| panic!("resolve {detail} closure owner: {err}"));
    assert_eq!(
        exact.len(),
        1,
        "parent_name should disambiguate the {detail} closure owner"
    );
    assert!(
        db.project_call_proof_facts_for_node(owner.id, "bd:fixture-call-graph")
            .unwrap_or_else(|err| panic!("project {detail} owner proof facts: {err}"))
            >= 3,
        "{detail} owner should project call/proof rows"
    );
    assert!(
        db.project_call_proof_facts_for_node(exact[0].id, "bd:fixture-call-graph")
            .unwrap_or_else(|err| panic!("project {detail} closure proof facts: {err}"))
            >= 3,
        "{detail} closure should project body call/proof rows"
    );

    let state = app_state_with_rag(db, crate_root).await;

    ClosureFixtureParts {
        state,
        file_path,
        module_path,
        owner: owner.id,
        closure: exact[0].id,
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

impl AxumTapIoConstructorToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_tap_io_constructor_target(db.as_ref());
        let mut rag_config = RagConfig::default();
        rag_config.proof_context.enabled = false;
        let state =
            axum_state_for_target_with_rag_config(Arc::clone(&db), &target, "tap_io", rag_config)
                .await;

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

impl AxumTapIoAcceptToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_method_target_by_body_and_file(
            db.as_ref(),
            "accept",
            "(self.tap_fn)(&mut io)",
            "axum/src/serve/listener.rs",
        );
        let mut rag_config = RagConfig::default();
        rag_config.proof_context.enabled = false;
        let state =
            axum_state_for_target_with_rag_config(Arc::clone(&db), &target, "accept", rag_config)
                .await;

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

impl MemchrRunnerSetterToolFixture {
    pub(crate) async fn new_fwd() -> Self {
        let db = memchr_call_graph_db();
        let target = method_target_by_body_and_file(
            db.as_ref(),
            "fwd",
            "self.fwd = Some(Box::new(search));",
            "src/tests/substring/mod.rs",
        );
        let mut rag_config = RagConfig::default();
        rag_config.proof_context.enabled = false;
        let state = source_state_for_target_with_rag_config(
            Arc::clone(&db),
            &target,
            "memchr fwd",
            rag_config,
        )
        .await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            owner: target.id,
            field_name: "fwd",
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl MemchrRunnerRunToolFixture {
    pub(crate) async fn new() -> Self {
        let db = memchr_call_graph_db();
        let target = method_target_by_body_and_file(
            db.as_ref(),
            "run",
            "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
            "src/tests/substring/mod.rs",
        );
        let mut rag_config = RagConfig::default();
        rag_config.proof_context.enabled = false;
        let state = source_state_for_target_with_rag_config(
            Arc::clone(&db),
            &target,
            "memchr run",
            rag_config,
        )
        .await;

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

impl AxumCallbackClosureToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_call_body_owner_target_by_label_and_parent(
            db.as_ref(),
            "Closure",
            "closure",
            "expand_attr_with",
            &["crate"],
            "axum-macros/src/lib.rs",
        );
        let exact = graph_resolve_exact_call_body_owner_for_parent(
            db.as_ref(),
            target.file_path.as_path(),
            &target.module_path,
            "closure",
            "Closure",
            "expand_attr_with",
        )
        .expect("resolve expand_attr_with IIFE closure owner");
        assert_eq!(
            exact.len(),
            1,
            "parent_name should disambiguate the expand_attr_with closure owner"
        );
        assert_eq!(exact[0].id, target.id);
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project expand_attr_with closure proof facts")
                >= 2,
            "expand_attr_with closure should project f(attr, input) proof rows"
        );
        let context = db
            .call_context_for_owner(target.id)
            .expect("expand_attr_with closure call context");
        let callback = context
            .iter()
            .find(|row| {
                row.site.kind == ploke_db::CallSiteKind::Path
                    && row.site.path.as_ref() == Some(&vec!["f".to_string()])
            })
            .unwrap_or_else(|| {
                panic!("expand_attr_with closure should expose f(attr, input): {context:#?}")
            });
        assert_eq!(callback.status.status, DbCallStatusKind::Ambiguous);
        assert_eq!(
            callback.targets.len(),
            2,
            "expand_attr_with closure f(attr, input) should expose both closure candidates: {callback:#?}"
        );
        assert!(
            callback.targets.iter().all(|target| {
                target.relation == ploke_db::CallRelationKind::Closure
                    && target.target_kind == ploke_db::CallTargetKind::Closure
            }),
            "expand_attr_with closure f(attr, input) should expose only closure candidates: {callback:#?}"
        );
        let state = axum_state_for_target(Arc::clone(&db), &target, "closure").await;

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

        db.upsert_proof_fact_values(&[
            json!({
                "fact_kind": "effect_seed",
                "schema_version": "ploke-proof-facts.v1",
                "effect_seed_id": "effect:axum-tui-test-client-task-spawn",
                "call_site_id": spawn_row.site.id.to_string(),
                "effect_class": "async_task_spawn",
                "confidence": "source-oracle",
                "blocker_if_unresolved": false,
                "evidence_use": "proof_only"
            }),
            json!({
                "fact_kind": "effect_policy",
                "schema_version": "ploke-proof-facts.v1",
                "effect_policy_id": "effect-policy:axum-tui-test-client:stored-policy",
                "build_domain_id": "bd:axum-call-graph",
                "definition_id": start.id.to_string(),
                "proof_policy_version": "axum-task-spawn-tool-policy-v1",
                "review_method": "source-oracle-review",
                "scope_of_validity": "axum deserialize_error_status_codes task-spawn tool oracle",
                "allowed_effects": ["ffi_boundary"],
                "invalidation_conditions": "source oracle, fixture hash, or proof policy changes",
                "status": "admitted",
                "evidence_use": "proof_only"
            }),
        ])
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

async fn source_state_for_target_with_rag_config(
    db: Arc<Database>,
    target: &TargetInfo,
    label: &str,
    rag_config: RagConfig,
) -> Arc<AppState> {
    let src_dir = target
        .file_path
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "src"))
        .unwrap_or_else(|| panic!("{label} file should live under a crate src directory"));
    let crate_root = src_dir
        .parent()
        .unwrap_or_else(|| panic!("{label} src directory should have a crate root"))
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

fn axum_tap_io_constructor_target(db: &Database) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from("tap_io"));

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
        .expect("query axum tap_io target");
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            body_key(data_str(&row[1], "method body"))
                .contains(&body_key("TapIo { listener: self, tap_fn, }"))
                && data_str(&row[2], "file_path").ends_with("axum/src/serve/listener.rs")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum ListenerExt::tap_io target; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("tap_io uuid"),
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

fn chrono_parse_internal_target(db: &Database) -> TargetInfo {
    function_target_by_body_and_file(
        db,
        "parse_internal",
        "set(parsed, v)?",
        "src/format/parse.rs",
    )
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

fn function_target_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
    label: &str,
) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    params.insert(
        "module_path".to_string(),
        DataValue::List(
            module_path
                .iter()
                .map(|part| DataValue::from(*part))
                .collect(),
        ),
    );

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
    mod_path == $module_path,
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query function {label}: {err}"));
    let matching = rows.rows.iter().collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one function target for {label}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn function_target_by_body_and_file(
    db: &Database,
    function_name: &str,
    body_needle: &str,
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *function {{ id, name: $function_name, body, module_id @ 'NOW' }},
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("function_name".to_string(), DataValue::from(function_name));

    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query function {function_name}: {err}"));
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
        "expected exactly one function {function_name:?} in {file_suffix:?} containing {body_needle:?}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).expect("function uuid"),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
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

pub(crate) fn assert_body_new_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
) {
    assert_expected_path_incoming_context(calls, callers, target, label, "Body::new");
}

pub(crate) fn assert_body_new_generated_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
) {
    assert_expected_path_incoming_subset(
        calls,
        callers,
        target,
        label,
        "generated Body::from -> Body::new",
    );
}

pub(crate) fn assert_body_new_impact_summary(
    impact: &serde_json::Map<String, serde_json::Value>,
    generated_callers: &[ExpectedCallSite],
    target: Uuid,
    label: &str,
) {
    let direct_call_sites = impact
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("{label} Body::new call_impact direct_call_sites array: {impact:#?}")
        });
    let callsite_buckets = impact
        .get("callsite_buckets")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("{label} Body::new call_impact callsite_buckets array: {impact:#?}")
        });

    assert_body_new_generated_incoming_context(direct_call_sites, generated_callers, target, label);

    let buckets = callsite_buckets
        .iter()
        .map(|bucket| serde_json::from_value::<CallSiteBucketInfo>(bucket.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed Body::new impact callsite buckets");
    assert!(
        buckets.iter().any(|bucket| {
            bucket.kind == CallSiteKind::Path
                && bucket.relation == CallTargetKind::AssociatedFunction
                && bucket.count >= generated_callers.len()
        }),
        "{label} Body::new impact should summarize a path/associated-function bucket covering the generated callers: {buckets:#?}"
    );
}

pub(crate) fn assert_generated_rejection_outgoing_context(
    calls: &[serde_json::Value],
    expected: &[ExpectedMethodEdge],
    label: &str,
) {
    let rows = calls
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed generated rejection call context");

    for expected in expected {
        let call = rows
            .iter()
            .find(|call| call.owner_id == expected.owner && call.site_id == expected.site)
            .unwrap_or_else(|| {
                panic!(
                    "{label} should include generated rejection self-call {}: {rows:#?}",
                    expected.site
                )
            });
        assert_eq!(call.kind, CallSiteKind::Method);
        assert_eq!(call.callee, expected.callee);
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(
            call.targets.len(),
            1,
            "{label} generated rejection self-call should expose one target: {call:#?}"
        );
        assert_eq!(call.targets[0].target_id, expected.target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Method);
    }
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

fn axum_body_from_impl_generated_callers(db: &Database, target: Uuid) -> Vec<ExpectedCallSite> {
    let owners = axum_method_ids_by_name_and_body_substring(
        db,
        "from",
        "Self::new(http_body_util::Full::from(buf))",
    );
    assert_eq!(
        owners.len(),
        7,
        "body_from_impl! should generate exactly seven From<T> for Body::from methods"
    );

    let expected_path = vec!["Self".to_string(), "new".to_string()];
    let mut expected = Vec::new();
    for owner in owners {
        let context = db
            .call_context_for_owner(owner)
            .expect("generated Body::from call context");
        let row = context
            .iter()
            .find(|row| {
                row.site.kind == DbCallSiteKind::Path
                    && row.site.path.as_ref() == Some(&expected_path)
                    && row.targets.iter().any(|candidate| {
                        candidate.target_id == target
                            && candidate.relation == DbCallRelationKind::AssociatedFunction
                    })
            })
            .unwrap_or_else(|| {
                panic!("generated Body::from owner should call Body::new: {context:#?}")
            });
        assert_eq!(row.status.status, DbCallStatusKind::Resolved);
        assert_eq!(
            row.status.resolution,
            Some(DbCallResolutionKind::LocalExact)
        );
        expected.push(ExpectedCallSite {
            owner,
            site: row.site.id,
            path: expected_path.clone(),
        });
    }

    expected
}

fn axum_generated_rejection_self_calls(db: &Database, owner: Uuid) -> Vec<ExpectedMethodEdge> {
    let context = db
        .call_context_for_owner(owner)
        .expect("generated MissingExtension::into_response call context");
    ["status", "body_text"]
        .into_iter()
        .map(|method| {
            let row = context
                .iter()
                .find(|row| {
                    row.site.kind == DbCallSiteKind::Method
                        && row.site.method.as_deref() == Some(method)
                        && row.site.receiver.as_ref() == Some(&CallReceiver::SelfValue)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "generated MissingExtension::into_response should call self.{method}(): {context:#?}"
                    )
                });
            assert_eq!(row.status.status, DbCallStatusKind::Resolved);
            assert_eq!(
                row.status.resolution,
                Some(DbCallResolutionKind::LocalExact)
            );
            assert_eq!(
                row.targets.len(),
                1,
                "generated self.{method}() should resolve to one target: {row:#?}"
            );
            assert_eq!(row.targets[0].relation, DbCallRelationKind::Method);
            assert_eq!(row.targets[0].target_kind, DbCallTargetKind::Method);
            ExpectedMethodEdge {
                owner,
                site: row.site.id,
                target: row.targets[0].target_id,
                callee: CallCalleeInfo::Method {
                    name: method.to_string(),
                    receiver: Some(CallReceiverInfo::SelfValue),
                },
            }
        })
        .collect()
}

fn axum_composite_rejection_delegate_call(db: &Database, owner: Uuid) -> ExpectedMethodEdge {
    let context = db
        .call_context_for_owner(owner)
        .expect("generated QueryRejection::into_response call context");
    let db_receiver = CallReceiver::EnumVariantBinding {
        name: "inner".to_string(),
        enum_path: vec!["Self".to_string()],
        variant_name: "FailedToDeserializeQueryString".to_string(),
        field_index: 0,
    };
    let row = context
        .iter()
        .find(|row| {
            row.site.kind == DbCallSiteKind::Method
                && row.site.method.as_deref() == Some("into_response")
                && row.site.receiver.as_ref() == Some(&db_receiver)
        })
        .unwrap_or_else(|| {
            panic!(
                "generated QueryRejection::into_response should delegate inner.into_response(): {context:#?}"
            )
        });
    assert_eq!(row.status.status, DbCallStatusKind::Resolved);
    assert_eq!(
        row.status.resolution,
        Some(DbCallResolutionKind::LocalExact)
    );
    assert_eq!(
        row.targets.len(),
        1,
        "generated QueryRejection delegate call should resolve to one target: {row:#?}"
    );
    assert_eq!(row.targets[0].relation, DbCallRelationKind::Method);
    assert_eq!(row.targets[0].target_kind, DbCallTargetKind::Method);
    ExpectedMethodEdge {
        owner,
        site: row.site.id,
        target: row.targets[0].target_id,
        callee: CallCalleeInfo::Method {
            name: "into_response".to_string(),
            receiver: Some(CallReceiverInfo::EnumVariantBinding {
                name: "inner".to_string(),
                enum_path: vec!["Self".to_string()],
                variant_name: "FailedToDeserializeQueryString".to_string(),
                field_index: 0,
            }),
        },
    }
}

fn axum_method_ids_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Vec<Uuid> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db
        .raw_query_params(
            r#"?[id, body] :=
                *method { id, name: $name, body @ 'NOW' }"#,
            params,
        )
        .unwrap_or_else(|err| panic!("query methods named {name}: {err}"));
    let normalized_marker = body_key(body_marker);
    rows.rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            body_key(body)
                .contains(&normalized_marker)
                .then(|| row[0].clone())
        })
        .map(|id| to_uuid(&id).unwrap_or_else(|err| panic!("method uuid: {err}")))
        .collect()
}

fn attach_body_empty_dependency_root_proof(
    db: &Database,
    target: Uuid,
    callers: &[ExpectedCallSite],
) -> Vec<Uuid> {
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
        } else if source.source_file.ends_with("axum/src/extract/raw_form.rs") {
            sites.push(caller.site);
            records.push(
                ploke_test_utils::axum_body_empty_reexport_dependency_record(
                    "bd:corpus-axum-call-graph",
                    caller.site,
                    caller.owner,
                    target,
                ),
            );
        }
    }
    assert_eq!(
        sites.len(),
        2,
        "Body::empty should identify the direct form.rs and re-exported raw_form.rs dependency-root caller sites"
    );
    db.upsert_proof_fact_values(&records)
        .expect("insert Body::empty dependency-root proof");
    sites
}

pub(crate) fn assert_body_empty_dependency_root_proof(
    proofs: &[serde_json::Value],
    sites: &[Uuid],
    target: Uuid,
    tool: &str,
) {
    let target = target.to_string();
    for site in sites {
        let site = site.to_string();
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
            "{tool} should expose Body::empty dependency-root proof site {site}: {proofs:#?}"
        );
    }
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

pub(crate) fn assert_from_fn_basic_body_empty_crate_boundary(
    edges: &[CrateBoundaryEdgeInfo],
    owner: Uuid,
    body_empty_target: Uuid,
    label: &str,
) {
    assert!(
        edges
            .iter()
            .all(|edge| edge.caller_crate != edge.callee_crate),
        "{label} crate-boundary rows should only contain cross-crate edges: {edges:#?}"
    );
    let edge = edges
        .iter()
        .find(|edge| edge.edge.caller_id == owner && edge.edge.callee_id == body_empty_target)
        .unwrap_or_else(|| {
            panic!("{label} should expose from_fn::tests::basic -> Body::empty: {edges:#?}")
        });
    assert_eq!(edge.caller_crate, "axum");
    assert_eq!(edge.callee_crate, "axum-core");
    assert_eq!(edge.caller.id, owner);
    assert_eq!(edge.caller.name, "basic");
    assert_eq!(
        edge.caller.module_path,
        vec![
            "crate".to_string(),
            "middleware".to_string(),
            "from_fn".to_string()
        ]
    );
    assert_eq!(edge.callee.id, body_empty_target);
    assert_eq!(edge.callee.name, "empty");
    assert_eq!(
        edge.callee.module_path,
        vec!["crate".to_string(), "body".to_string()]
    );
    assert_eq!(edge.edge.source_kind, CallSiteKind::Path);
    assert_eq!(edge.edge.relation, CallTargetKind::AssociatedFunction);
    assert_eq!(edge.site.owner_id, owner);
    assert_eq!(edge.site.kind, CallSiteKind::Path);
    assert_eq!(edge.site.status, CallStatusKind::Resolved);
    assert_eq!(
        edge.site.callee,
        CallCalleeInfo::Path {
            path: vec!["Body".to_string(), "empty".to_string()]
        }
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

pub(crate) fn assert_no_external_summary_need_for_site(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    assert!(
        needs.iter().all(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                != Some(site.as_str())
        }),
        "{tool} should not expose an external summary need for admitted {label}: {needs:#?}"
    );
}

pub(crate) fn assert_serde_json_summary_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    assert_admitted_external_summary_proof(
        proofs,
        owner,
        site_id,
        ExternalSummaryCase {
            path: &["serde_json", "Deserializer", "from_slice"],
            records: axum_serde_json_from_slice_summary_records,
            summary_id: AXUM_SERDE_JSON_FROM_SLICE_SUMMARY_ID,
        },
        label,
        tool,
    );
}

pub(crate) fn assert_serde_json_surface_measure_effect(
    effects: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = effects
        .iter()
        .filter_map(|effect| serde_json::from_value::<CallReachEffectInfo>(effect.clone()).ok())
        .collect::<Vec<_>>();
    let effect = rows
        .iter()
        .find(|effect| effect.effect_seed_id == "effect:axum-json-parse-surface-measure")
        .unwrap_or_else(|| {
            panic!("{tool} should return the serde_json surface-measure effect for {label}: {effects:#?}")
        });
    assert_eq!(effect.effect_class, "surface_measure");
    assert_eq!(effect.confidence.as_deref(), Some("source-oracle"));
    assert_eq!(effect.blocker_if_unresolved, Some(false));
    assert_eq!(effect.call_site.site_id, site_id);
    assert_eq!(effect.call_site.owner_id, owner);
    assert_eq!(effect.call_site.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "{tool} direct serde_json effect should not need an intermediate path for {label}: {effect:#?}"
    );
    assert!(
        effect.call_site.targets.is_empty(),
        "{tool} surface-measure effects must not fabricate target rows for {label}: {effect:#?}"
    );
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

fn assert_expected_path_incoming_subset(
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
    assert!(
        matching.len() >= callers.len(),
        "{label} should return at least the {target_label} incoming caller-site rows: {calls:#?}"
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

pub(crate) fn assert_task_spawn_policy_violation(
    violations: &[serde_json::Value],
    fixture: &AxumTaskSpawnEffectToolFixture,
    label: &str,
) {
    let spawn_owner = fixture.spawn_owner.to_string();
    let spawn_site = fixture.spawn_site.to_string();
    let violation = violations
        .iter()
        .find(|violation| {
            violation
                .get("effect")
                .and_then(|effect| effect.get("effect_seed_id"))
                .and_then(serde_json::Value::as_str)
                == Some("effect:axum-tui-test-client-task-spawn")
        })
        .unwrap_or_else(|| {
            panic!("{label} should include the axum task-spawn policy violation: {violations:#?}")
        });
    assert_eq!(
        violation
            .get("allowed_effects")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec!["ffi_boundary"]),
        "{label} should preserve the effect-policy allowlist: {violation:#?}"
    );

    let effect = violation
        .get("effect")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| {
            panic!("{label} violation should include effect payload: {violation:#?}")
        });
    assert_eq!(
        effect
            .get("effect_class")
            .and_then(serde_json::Value::as_str),
        Some("async_task_spawn")
    );
    let call_site = effect
        .get("call_site")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{label} effect should include call_site: {effect:#?}"));
    assert_eq!(
        call_site.get("site_id").and_then(serde_json::Value::as_str),
        Some(spawn_site.as_str())
    );
    assert_eq!(
        call_site
            .get("owner_id")
            .and_then(serde_json::Value::as_str),
        Some(spawn_owner.as_str())
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
        "{label} policy violation must not fabricate targets: {call_site:#?}"
    );

    let owner = fixture.owner.to_string();
    let paths = effect
        .get("paths_to_owner")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("{label} policy violation should include paths_to_owner: {effect:#?}")
        });
    assert!(
        paths.iter().any(|path| {
            path.get("start_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                && path.get("end_id").and_then(serde_json::Value::as_str)
                    == Some(spawn_owner.as_str())
                && path.get("depth").and_then(serde_json::Value::as_u64) == Some(2)
        }),
        "{label} policy violation should preserve the path to spawn_service: {effect:#?}"
    );
}

pub(crate) fn assert_process_invariant_findings(
    findings: &[serde_json::Value],
    expected: &ExpectedCallSite,
    label: &str,
) {
    let owner = expected.owner.to_string();
    let site = expected.site.to_string();
    let finding = findings
        .iter()
        .find(|finding| {
            finding.get("invariant").and_then(serde_json::Value::as_str)
                == Some("detached_process_successor_handoff")
                && finding
                    .get("call_site_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(site.as_str())
        })
        .unwrap_or_else(|| {
            panic!("{label} should include the axum process invariant finding: {findings:#?}")
        });
    assert_eq!(
        finding.get("status").and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert!(
        finding
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|reason| reason.contains("external_dependency_summary_missing")),
        "{label} should preserve the external-summary blocker reason: {finding:#?}"
    );
    let call_site = finding
        .get("call_site")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{label} finding should include call_site: {finding:#?}"));
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
        "{label} invariant callsite must remain targetless: {finding:#?}"
    );
    assert!(
        call_site
            .get("callee")
            .and_then(|callee| callee.get("path"))
            .and_then(|path_variant| path_variant.get("path"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|path| {
                path.iter()
                    .filter_map(serde_json::Value::as_str)
                    .eq(expected.path.iter().map(String::as_str))
            }),
        "{label} invariant callsite should preserve the expected callee path: {finding:#?}"
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

pub(crate) fn assert_forwarded_returned_closure_binding_flow(
    flows: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = flows
        .iter()
        .filter_map(|flow| serde_json::from_value::<ReturnedCallBindingFlowInfo>(flow.clone()).ok())
        .collect::<Vec<_>>();
    let path = vec!["make_forwarded_returned_closure".to_string()];
    let matches = rows
        .iter()
        .filter(|flow| {
            flow.caller_id == owner
                && flow.dynamic.path == path
                && flow.dynamic.relation == CallTargetKind::DynamicClosure
                && flow.dynamic.target_kind == CallEndpointKind::Closure
                && flow.producer.path == path
                && flow.binding.source.relation == LocalBindingRelationKind::BindingSourceCallResult
                && flow.binding.source.kind == ReturnedCallSourceKind::Path
        })
        .count();
    assert_eq!(
        matches, 1,
        "{tool} should return exactly one returned-call binding flow for {label}: {flows:#?}"
    );
}

pub(crate) fn assert_initialized_path_local_binding_payload(
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    target: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let binding = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "f"
                && binding.source_kind == "InitializedPath"
                && matches!(
                    binding.source_path.as_deref(),
                    Some([segment]) if segment == "local_target"
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose initialized local binding `f = local_target` for {label}: {bindings:#?}"
            )
        });

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for {label}: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == binding.id
                && edge.target_id == target
                && edge.relation == LocalBindingRelationKind::BindingSourceFunction
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "Function"
        }),
        "{tool} should expose BindingSourceFunction for {label}: {edges:#?}"
    );
}

pub(crate) fn assert_local_function_binding_payload(
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let binding = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LocalFunctionBinding"
                && binding.name == "inner"
                && binding.source_kind == "LocalFunction"
                && binding.source_id.is_some()
                && binding.source_call_kind.is_none()
                && binding.source_path.is_none()
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose local function binding `inner` for {label}: {bindings:#?}")
        });
    let local_item = binding
        .source_id
        .expect("local function binding should carry local-item source id");

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for {label}: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == binding.id
                && edge.target_id == local_item
                && edge.relation == LocalBindingRelationKind::BindingSourceLocalItem
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalItem"
        }),
        "{tool} should expose BindingSourceLocalItem for {label}: {edges:#?}"
    );
}

pub(crate) fn assert_typed_setter_local_binding_payload(
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let binding = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "set"
                && binding.source_kind == "Typed"
                && binding
                    .source_path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["Setter"]))
                && binding.source_id.is_none()
                && binding.source_call_kind.is_none()
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose typed setter binding for {label}: {bindings:#?}")
        });

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for typed setter {label}: {edges:#?}"
    );
}

pub(crate) fn assert_chrono_typed_setter_candidate_payload(
    calls: &[serde_json::Value],
    proofs: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["set".to_string()],
                    }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one ambiguous typed setter row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::Ambiguous);
    assert_eq!(call.resolution, None);
    assert_eq!(
        call.targets.len(),
        21,
        "{tool} should expose every reviewed setter candidate for {label}: {call:#?}"
    );
    let function_count = call
        .targets
        .iter()
        .filter(|target| target.relation == CallTargetKind::Function)
        .count();
    let associated_count = call
        .targets
        .iter()
        .filter(|target| target.relation == CallTargetKind::AssociatedFunction)
        .count();
    assert_eq!(
        function_count, 2,
        "{tool} should preserve the two free setter function candidates for {label}: {call:#?}"
    );
    assert_eq!(
        associated_count, 19,
        "{tool} should preserve the nineteen Parsed::* setter candidates for {label}: {call:#?}"
    );

    let site = call.site_id;
    let site_string = site.to_string();
    let expected = call
        .targets
        .iter()
        .map(|target| target.target_id.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    let resolution = rows
        .iter()
        .find(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_string.as_str())
                && proof.resolution_state.as_deref() == Some("ambiguous")
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose ambiguous setter proof row for {label}: {proofs:#?}")
        });
    let actual = resolution
        .candidate_def_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "{tool} should preserve proof candidate IDs for {label}"
    );
    assert!(
        rows.iter().all(|proof| {
            !(proof.kind == "call_edge"
                && proof.call_site_id.as_deref() == Some(site_string.as_str()))
        }),
        "{tool} should not fabricate call_edge proofs for ambiguous setter {label}: {proofs:#?}"
    );

    site
}

pub(crate) fn assert_method_argument_parameter_local_binding_payload(
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    target: Uuid,
    parameter: Uuid,
    method_call_site: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let binding = rows
        .iter()
        .find(|binding| {
            binding.id == parameter
                && binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "f"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose method parameter binding `f` for {label}: {bindings:#?}")
        });

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for {label}: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == method_call_site
                && edge.target_id == binding.id
                && edge.relation == LocalBindingRelationKind::ArgumentSuppliesParameter
                && edge.source_kind == "Method"
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose method ArgumentSuppliesParameter for {label}: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == binding.id
                && edge.target_id == target
                && edge.relation == LocalBindingRelationKind::BindingSourceFunction
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "Function"
        }),
        "{tool} should expose method parameter BindingSourceFunction for {label}: {edges:#?}"
    );
}

pub(crate) fn assert_result_callback_binding_payload(
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    target: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let parameter = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "f"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose result callback parameter `f` for {label}: {bindings:#?}")
        });

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == parameter.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for {label}: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.target_id == parameter.id
                && edge.relation == LocalBindingRelationKind::ArgumentSuppliesParameter
                && edge.source_kind == "Path"
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose path ArgumentSuppliesParameter for {label}: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == parameter.id
                && edge.target_id == target
                && edge.relation == LocalBindingRelationKind::BindingSourceFunction
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "Function"
        }),
        "{tool} should expose result callback BindingSourceFunction for {label}: {edges:#?}"
    );
}

pub(crate) fn assert_aliased_parameter_local_binding_payload(
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let holder = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "holder"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose holder parameter for {label}: {bindings:#?}")
        });
    let alias = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "alias"
                && binding.source_kind == "ValueAlias"
                && matches!(
                    binding.source_path.as_deref(),
                    Some([segment]) if segment == "holder"
                )
        })
        .unwrap_or_else(|| panic!("{tool} should expose alias binding for {label}: {bindings:#?}"));

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == holder.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for holder in {label}: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == alias.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for alias in {label}: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == alias.id
                && edge.target_id == holder.id
                && edge.relation == LocalBindingRelationKind::BindingAliasesBinding
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose BindingAliasesBinding for {label}: {edges:#?}"
    );
}

pub(crate) fn assert_tap_io_constructor_local_binding_payload(
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    tool: &str,
) {
    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let return_binding = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ReturnExpression"
                && binding.name == "return"
                && binding.source_kind == "Constructed"
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "TapIo")
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose tap_io constructed return binding: {bindings:#?}")
        });
    let parameter = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "tap_fn"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| panic!("{tool} should expose tap_io tap_fn parameter: {bindings:#?}"));
    let projection = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "FieldProjection"
                && binding.name == "return.tap_fn"
                && binding.source_kind == "FieldProjection"
                && binding.source_id == Some(return_binding.id)
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "tap_fn")
                && binding.callee_kind.as_deref() == Some("Path")
                && matches!(binding.callee_path.as_deref(), Some([segment]) if segment == "tap_fn")
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose tap_io return.tap_fn projection: {bindings:#?}")
        });

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == return_binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for tap_io return binding: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == parameter.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for tap_io tap_fn parameter: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == projection.id
                && edge.target_id == return_binding.id
                && edge.relation == LocalBindingRelationKind::BindingProjectsField
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose BindingProjectsField for tap_io return.tap_fn: {edges:#?}"
    );
}

pub(crate) fn assert_memchr_runner_setter_local_binding_payload(
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    field_name: &str,
    tool: &str,
) {
    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let parameter = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "search"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose memchr Runner::{field_name} search parameter: {bindings:#?}"
            )
        });
    let assignment = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "FieldAssignment"
                && binding.name == format!("self.{field_name}")
                && binding.source_kind == "SelfFieldAssignment"
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == field_name)
                && binding.callee_kind.as_deref() == Some("Path")
                && matches!(binding.callee_path.as_deref(), Some([segment]) if segment == "search")
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose memchr Runner::{field_name} self-field assignment: {bindings:#?}"
            )
        });

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == assignment.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for memchr Runner::{field_name} assignment: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == assignment.id
                && edge.target_id == parameter.id
                && edge.relation == LocalBindingRelationKind::BindingSourceParameter
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose BindingSourceParameter for memchr Runner::{field_name}: {edges:#?}"
    );
}

pub(crate) fn assert_memchr_runner_run_assignment_flow_payload(
    flows: &[serde_json::Value],
    owner: Uuid,
    tool: &str,
) {
    let rows = flows
        .iter()
        .filter_map(|flow| serde_json::from_value::<SelfFieldAssignmentFlowInfo>(flow.clone()).ok())
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        3,
        "{tool} should expose same-type Runner::run assignment flow candidates: {flows:#?}"
    );
    let count_for = |field_name: &str| {
        rows.iter()
            .filter(|flow| {
                flow.site.owner_id == owner
                    && flow.site.kind == CallSiteKind::Path
                    && flow
                        .site
                        .path
                        .as_deref()
                        .is_some_and(|path| path.iter().map(String::as_str).eq([field_name]))
            })
            .count()
    };
    assert_eq!(
        count_for("fwd"),
        2,
        "{tool} should expose the substring and packedpair Runner::fwd candidates: {flows:#?}"
    );
    assert_eq!(
        count_for("rev"),
        1,
        "{tool} should expose the substring Runner::rev candidate: {flows:#?}"
    );
    for field_name in ["fwd", "rev"] {
        let flow = rows
            .iter()
            .find(|flow| {
                flow.site.owner_id == owner
                    && flow.site.kind == CallSiteKind::Path
                    && flow
                        .site
                        .path
                        .as_deref()
                        .is_some_and(|path| path.iter().map(String::as_str).eq([field_name]))
            })
            .unwrap_or_else(|| {
                panic!("{tool} should expose the {field_name} assignment flow: {flows:#?}")
            });

        assert_eq!(flow.site.status, CallStatusKind::Unsupported);
        assert!(
            flow.site.targets.is_empty(),
            "{tool} must keep memchr Runner::{field_name} boxed callable targetless: {flow:#?}"
        );
        assert_eq!(flow.owner_type, "Runner");
        assert_eq!(flow.assignment_binding.owner_id, flow.setter_id);
        assert_eq!(flow.assignment_binding.kind, "FieldAssignment");
        assert_eq!(flow.assignment_binding.name, format!("self.{field_name}"));
        assert_eq!(flow.assignment_binding.source_kind, "SelfFieldAssignment");
        assert!(
            flow.assignment_binding
                .source_path
                .as_deref()
                .is_some_and(|path| path.iter().map(String::as_str).eq([field_name])),
            "{tool} should expose the {field_name} assignment source path: {flow:#?}"
        );
        assert_eq!(flow.assignment_binding.callee_kind.as_deref(), Some("Path"));
        assert!(
            flow.assignment_binding
                .callee_path
                .as_deref()
                .is_some_and(|path| path.iter().map(String::as_str).eq(["search"])),
            "{tool} should expose the {field_name} assignment parameter path: {flow:#?}"
        );
        assert_eq!(flow.parameter_binding.owner_id, flow.setter_id);
        assert_eq!(flow.parameter_binding.kind, "ParameterBinding");
        assert_eq!(flow.parameter_binding.name, "search");
        assert_eq!(flow.parameter_binding.source_kind, "Parameter");
        assert_eq!(
            flow.source_edge.relation,
            LocalBindingRelationKind::BindingSourceParameter
        );
        assert_eq!(flow.source_edge.source_id, flow.assignment_binding.id);
        assert_eq!(flow.source_edge.target_id, flow.parameter_binding.id);
    }
}

pub(crate) fn assert_handle_error_returned_future_local_binding_payload(
    calls: &[serde_json::Value],
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
    owner: Uuid,
    tool: &str,
) {
    let call_rows = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .collect::<Vec<_>>();
    let box_pin = call_rows
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call
                    .path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"]))
        })
        .unwrap_or_else(|| panic!("{tool} should expose HandleError::call Box::pin: {calls:#?}"));

    let rows = bindings
        .iter()
        .filter_map(|binding| serde_json::from_value::<LocalBindingInfo>(binding.clone()).ok())
        .collect::<Vec<_>>();
    let return_binding = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ReturnExpression"
                && binding.name == "return"
                && binding.source_kind == "Constructed"
                && binding.source_path.as_deref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["future", "HandleErrorFuture"])
                })
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose HandleError::call constructed returned future binding: {bindings:#?}"
            )
        });
    let future_field = rows
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "return.future"
                && binding.source_kind == "PathCallResult"
                && binding.source_id == Some(box_pin.site_id)
                && binding.source_call_kind.as_deref() == Some("Path")
                && binding
                    .source_path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"]))
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose HandleError::call return.future sourced by Box::pin: {bindings:#?}"
            )
        });

    let edge_rows = edges
        .iter()
        .filter_map(|edge| serde_json::from_value::<LocalBindingEdgeInfo>(edge.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == return_binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for HandleError::call return binding: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == future_field.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "{tool} should expose OwnerContainsBinding for HandleError::call return.future: {edges:#?}"
    );
    assert!(
        edge_rows.iter().any(|edge| {
            edge.source_id == future_field.id
                && edge.target_id == box_pin.site_id
                && edge.relation == LocalBindingRelationKind::BindingSourceCallResult
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "Path"
        }),
        "{tool} should expose BindingSourceCallResult for HandleError::call return.future: {edges:#?}"
    );
}

#[derive(Clone, Copy)]
pub(crate) struct PollProducerCase {
    pub(crate) owner_type: &'static str,
    pub(crate) receiver_method: &'static str,
    pub(crate) field_path: &'static [&'static str],
    pub(crate) return_path: &'static [&'static str],
    pub(crate) field_binding: &'static str,
}

pub(crate) fn assert_poll_producer(
    flows: &[serde_json::Value],
    owner: Uuid,
    expected: PollProducerCase,
    tool: &str,
) {
    let rows = flows
        .iter()
        .filter_map(|flow| {
            serde_json::from_value::<FuturePollFieldProducerFlowInfo>(flow.clone()).ok()
        })
        .collect::<Vec<_>>();
    let flow = rows
        .iter()
        .find(|flow| {
            flow.site.owner_id == owner
                && flow.site.kind == CallSiteKind::Method
                && flow.site.status == CallStatusKind::Unsupported
                && flow.poll_owner_type == expected.owner_type
                && matches!(
                    &flow.site.callee,
                    CallCalleeInfo::Method {
                        name,
                        receiver: Some(CallReceiverInfo::MethodResultField {
                            method_name,
                            field_path,
                            ..
                        }),
                    } if name == "poll" && method_name == expected.receiver_method
                        && field_path.iter().map(String::as_str).eq(expected.field_path.iter().copied())
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose {}::poll producer flow: {flows:#?}",
                expected.owner_type
            )
        });

    assert!(
        flow.site.targets.is_empty(),
        "{tool} should keep {}::poll targetless: {flow:#?}",
        expected.owner_type
    );
    assert_eq!(flow.return_binding.owner_id, flow.producer_id);
    assert_eq!(flow.return_binding.kind, "ReturnExpression");
    assert_eq!(flow.return_binding.name, "return");
    assert_eq!(flow.return_binding.source_kind, "Constructed");
    assert!(
        flow.return_binding
            .source_path
            .as_deref()
            .is_some_and(|path| path
                .iter()
                .map(String::as_str)
                .eq(expected.return_path.iter().copied())),
        "{tool} should expose the constructed {} return binding: {flow:#?}",
        expected.owner_type
    );
    assert_eq!(flow.field_binding.owner_id, flow.producer_id);
    assert_eq!(flow.field_binding.kind, "LetBinding");
    assert_eq!(flow.field_binding.name, expected.field_binding);
    assert_eq!(flow.field_binding.source_kind, "PathCallResult");
    assert_eq!(flow.field_binding.source_id, Some(flow.source_site.site_id));
    assert_eq!(flow.source_site.owner_id, flow.producer_id);
    assert_eq!(flow.source_site.kind, CallSiteKind::Path);
    assert!(
        flow.source_site
            .path
            .as_deref()
            .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"])),
        "{tool} should link return.future to the Box::pin source site: {flow:#?}"
    );
    assert_eq!(
        flow.source_edge.relation,
        LocalBindingRelationKind::BindingSourceCallResult
    );
    assert_eq!(flow.source_edge.source_id, flow.field_binding.id);
    assert_eq!(flow.source_edge.target_id, flow.source_site.site_id);
}

pub(crate) fn assert_forwarded_async_future_awaited_site(
    sites: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = sites
        .iter()
        .filter_map(|site| serde_json::from_value::<AwaitedCallSiteInfo>(site.clone()).ok())
        .collect::<Vec<_>>();
    let path = vec!["make_forwarded_returned_async_future".to_string()];
    let matches = rows
        .iter()
        .filter(|site| {
            site.owner_id == owner
                && site.kind == CallSiteKind::Path
                && site.path.as_deref() == Some(path.as_slice())
                && site.callee == (CallCalleeInfo::Path { path: path.clone() })
        })
        .count();
    assert_eq!(
        matches, 1,
        "{tool} should return exactly one awaited producer callsite for {label}: {sites:#?}"
    );
}

pub(crate) fn assert_forwarded_async_future_flow(
    flows: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = flows
        .iter()
        .filter_map(|flow| serde_json::from_value::<ReturnedFutureFlowInfo>(flow.clone()).ok())
        .collect::<Vec<_>>();
    let producer_path = vec!["make_forwarded_returned_async_future".to_string()];
    let future_path = vec!["make_returned_async_closure".to_string()];
    let matches = rows
        .iter()
        .filter(|flow| {
            flow.caller_id == owner
                && flow.producer.path == producer_path
                && flow.future.path == future_path
                && flow.future.callee_kind == "ReturnedPathCall"
                && flow.binding.source.relation == LocalBindingRelationKind::BindingSourceCallResult
                && flow.binding.source.kind == ReturnedCallSourceKind::Dynamic
        })
        .count();
    assert_eq!(
        matches, 1,
        "{tool} should return exactly one returned future flow for {label}: {flows:#?}"
    );
}

pub(crate) fn assert_forwarded_async_future_execution_flow(
    flows: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) {
    let rows = flows
        .iter()
        .filter_map(|flow| {
            serde_json::from_value::<ReturnedFutureExecutionFlowInfo>(flow.clone()).ok()
        })
        .collect::<Vec<_>>();
    let producer_path = vec!["make_forwarded_returned_async_future".to_string()];
    let future_path = vec!["make_returned_async_closure".to_string()];
    let matches = rows
        .iter()
        .filter(|flow| {
            flow.caller_id == owner
                && flow.producer.path == producer_path
                && flow.producer_binding.source.relation
                    == LocalBindingRelationKind::BindingSourceCallResult
                && flow.producer_binding.source.kind == ReturnedCallSourceKind::Dynamic
                && flow.future.path == future_path
                && flow.future.callee_kind == "ReturnedPathCall"
                && flow.maker.path == future_path
                && flow.callable_binding.source.relation
                    == LocalBindingRelationKind::BindingSourceClosure
                && flow.callable_binding.source.kind == ReturnedCallSourceKind::Closure
                && flow.body_edge.relation == CallTargetKind::Function
                && flow.body_edge.source_kind == CallSiteKind::Path
                && flow.body_edge.target_kind == CallEndpointKind::Function
        })
        .count();
    assert_eq!(
        matches, 1,
        "{tool} should return exactly one contextual returned future execution flow for {label}: {flows:#?}"
    );
}

pub(crate) fn assert_tap_io_accept_self_field_parameter_flow_payload(
    flows: &[serde_json::Value],
    owner: Uuid,
    tool: &str,
) {
    let rows = flows
        .iter()
        .filter_map(|flow| serde_json::from_value::<SelfFieldParameterFlowInfo>(flow.clone()).ok())
        .collect::<Vec<_>>();
    let matches = rows
        .iter()
        .filter(|flow| {
            flow.site.owner_id == owner
                && flow.site.kind == CallSiteKind::Dynamic
                && flow
                    .site
                    .path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["self", "tap_fn"]))
                && flow.site.status == CallStatusKind::Unsupported
                && flow.site.targets.is_empty()
                && flow.return_binding.kind == "ReturnExpression"
                && flow.return_binding.source_kind == "Constructed"
                && flow.field_binding.kind == "FieldProjection"
                && flow.field_binding.source_id == Some(flow.return_binding.id)
                && flow
                    .field_binding
                    .source_path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["tap_fn"]))
                && flow.parameter_binding.kind == "ParameterBinding"
                && flow.parameter_binding.name == "tap_fn"
                && flow.parameter_binding.source_kind == "Parameter"
        })
        .count();
    assert_eq!(
        matches, 1,
        "{tool} should expose exactly one TapIo::accept self-field parameter flow: {flows:#?}"
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
                        receiver: Some(CallReceiverInfo::AwaitMethodCallResult {
                            method_name: "acquire_owned".to_string(),
                        }),
                    })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{label} should return exactly one targetless AwaitMethodCallResult unwrap row: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::External);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{label} should not fabricate a target for AwaitMethodCallResult unwrap: {call:#?}"
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
        "{label} should return the AwaitMethodCallResult unwrap call_site proof row: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some("blocked")
                && proof.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
        }),
        "{label} should return the AwaitMethodCallResult unwrap external frontier proof row: {proofs:#?}"
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
