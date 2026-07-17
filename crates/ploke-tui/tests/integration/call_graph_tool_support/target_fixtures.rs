use super::*;

pub(crate) struct AxumBodyEmptyToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
    pub(crate) dependency_root_sites: Vec<Uuid>,
}

pub(crate) struct AxumBodyNewToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
    pub(crate) generated_callers: Vec<ExpectedCallSite>,
}

pub(crate) struct AxumGeneratedRejectionToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) calls: Vec<ExpectedMethodEdge>,
}

pub(crate) struct AxumCompositeRejectionToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) call: ExpectedMethodEdge,
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

pub(crate) struct AxumFromFnBasicToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) body_empty_target: Uuid,
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

pub(crate) struct ChronoParseInternalToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

pub(crate) struct AxumExpandWithToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
}

pub(crate) struct AxumRouteMethodToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) cases: Vec<RouteMethodCase>,
}

pub(crate) struct RouteMethodCase {
    pub(crate) name: &'static str,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) call: ExpectedMethodEdge,
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

pub(crate) struct AxumHandleErrorCallToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
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
        let dependency_root_sites =
            attach_body_empty_dependency_root_proof(&db, target.id, &callers);
        let state = axum_state_for_target(Arc::clone(&db), &target, "Body::empty").await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
            callers,
            dependency_root_sites,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumBodyNewToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_method_target_by_body_and_file(
            &db,
            "new",
            "try_downcast(body)",
            "axum-core/src/body.rs",
        );
        let callers = db
            .callers_for_target(target.id)
            .expect("Body::new incoming callers")
            .into_iter()
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .expect("Body::new caller should carry a path"),
            })
            .collect::<Vec<_>>();
        let generated_callers = axum_body_from_impl_generated_callers(&db, target.id);
        assert!(
            callers.len() >= generated_callers.len(),
            "Body::new should expose all generated body_from_impl! caller rows"
        );
        for generated in &generated_callers {
            assert!(
                callers.iter().any(|caller| {
                    caller.owner == generated.owner
                        && caller.site == generated.site
                        && caller.path == generated.path
                }),
                "Body::new target-centered callers should include generated caller {generated:#?}: {callers:#?}"
            );
        }
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .expect("project axum Body::new proof facts")
                >= callers.len(),
            "Body::new should project target-scoped proof rows for real-corpus callers"
        );
        let state = axum_state_for_target(Arc::clone(&db), &target, "Body::new").await;

        Self {
            state,
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
            callers,
            generated_callers,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumGeneratedRejectionToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let owner = axum_method_target_by_body_and_file(
            &db,
            "into_response",
            "rejection_type=MissingExtension",
            "axum/src/extract/rejection.rs",
        );
        let calls = axum_generated_rejection_self_calls(&db, owner.id);
        assert_eq!(
            calls.len(),
            2,
            "MissingExtension::into_response should expose self.status() and self.body_text()"
        );
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-axum-call-graph")
                .expect("project axum MissingExtension::into_response proof facts")
                >= calls.len(),
            "MissingExtension::into_response should project generated self-call proof rows"
        );
        let state =
            axum_state_for_target(Arc::clone(&db), &owner, "MissingExtension::into_response").await;

        Self {
            state,
            file_path: owner.file_path,
            module_path: owner.module_path,
            owner: owner.id,
            calls,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl AxumCompositeRejectionToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let owner = axum_method_target_by_body_and_file(
            &db,
            "into_response",
            "Self::FailedToDeserializeQueryString(inner)=>inner.into_response()",
            "axum/src/extract/rejection.rs",
        );
        let call = axum_composite_rejection_delegate_call(&db, owner.id);
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-axum-call-graph")
                .expect("project axum QueryRejection::into_response proof facts")
                >= 1,
            "QueryRejection::into_response should project generated delegate proof rows"
        );
        let state =
            axum_state_for_target(Arc::clone(&db), &owner, "QueryRejection::into_response").await;

        Self {
            state,
            file_path: owner.file_path,
            module_path: owner.module_path,
            owner: owner.id,
            call,
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

    pub(crate) fn admit_serde_summary(&self) -> Uuid {
        let context = self
            .state
            .db
            .call_context_for_owner(self.target)
            .expect("Json::from_bytes call context");
        let site = context
            .iter()
            .find(|row| {
                row.site.path.as_ref().is_some_and(|call_path| {
                    call_path
                        .iter()
                        .map(String::as_str)
                        .eq(["serde_json", "Deserializer", "from_slice"])
                }) && row.status.status == DbCallStatusKind::External
                    && row.targets.is_empty()
            })
            .unwrap_or_else(|| {
                panic!(
                    "Json::from_bytes should expose the targetless serde_json::Deserializer::from_slice frontier: {context:#?}"
                )
            })
            .site
            .id;
        self.state
            .db
            .upsert_proof_fact_values(
                &[
                    axum_serde_json_from_slice_summary_records(site),
                    vec![serde_json::json!({
                        "fact_kind": "effect_seed",
                        "schema_version": "ploke-proof-facts.v1",
                        "effect_seed_id": "effect:axum-json-parse-surface-measure",
                        "call_site_id": site.to_string(),
                        "effect_class": "surface_measure",
                        "confidence": "source-oracle",
                        "blocker_if_unresolved": false,
                        "evidence_use": "proof_only"
                    })],
                ]
                .concat(),
            )
            .expect("insert Json::from_bytes serde external summary");
        site
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

impl AxumFromFnBasicToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let owner =
            axum_function_target_by_name_and_file(&db, "basic", "axum/src/middleware/from_fn.rs");
        let body_empty = axum_body_empty_target(&db);
        let edges = db
            .crate_boundary_edges_from_owner(
                owner.id,
                ploke_db::CallPathOptions {
                    max_depth: 1,
                    max_paths: 64,
                },
            )
            .expect("from_fn::tests::basic crate-boundary edges");
        assert!(
            edges.iter().any(|edge| {
                edge.edge.caller_id == owner.id && edge.edge.callee_id == body_empty.id
            }),
            "current axum fixture should expose from_fn::tests::basic -> Body::empty as a crate-boundary edge: {edges:#?}"
        );
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-axum-call-graph")
                .expect("project from_fn::tests::basic proof facts")
                >= 1,
            "from_fn::tests::basic should project proof rows for its callsites"
        );
        let state = axum_state_for_target(Arc::clone(&db), &owner, "from_fn::tests::basic").await;

        Self {
            state,
            file_path: owner.file_path,
            module_path: owner.module_path,
            owner: owner.id,
            body_empty_target: body_empty.id,
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

impl ChronoParseInternalToolFixture {
    pub(crate) async fn new() -> Self {
        let db = chrono_call_graph_db();
        let owner = chrono_parse_internal_target(&db);
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-chrono-call-graph")
                .expect("project chrono parse_internal proof facts")
                >= 2,
            "chrono parse_internal should project node-scoped typed-setter proof rows"
        );
        let state = source_state_for_target_with_rag_config(
            Arc::clone(&db),
            &owner,
            "parse_internal",
            RagConfig::default(),
        )
        .await;

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

impl AxumRouteMethodToolFixture {
    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let on = axum_method_target_by_body_and_file(
            &db,
            "on",
            "self.on_endpoint(filter, &MethodEndpoint::BoxedHandler",
            "axum/src/routing/method_routing.rs",
        );
        let on_service = axum_method_target_by_body_and_file(
            &db,
            "on_service",
            "self.on_endpoint(filter, &MethodEndpoint::Route",
            "axum/src/routing/method_routing.rs",
        );
        let cases = vec![
            route_method_case(&db, "post", "self.on(MethodFilter::", "on", on.id),
            route_method_case(
                &db,
                "post_service",
                "self.on_service(MethodFilter::",
                "on_service",
                on_service.id,
            ),
        ];
        for case in &cases {
            assert!(
                db.project_call_proof_facts_for_node(case.owner, "bd:corpus-axum-call-graph")
                    .expect("project generated route method proof facts")
                    >= 1,
                "generated {} should project its self-call proof row",
                case.name
            );
        }
        let state = axum_state_for_target(Arc::clone(&db), &on, "generated route method").await;

        Self { state, cases }
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl RouteMethodCase {
    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }
}

fn route_method_case(
    db: &Database,
    name: &'static str,
    body: &str,
    callee: &'static str,
    target: Uuid,
) -> RouteMethodCase {
    let owner =
        axum_method_target_by_body_and_file(db, name, body, "axum/src/routing/method_routing.rs");
    let context = db
        .call_context_for_owner(owner.id)
        .unwrap_or_else(|err| panic!("generated {name} call context: {err}"));
    let row = context
        .iter()
        .find(|row| {
            row.site.kind == DbCallSiteKind::Method
                && row.site.method.as_deref() == Some(callee)
                && row.site.receiver.as_ref() == Some(&CallReceiver::SelfValue)
        })
        .unwrap_or_else(|| panic!("generated {name} should call self.{callee}(): {context:#?}"));
    assert_eq!(row.status.status, DbCallStatusKind::Resolved);
    assert_eq!(
        row.status.resolution,
        Some(DbCallResolutionKind::LocalExact)
    );
    assert_eq!(
        row.targets.len(),
        1,
        "generated {name} should resolve self.{callee}() to one target: {row:#?}"
    );
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, DbCallRelationKind::Method);
    assert_eq!(row.targets[0].target_kind, DbCallTargetKind::Method);

    RouteMethodCase {
        name,
        file_path: owner.file_path,
        module_path: owner.module_path,
        owner: owner.id,
        call: ExpectedMethodEdge {
            owner: owner.id,
            site: row.site.id,
            target,
            callee: CallCalleeInfo::Method {
                name: callee.to_string(),
                receiver: Some(CallReceiverInfo::SelfValue),
            },
        },
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
        let domain_id = "bd:corpus-axum-call-graph";
        let mut records = ploke_test_utils::axum_call_graph_domain_records(domain_id);
        records.push(ploke_test_utils::axum_entrypoint_record(
            domain_id, target.id,
        ));
        records.push(ploke_test_utils::axum_entrypoint_effect_policy_record(
            domain_id,
            target.id,
            &["ffi_boundary"],
        ));
        db.upsert_proof_fact_values(&records)
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

impl AxumHandleErrorCallToolFixture {
    pub(crate) const BODY_MARKER: &'static str = "Err(err) => Ok(f(err).await.into_response())";

    pub(crate) async fn new() -> Self {
        let db = axum_call_graph_db();
        let target = axum_method_target_by_body_and_file(
            &db,
            "call",
            Self::BODY_MARKER,
            "axum/src/error_handling/mod.rs",
        );
        let mut rag_config = RagConfig::default();
        rag_config.proof_context.enabled = false;
        let state = axum_state_for_target_with_rag_config(
            Arc::clone(&db),
            &target,
            "HandleError::call",
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
