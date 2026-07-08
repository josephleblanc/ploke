use super::*;

#[derive(Clone, Copy)]
pub(crate) struct DynamicToolCase {
    pub(crate) label: &'static str,
    pub(crate) method: &'static str,
    pub(crate) owner_type: &'static str,
    pub(crate) file_suffix: &'static str,
    pub(crate) body: &'static str,
    pub(crate) expected_arg_count: Option<u32>,
    corpus: DynamicToolCorpus,
}

pub(crate) struct DynamicToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: DynamicToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

#[derive(Clone, Copy)]
enum DynamicToolCorpus {
    Axum,
    Memchr,
}

#[derive(Clone)]
pub(crate) struct ReceiverToolCase {
    pub(crate) label: &'static str,
    pub(crate) item: &'static str,
    pub(crate) callee: &'static str,
    pub(crate) status: CallStatusKind,
    pub(crate) owner_type: Option<&'static str>,
    pub(crate) module_path: Option<&'static [&'static str]>,
    pub(crate) file_suffix: &'static str,
    pub(crate) body: &'static str,
    pub(crate) generic_arg_count: Option<u32>,
    receiver: ReceiverShape,
}

#[derive(Clone, Copy)]
enum ReceiverShape {
    MethodResult { method: &'static str },
    SelfField { path: &'static [&'static str] },
    Unsupported,
}

pub(crate) struct ReceiverToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: ReceiverToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

#[derive(Clone)]
pub(crate) struct PathToolCase {
    pub(crate) label: &'static str,
    pub(crate) item: &'static str,
    pub(crate) path: &'static [&'static str],
    pub(crate) status: CallStatusKind,
    corpus: DynamicToolCorpus,
    owner: PathOwner,
}

#[derive(Clone, Copy)]
enum PathOwner {
    Function {
        module_path: &'static [&'static str],
        file_suffix: &'static str,
        body: &'static str,
    },
    Method {
        trait_name: &'static str,
        type_name: &'static str,
        module_path: &'static [&'static str],
        file_suffix: &'static str,
        body: &'static str,
    },
}

pub(crate) struct PathToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: PathToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

impl DynamicToolCase {
    pub(crate) const AXUM: [Self; 4] = [
        Self {
            label: "axum/src/boxed.rs:85 MakeErasedHandler::into_route callable field",
            method: "into_route",
            owner_type: "MakeErasedHandler",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.into_route)(self.handler, state)",
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
        Self {
            label: "axum/src/boxed.rs:120 MakeErasedRouter::into_route callable field",
            method: "into_route",
            owner_type: "MakeErasedRouter",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.into_route)(self.router, state)",
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
        Self {
            label: "axum/src/boxed.rs:159 Map::into_route layer trait object",
            method: "into_route",
            owner_type: "Map",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.layer)(self.inner.into_route(state))",
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
        Self {
            label: "axum/src/serve/listener.rs:236 TapIo::accept callable field",
            method: "accept",
            owner_type: "TapIo",
            file_suffix: "axum/src/serve/listener.rs",
            body: "(self.tap_fn)(&mut io)",
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
    ];

    pub(crate) const MEMCHR: [Self; 2] = [
        Self {
            label: "memchr/src/memmem/searcher.rs:222 Searcher.call",
            method: "find",
            owner_type: "Searcher",
            file_suffix: "src/memmem/searcher.rs",
            body: "(self.call)(self, prestate, haystack, needle)",
            expected_arg_count: Some(4),
            corpus: DynamicToolCorpus::Memchr,
        },
        Self {
            label: "memchr/src/memmem/searcher.rs:718 Prefilter.call",
            method: "find",
            owner_type: "Prefilter",
            file_suffix: "src/memmem/searcher.rs",
            body: "(self.call)(self, haystack)",
            expected_arg_count: Some(2),
            corpus: DynamicToolCorpus::Memchr,
        },
    ];

    pub(crate) fn build_domain(self) -> &'static str {
        self.corpus.build_domain()
    }
}

impl DynamicToolCorpus {
    fn db(self) -> Arc<Database> {
        match self {
            Self::Axum => axum_call_graph_db(),
            Self::Memchr => memchr_call_graph_db(),
        }
    }

    fn build_domain(self) -> &'static str {
        match self {
            Self::Axum => "bd:corpus-axum-call-graph",
            Self::Memchr => "bd:corpus-memchr-call-graph",
        }
    }
}

impl ReceiverToolCase {
    pub(crate) const ROUTE_ONESHOT: [Self; 2] = [
        Self {
            label: "axum/src/routing/route.rs:51 Route::oneshot_inner",
            item: "oneshot_inner",
            callee: "oneshot",
            status: CallStatusKind::External,
            owner_type: Some("Route"),
            module_path: None,
            file_suffix: "axum/src/routing/route.rs",
            body: "self.0.clone().oneshot(req)",
            generic_arg_count: None,
            receiver: ReceiverShape::MethodResult { method: "clone" },
        },
        Self {
            label: "axum/src/routing/route.rs:57 Route::oneshot_inner_owned",
            item: "oneshot_inner_owned",
            callee: "oneshot",
            status: CallStatusKind::External,
            owner_type: Some("Route"),
            module_path: None,
            file_suffix: "axum/src/routing/route.rs",
            body: "self.0.oneshot(req)",
            generic_arg_count: None,
            receiver: ReceiverShape::SelfField { path: &["0"] },
        },
    ];

    pub(crate) const SIZE_HINT: [Self; 1] = [Self {
        label: "axum-core/src/body.rs:127 Body::size_hint self-field external frontier",
        item: "size_hint",
        callee: "size_hint",
        status: CallStatusKind::External,
        owner_type: Some("Body"),
        module_path: None,
        file_suffix: "axum-core/src/body.rs",
        body: "self.0.size_hint()",
        generic_arg_count: None,
        receiver: ReceiverShape::SelfField { path: &["0"] },
    }];

    pub(crate) const REQUEST_PARTS_TURBOFISH: [Self; 1] = [Self {
        label: "axum-core/src/ext_traits/request_parts.rs:164 extract_with_state turbofish unsupported receiver",
        item: "extract_with_state",
        callee: "extract_with_state",
        status: CallStatusKind::Unsupported,
        owner_type: None,
        module_path: Some(&["crate", "ext_traits", "request_parts", "tests"]),
        file_suffix: "axum-core/src/ext_traits/request_parts.rs",
        body: "parts.extract_with_state::<State<String>, String>(&state)",
        generic_arg_count: Some(2),
        receiver: ReceiverShape::Unsupported,
    }];

    pub(crate) const FUTURE_POLL: [Self; 1] = [Self {
        label: "axum/src/error_handling/mod.rs:251 HandleErrorFuture::poll dyn Future",
        item: "poll",
        callee: "poll",
        status: CallStatusKind::Unsupported,
        owner_type: Some("HandleErrorFuture"),
        module_path: Some(&["crate", "error_handling", "future"]),
        file_suffix: "axum/src/error_handling/mod.rs",
        body: "self.project().future.poll(cx)",
        generic_arg_count: None,
        receiver: ReceiverShape::Unsupported,
    }];

    pub(crate) fn callee(&self) -> CallCalleeInfo {
        let receiver = match self.receiver {
            ReceiverShape::MethodResult { method } => Some(CallReceiverInfo::MethodCallResult {
                method_name: method.to_string(),
            }),
            ReceiverShape::SelfField { path } => Some(CallReceiverInfo::SelfField {
                path: path.iter().map(|segment| (*segment).to_string()).collect(),
            }),
            ReceiverShape::Unsupported => Some(CallReceiverInfo::Unsupported),
        };
        CallCalleeInfo::Method {
            name: self.callee.to_string(),
            receiver,
        }
    }

    pub(crate) fn node_kind(&self) -> &'static str {
        if self.owner_type.is_some() {
            "method"
        } else {
            "function"
        }
    }

    pub(crate) fn owner_type(&self) -> Option<&'static str> {
        self.owner_type
    }

    pub(crate) fn expects_runtime_dispatch_blocker(&self) -> bool {
        self.file_suffix == "axum/src/error_handling/mod.rs"
            && self.body == "self.project().future.poll(cx)"
            && self.callee == "poll"
    }
}

impl PathToolCase {
    pub(crate) const FROM_REF_DEP_ROOT: [Self; 1] = [Self {
        label: "axum/src/middleware/from_extractor.rs:328 Secret::from_ref dependency root",
        item: "test_from_extractor",
        path: &["Secret", "from_ref"],
        status: CallStatusKind::Resolved,
        corpus: DynamicToolCorpus::Axum,
        owner: PathOwner::Function {
            module_path: &["crate", "middleware", "from_extractor", "tests"],
            file_suffix: "axum/src/middleware/from_extractor.rs",
            body: "Secret::from_ref(state)",
        },
    }];

    pub(crate) const REQUEST_BUILDER_ALIAS: [Self; 1] = [Self {
        label: "axum/src/middleware/from_fn.rs:411 Request::builder alias external frontier",
        item: "basic",
        path: &["Request", "builder"],
        status: CallStatusKind::External,
        corpus: DynamicToolCorpus::Axum,
        owner: PathOwner::Function {
            module_path: &["crate", "middleware", "from_fn", "tests"],
            file_suffix: "axum/src/middleware/from_fn.rs",
            body: "Request::builder().uri(\"/\")",
        },
    }];

    pub(crate) const SHADOWED_GET: [Self; 1] = [Self {
        label: "axum/src/routing/tests/mod.rs:412-434 shadowed get boundary",
        item: "what_matches_wildcard",
        path: &["get"],
        status: CallStatusKind::Unsupported,
        corpus: DynamicToolCorpus::Axum,
        owner: PathOwner::Function {
            module_path: &["crate", "routing", "tests"],
            file_suffix: "axum/src/routing/tests/mod.rs",
            body: "let get = |path|",
        },
    }];

    pub(crate) const STD_MEM_REPLACE: [Self; 1] = [Self {
        label: "axum/src/response/sse.rs:449 std::mem::replace external frontier",
        item: "write_buf",
        path: &["std", "mem", "replace"],
        status: CallStatusKind::External,
        corpus: DynamicToolCorpus::Axum,
        owner: PathOwner::Method {
            trait_name: "",
            type_name: "EventDataWriter",
            module_path: &["crate", "response", "sse"],
            file_suffix: "axum/src/response/sse.rs",
            body: "std::mem::replace(&mut self.data_written, true)",
        },
    }];

    pub(crate) const INTO_SERVICE_FUTURE_NEW: [Self; 1] = [Self {
        label: "axum/src/handler/service.rs:174 IntoServiceFuture::new generated frontier",
        item: "call",
        path: &["super", "future", "IntoServiceFuture", "new"],
        status: CallStatusKind::Unresolved,
        corpus: DynamicToolCorpus::Axum,
        owner: PathOwner::Method {
            trait_name: "Service<Request>",
            type_name: "HandlerService",
            module_path: &["crate", "handler", "service"],
            file_suffix: "axum/src/handler/service.rs",
            body: "super::future::IntoServiceFuture::new(future)",
        },
    }];

    pub(crate) const MEMCHR_CALLABLE_TRAIT_OBJECT: [Self; 2] = [
        Self {
            label: "memchr/src/tests/substring/mod.rs:94 Runner.fwd boxed dyn FnMut",
            item: "run",
            path: &["fwd"],
            status: CallStatusKind::Unsupported,
            corpus: DynamicToolCorpus::Memchr,
            owner: PathOwner::Method {
                trait_name: "",
                type_name: "Runner",
                module_path: &["crate", "tests", "substring"],
                file_suffix: "src/tests/substring/mod.rs",
                body: "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
            },
        },
        Self {
            label: "memchr/src/tests/substring/mod.rs:110 Runner.rev boxed dyn FnMut",
            item: "run",
            path: &["rev"],
            status: CallStatusKind::Unsupported,
            corpus: DynamicToolCorpus::Memchr,
            owner: PathOwner::Method {
                trait_name: "",
                type_name: "Runner",
                module_path: &["crate", "tests", "substring"],
                file_suffix: "src/tests/substring/mod.rs",
                body: "rev(t.haystack.as_bytes(), t.needle.as_bytes())",
            },
        },
    ];

    pub(crate) fn callee(&self) -> CallCalleeInfo {
        CallCalleeInfo::Path {
            path: self.path.iter().map(|part| (*part).to_string()).collect(),
        }
    }

    pub(crate) fn build_domain(&self) -> &'static str {
        self.corpus.build_domain()
    }

    pub(crate) fn node_kind(&self) -> &'static str {
        match self.owner {
            PathOwner::Function { .. } => "function",
            PathOwner::Method { .. } => "method",
        }
    }

    pub(crate) fn expects_admitted_external_summary(&self) -> bool {
        self.path == ["std", "mem", "replace"]
            && matches!(
                self.owner,
                PathOwner::Method {
                    file_suffix: "axum/src/response/sse.rs",
                    ..
                }
            )
    }

    pub(crate) fn owner_trait(&self) -> Option<&'static str> {
        match self.owner {
            PathOwner::Function { .. } => None,
            PathOwner::Method { trait_name, .. } if trait_name.is_empty() => None,
            PathOwner::Method { trait_name, .. } => Some(trait_name),
        }
    }

    pub(crate) fn owner_type(&self) -> Option<&'static str> {
        match self.owner {
            PathOwner::Function { .. } => None,
            PathOwner::Method { type_name, .. } => Some(type_name),
        }
    }
}

impl DynamicToolFixture {
    pub(crate) async fn new(case: DynamicToolCase) -> Self {
        let db = case.corpus.db();
        let owner = owner_by_body(
            &db,
            case.method,
            case.owner_type,
            None,
            case.file_suffix,
            case.body,
            case.label,
        );
        assert!(
            db.project_call_proof_facts_for_node(owner.id, case.build_domain())
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label))
                >= 2,
            "{} should project targetless dynamic call-site proof rows",
            case.label
        );
        let state = axum_state_for_target(Arc::clone(&db), &owner, case.label).await;

        Self {
            state,
            case,
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

impl ReceiverToolFixture {
    pub(crate) async fn new(case: ReceiverToolCase) -> Self {
        let db = axum_call_graph_db();
        let owner = match case.owner_type {
            Some(owner_type) => owner_by_body(
                &db,
                case.item,
                owner_type,
                case.module_path,
                case.file_suffix,
                case.body,
                case.label,
            ),
            None => function_owner_by_body(
                &db,
                case.item,
                case.module_path
                    .expect("function-owned receiver cases require module_path"),
                case.file_suffix,
                case.body,
                case.label,
            ),
        };
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-axum-call-graph")
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label))
                >= 2,
            "{} should project targetless receiver proof rows",
            case.label
        );
        attach_runtime_dispatch_blocker_if_needed(&db, owner.id, &case);
        attach_parts_blocker_if_needed(&db, owner.id, &case);
        let state = axum_state_for_target(Arc::clone(&db), &owner, case.label).await;

        Self {
            state,
            case,
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

fn attach_runtime_dispatch_blocker_if_needed(db: &Database, owner: Uuid, case: &ReceiverToolCase) {
    if !case.expects_runtime_dispatch_blocker() {
        return;
    }
    let site = db
        .call_context_for_owner(owner)
        .unwrap_or_else(|err| panic!("{} call context lookup: {err}", case.label))
        .into_iter()
        .find(|row| {
            row.site.method.as_deref() == Some(case.callee)
                && row.status.status == DbCallStatusKind::Unsupported
        })
        .unwrap_or_else(|| {
            panic!(
                "{} should expose the dyn Future::poll unsupported callsite before blocker insertion",
                case.label
            )
        })
        .site
        .id
        .to_string();

    db.upsert_proof_fact_values(&[json!({
        "fact_kind": "proof_blocker",
        "schema_version": "ploke-proof-facts.v1",
        "blocker_id": "blocker:axum-dyn-future-poll-runtime-dispatch",
        "reason": "dynamic_dispatch_unbounded",
        "status": "blocked",
        "call_site_id": site,
        "detail": "axum/src/error_handling/mod.rs:251 dyn Future::poll concrete runtime future unresolved",
        "evidence_use": "proof_only"
    })])
    .unwrap_or_else(|err| panic!("{} runtime dispatch blocker insert: {err}", case.label));
}

fn attach_parts_blocker_if_needed(db: &Database, owner: Uuid, case: &ReceiverToolCase) {
    if !case.label.contains("request_parts.rs:164") {
        return;
    }
    let site = db
        .call_context_for_owner(owner)
        .unwrap_or_else(|err| panic!("{} call context lookup: {err}", case.label))
        .into_iter()
        .find(|row| {
            row.site.method.as_deref() == Some(case.callee)
                && row.status.status == DbCallStatusKind::Unsupported
                && row.site.generic_arg_count == Some(2)
        })
        .unwrap_or_else(|| {
            panic!(
                "{} should expose the request-parts turbofish unsupported callsite before blocker insertion",
                case.label
            )
        })
        .site
        .id;

    db.upsert_proof_fact_values(&[ploke_test_utils::axum_parts_blocker(site)])
        .unwrap_or_else(|err| panic!("{} request-parts blocker insert: {err}", case.label));
}

impl PathToolFixture {
    pub(crate) async fn new(case: PathToolCase) -> Self {
        let db = case.corpus.db();
        let owner = match case.owner {
            PathOwner::Function {
                module_path,
                file_suffix,
                body,
            } => function_owner_by_body(&db, case.item, module_path, file_suffix, body, case.label),
            PathOwner::Method {
                type_name,
                module_path,
                file_suffix,
                body,
                ..
            } => owner_by_body(
                &db,
                case.item,
                type_name,
                Some(module_path),
                file_suffix,
                body,
                case.label,
            ),
        };
        assert!(
            db.project_call_proof_facts_for_node(owner.id, case.build_domain())
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label))
                >= 2,
            "{} should project targetless path proof rows",
            case.label
        );
        attach_admitted_external_summary_if_needed(&db, owner.id, &case);
        let state = axum_state_for_target(Arc::clone(&db), &owner, case.label).await;

        Self {
            state,
            case,
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

fn attach_admitted_external_summary_if_needed(db: &Database, owner: Uuid, case: &PathToolCase) {
    if !case.expects_admitted_external_summary() {
        return;
    }
    let site = db
        .call_context_for_owner(owner)
        .unwrap_or_else(|err| panic!("{} call context lookup: {err}", case.label))
        .into_iter()
        .find(|row| {
            row.site.path.as_ref().is_some_and(|path| {
                path.iter()
                    .map(String::as_str)
                    .eq(["std", "mem", "replace"])
            }) && row.status.status == DbCallStatusKind::External
        })
        .unwrap_or_else(|| {
            panic!(
                "{} should expose the std::mem::replace external callsite before summary insertion",
                case.label
            )
        })
        .site
        .id
        .to_string();

    db.upsert_proof_fact_values(&axum_std_mem_replace_summary_records(site))
        .unwrap_or_else(|err| panic!("{} admitted external summary insert: {err}", case.label));
}

fn axum_std_mem_replace_summary_records(site_id: String) -> Vec<serde_json::Value> {
    const SUMMARY_ID: &str = "external-summary:axum-std-mem-replace";
    vec![
        json!({
            "fact_kind": "build_domain",
            "schema_version": "ploke-proof-facts.v1",
            "build_domain_id": "bd:corpus-axum-call-graph",
            "cargo_metadata_hash": "sha256:axum-metadata",
            "cargo_lock_hash": "sha256:axum-lock",
            "package_id": "github:tokio-rs/axum",
            "target_kind": "library",
            "target_name": "axum",
            "target_root": "axum/src/lib.rs",
            "target_triple": "x86_64-unknown-linux-gnu",
            "host_triple": "x86_64-unknown-linux-gnu",
            "profile": "dev",
            "features_hash": "sha256:axum-features",
            "active_cfg_hash": "sha256:axum-cfg",
            "rustc_version": "rustc fixture",
            "extractor_version": "ploke-test",
            "proof_policy_version": "proof-policy-test",
            "evidence_use": "proof_only"
        }),
        json!({
            "fact_kind": "cfg_domain",
            "schema_version": "ploke-proof-facts.v1",
            "cfg_domain_id": "cfg:corpus-axum-call-graph",
            "build_domain_id": "bd:corpus-axum-call-graph",
            "active_cfg_hash": "sha256:axum-cfg",
            "status": "admitted",
            "evidence_use": "proof_only"
        }),
        json!({
            "fact_kind": "rustc_invocation",
            "schema_version": "ploke-proof-facts.v1",
            "invocation_id": "rustc:corpus-axum-call-graph",
            "build_domain_id": "bd:corpus-axum-call-graph",
            "rustc_program": "rustc",
            "rustc_version": "rustc fixture",
            "working_directory": "/workspace/axum",
            "argument_vector_hash": "sha256:axum-rustc-argv",
            "environment_hash": "sha256:axum-rustc-env",
            "status": "admitted",
            "evidence_use": "proof_only"
        }),
        json!({
            "fact_kind": "call_resolution",
            "schema_version": "ploke-proof-facts.v1",
            "call_site_id": site_id,
            "resolution_state": "externally_summarized",
            "external_summary_id": SUMMARY_ID,
            "evidence_use": "proof_and_navigation"
        }),
        json!({
            "fact_kind": "external_summary",
            "schema_version": "ploke-proof-facts.v1",
            "external_summary_id": SUMMARY_ID,
            "build_domain_id": "bd:corpus-axum-call-graph",
            "summary_class": "audited_no_process_effects",
            "artifact_hash": "sha256:axum-std-mem-replace-summary",
            "version": "axum-call-graph-summary-v1",
            "review_method": "source-oracle-review",
            "scope_of_validity": "axum std::mem::replace frontier in corpus_axum_call_graph",
            "allowed_effects": ["external_summary_boundary"],
            "required_containment": "none",
            "invalidation_conditions": "source oracle, fixture hash, or proof policy changes",
            "status": "admitted",
            "evidence_use": "proof_only"
        }),
    ]
}

pub(crate) fn assert_dynamic_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    expected_arg_count: Option<u32>,
    label: &str,
    tool: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one dynamic targetless row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    if let Some(expected) = expected_arg_count {
        assert_eq!(
            call.arg_count,
            Some(expected),
            "{tool} should preserve dynamic argument count for {label}: {call:#?}"
        );
    }
    assert_eq!(call.status, CallStatusKind::Unsupported);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{tool} should not fabricate traversal targets for {label}: {call:#?}"
    );
    call.site_id
}

pub(crate) fn assert_method_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    status: &CallStatusKind,
    generic_arg_count: Option<u32>,
    label: &str,
    tool: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner && call.kind == CallSiteKind::Method && &call.callee == callee
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one method targetless row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(&call.status, status);
    if let Some(expected) = generic_arg_count {
        assert_eq!(
            call.generic_arg_count,
            Some(expected),
            "{tool} should preserve method generic argument count for {label}: {call:#?}"
        );
    }
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{tool} should not fabricate traversal targets for {label}: {call:#?}"
    );
    call.site_id
}

pub(crate) fn assert_path_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    status: &CallStatusKind,
    label: &str,
    tool: &str,
) -> Uuid {
    let matching = matching_path_context(calls, owner, callee);
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one path targetless row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(&call.status, status);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{tool} should not fabricate traversal targets for {label}: {call:#?}"
    );
    call.site_id
}

pub(crate) fn assert_path_context_count(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    status: &CallStatusKind,
    expected_count: usize,
    label: &str,
    tool: &str,
) -> Vec<Uuid> {
    let matching = matching_path_context(calls, owner, callee);
    assert_eq!(
        matching.len(),
        expected_count,
        "{tool} should return exactly {expected_count} path targetless rows for {label}: {calls:#?}"
    );
    for call in &matching {
        assert_eq!(&call.status, status);
        assert_eq!(call.resolution, None);
        assert!(
            call.targets.is_empty(),
            "{tool} should not fabricate traversal targets for {label}: {call:#?}"
        );
    }
    matching.iter().map(|call| call.site_id).collect()
}

pub(crate) fn assert_path_context_absent(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    label: &str,
    tool: &str,
) {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner && call.kind == CallSiteKind::Path && &call.callee == callee
        })
        .collect::<Vec<_>>();
    assert!(
        matching.is_empty(),
        "{tool} should not flatten the nested local-item path row into {label}: {calls:#?}"
    );
}

fn matching_path_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
) -> Vec<CallContextInfo> {
    calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner && call.kind == CallSiteKind::Path && &call.callee == callee
        })
        .collect()
}

pub(crate) fn assert_dynamic_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    build_domain: &str,
    label: &str,
    tool: &str,
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
                && proof.build_domain_id.as_deref() == Some(build_domain)
        }),
        "{tool} should return the dynamic call_site proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some("blocked")
                && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
        }),
        "{tool} should return the dynamic blocked resolution proof row for {label}: {proofs:#?}"
    );
}

pub(crate) fn assert_method_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    status: &CallStatusKind,
    label: &str,
    tool: &str,
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
        "{tool} should return the targetless method call_site proof row for {label}: {proofs:#?}"
    );
    let state = match status {
        CallStatusKind::Resolved => "resolved",
        CallStatusKind::Unresolved => "unresolved",
        CallStatusKind::Ambiguous => "ambiguous",
        CallStatusKind::External | CallStatusKind::Unsupported => "blocked",
    };
    let reason = match status {
        CallStatusKind::External => "external_dependency_summary_missing",
        _ => "type_resolution_missing",
    };
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some(state)
                && proof.blocker_reason.as_deref() == Some(reason)
        }),
        "{tool} should return the targetless method {state} resolution proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().all(|proof| {
            proof.kind != "call_edge" || proof.call_site_id.as_deref() != Some(site_id.as_str())
        }),
        "{tool} should not fabricate a call_edge for targetless method row {label}: {proofs:#?}"
    );
}

pub(crate) fn assert_runtime_dispatch_blocker(
    proofs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site_id = site_id.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "proof_blocker"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
                && proof.status.as_deref() == Some("blocked")
        }),
        "{tool} should return the runtime-dispatch proof blocker for {label}: {proofs:#?}"
    );
}

pub(crate) fn assert_parts_blocker(
    proofs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site_id = site_id.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "proof_blocker"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
                && proof.status.as_deref() == Some("blocked")
        }),
        "{tool} should return the request-parts external return proof blocker for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some("blocked")
                && proof.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "{tool} should keep the request-parts projected call_resolution fail-closed for {label}: {proofs:#?}"
    );
}

pub(crate) fn assert_path_blocker_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    build_domain: &str,
    blocker_reason: &str,
    label: &str,
    tool: &str,
) {
    assert_path_resolution_proof(
        proofs,
        owner,
        site_id,
        build_domain,
        "blocked",
        blocker_reason,
        label,
        tool,
    );
}

pub(crate) fn assert_admitted_external_summary_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let site_id = site_id.to_string();
    const BUILD_DOMAIN: &str = "bd:corpus-axum-call-graph";
    const SUMMARY_ID: &str = "external-summary:axum-std-mem-replace";
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.build_domain_id.as_deref() == Some(BUILD_DOMAIN)
        }),
        "{tool} should return the path call_site proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some("externally_summarized")
                && proof.external_summary_id.as_deref() == Some(SUMMARY_ID)
                && proof.blocker_reason.is_none()
        }),
        "{tool} should return the externally summarized call_resolution for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(SUMMARY_ID)
                && proof.summary_class.as_deref() == Some("audited_no_process_effects")
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == vec!["external_summary_boundary".to_string()]
        }),
        "{tool} should return the admitted external_summary artifact for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().all(|proof| {
            proof.call_site_id.as_deref() != Some(site_id.as_str())
                || proof.blocker_reason.as_deref() != Some("external_dependency_summary_missing")
        }),
        "{tool} should not retain the missing-summary blocker after admission for {label}: {proofs:#?}"
    );
}

pub(crate) fn assert_path_resolution_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    build_domain: &str,
    resolution_state: &str,
    blocker_reason: &str,
    label: &str,
    tool: &str,
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
                && proof.build_domain_id.as_deref() == Some(build_domain)
        }),
        "{tool} should return the path call_site proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some(resolution_state)
                && proof.blocker_reason.as_deref() == Some(blocker_reason)
        }),
        "{tool} should return the {resolution_state} {blocker_reason} proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().all(|proof| {
            proof.kind != "call_edge" || proof.call_site_id.as_deref() != Some(site_id.as_str())
        }),
        "{tool} should not fabricate a call_edge for targetless path row {label}: {proofs:#?}"
    );
}

fn owner_by_body(
    db: &Database,
    method: &str,
    owner_type: &str,
    module_path: Option<&[&str]>,
    file_suffix: &str,
    body_marker: &str,
    label: &str,
) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(method));
    params.insert("owner_type".to_string(), DataValue::from(owner_type));
    params.insert(
        "owner_path".to_string(),
        DataValue::List(vec![DataValue::from(owner_type)]),
    );

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

impl_self_target[self_target_id] := *struct {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_target[self_target_id] := *enum {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_target[self_target_id] := *union {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_type[self_type_id] :=
    *type_relation {{
        source_id: self_type_id,
        target_id: self_target_id,
        relation_kind: "Ordinary" @ 'NOW'
    }},
    impl_self_target[self_target_id]
impl_self_type[self_type_id] :=
    *named_type {{ type_id: self_type_id, path @ 'NOW' }},
    path == $owner_path

?[id, body, file_path, mod_path] :=
    *method {{ id, name: $name, body, owner_id: impl_id @ 'NOW' }},
    *impl {{ id: impl_id, self_type: self_type_id @ 'NOW' }},
    impl_self_type[self_type_id],
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query {label} owner: {err}"));
    let marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            let module_matches = module_path.is_none_or(|expected| {
                let actual = data_path(&row[3], "module path");
                actual
                    .iter()
                    .map(String::as_str)
                    .eq(expected.iter().copied())
            });
            body_key(body).contains(&marker)
                && module_matches
                && data_str(&row[2], "file_path").ends_with(file_suffix)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one owner for {label}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn function_owner_by_body(
    db: &Database,
    name: &str,
    module_path: &[&str],
    file_suffix: &str,
    body_marker: &str,
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
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *function {{ id, name: $name, body, module_id @ 'NOW' }},
    *module{{ id: module_id, path: $module_path @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query {label} owner: {err}"));
    let marker = body_key(body_marker);
    let mut matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            let module_matches = data_path(&row[3], "module path")
                .iter()
                .map(String::as_str)
                .eq(module_path.iter().copied());
            body_key(body).contains(&marker)
                && data_str(&row[2], "file_path").ends_with(file_suffix)
                && module_matches
        })
        .collect::<Vec<_>>();
    matching.sort_by_key(|row| to_uuid(&row[0]).expect("function owner uuid"));
    matching.dedup_by_key(|row| to_uuid(&row[0]).expect("function owner uuid"));
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one function owner for {label}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}
