use super::*;

#[derive(Clone, Copy)]
pub(crate) struct DynamicToolCase {
    pub(crate) label: &'static str,
    pub(crate) method: &'static str,
    pub(crate) owner_type: &'static str,
    pub(crate) file_suffix: &'static str,
    pub(crate) body: &'static str,
    pub(crate) expected_path: Option<&'static [&'static str]>,
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

#[derive(Clone)]
pub(crate) struct AmbiguousDynamicToolCase {
    pub(crate) label: &'static str,
    pub(crate) method: &'static str,
    pub(crate) owner_type: &'static str,
    pub(crate) file_suffix: &'static str,
    pub(crate) body: &'static str,
    pub(crate) expected_path: &'static [&'static str],
    pub(crate) expected_arg_count: Option<u32>,
    pub(crate) expected_relation: CallTargetKind,
    candidate_names: &'static [&'static str],
    corpus: DynamicToolCorpus,
}

pub(crate) struct AmbiguousDynamicToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: AmbiguousDynamicToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) candidates: Vec<Uuid>,
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
    MethodResult {
        method: &'static str,
    },
    SelfField {
        path: &'static [&'static str],
    },
    TupleMethodReturn {
        name: &'static str,
        method_name: &'static str,
        method_span: (usize, usize),
        index: usize,
    },
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
    pub(crate) const AXUM: [Self; 2] = [
        Self {
            label: "axum/src/boxed.rs:120 MakeErasedRouter::into_route callable field",
            method: "into_route",
            owner_type: "MakeErasedRouter",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.into_route)(self.router, state)",
            expected_path: Some(&["self", "into_route"]),
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
        Self {
            label: "axum/src/serve/listener.rs:236 TapIo::accept callable field",
            method: "accept",
            owner_type: "TapIo",
            file_suffix: "axum/src/serve/listener.rs",
            body: "(self.tap_fn)(&mut io)",
            expected_path: Some(&["self", "tap_fn"]),
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
    ];

    pub(crate) fn build_domain(&self) -> &'static str {
        self.corpus.build_domain()
    }
}

impl AmbiguousDynamicToolCase {
    pub(crate) const AXUM_LAYER: [Self; 2] = [
        Self {
            label: "axum/src/boxed.rs:159 Map::into_route layer trait object",
            method: "into_route",
            owner_type: "Map",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.layer)(self.inner.into_route(state))",
            expected_path: &["self", "layer"],
            expected_arg_count: Some(1),
            expected_relation: CallTargetKind::DynamicClosure,
            candidate_names: &[],
            corpus: DynamicToolCorpus::Axum,
        },
        Self {
            label: "axum/src/boxed.rs:163 Map::call_with_state layer trait object",
            method: "call_with_state",
            owner_type: "Map",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.layer)(self.inner.into_route(state)).call(request)",
            expected_path: &["self", "layer"],
            expected_arg_count: Some(1),
            expected_relation: CallTargetKind::DynamicClosure,
            candidate_names: &[],
            corpus: DynamicToolCorpus::Axum,
        },
    ];

    pub(crate) const MEMCHR_SELF_FIELD: [Self; 2] = [
        Self {
            label: "memchr/src/memmem/searcher.rs:222 Searcher.call",
            method: "find",
            owner_type: "Searcher",
            file_suffix: "src/memmem/searcher.rs",
            body: "(self.call)(self, prestate, haystack, needle)",
            expected_path: &["self", "call"],
            expected_arg_count: Some(4),
            expected_relation: CallTargetKind::DynamicFunction,
            candidate_names: &[
                "searcher_kind_empty",
                "searcher_kind_one_byte",
                "searcher_kind_two_way",
                "searcher_kind_two_way_with_prefilter",
                "searcher_kind_sse2",
                "searcher_kind_avx2",
            ],
            corpus: DynamicToolCorpus::Memchr,
        },
        Self {
            label: "memchr/src/memmem/searcher.rs:718 Prefilter.call",
            method: "find",
            owner_type: "Prefilter",
            file_suffix: "src/memmem/searcher.rs",
            body: "(self.call)(self, haystack)",
            expected_path: &["self", "call"],
            expected_arg_count: Some(2),
            expected_relation: CallTargetKind::DynamicFunction,
            candidate_names: &[
                "prefilter_kind_fallback",
                "prefilter_kind_sse2",
                "prefilter_kind_avx2",
            ],
            corpus: DynamicToolCorpus::Memchr,
        },
    ];

    pub(crate) fn build_domain(&self) -> &'static str {
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
        label: "axum-core/src/ext_traits/request_parts.rs:164 extract_with_state turbofish tuple-return receiver",
        item: "extract_with_state",
        callee: "extract_with_state",
        status: CallStatusKind::Resolved,
        owner_type: None,
        module_path: Some(&["crate", "ext_traits", "request_parts", "tests"]),
        file_suffix: "axum-core/src/ext_traits/request_parts.rs",
        body: "parts.extract_with_state::<State<String>, String>(&state)",
        generic_arg_count: Some(2),
        receiver: ReceiverShape::TupleMethodReturn {
            name: "parts",
            method_name: "into_parts",
            method_span: (4640, 4669),
            index: 0,
        },
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
            ReceiverShape::TupleMethodReturn {
                name,
                method_name,
                method_span,
                index,
            } => Some(CallReceiverInfo::TupleMethodReturn {
                name: name.to_string(),
                method_name: method_name.to_string(),
                method_span,
                index,
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

    pub(crate) fn db_status(&self) -> DbCallStatusKind {
        match &self.status {
            CallStatusKind::Resolved => DbCallStatusKind::Resolved,
            CallStatusKind::Unresolved => DbCallStatusKind::Unresolved,
            CallStatusKind::Ambiguous => DbCallStatusKind::Ambiguous,
            CallStatusKind::External => DbCallStatusKind::External,
            CallStatusKind::Unsupported => DbCallStatusKind::Unsupported,
        }
    }

    pub(crate) fn admitted_external_summary(&self) -> Option<ExternalSummaryCase> {
        if self.file_suffix == "axum-core/src/body.rs"
            && self.body == "self.0.size_hint()"
            && self.callee == "size_hint"
        {
            return Some(ExternalSummaryCase {
                path: &[],
                records: axum_body_size_hint_summary_records,
                summary_id: AXUM_BODY_SIZE_HINT_SUMMARY_ID,
            });
        }

        None
    }

    pub(crate) fn expects_runtime_dispatch_blocker(&self) -> bool {
        self.file_suffix == "axum/src/error_handling/mod.rs"
            && self.body == "self.project().future.poll(cx)"
            && self.callee == "poll"
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MacroBoundaryCase {
    pub(crate) boundary_id: fn(Uuid) -> String,
    pub(crate) records: fn(Uuid) -> Vec<serde_json::Value>,
    pub(crate) summary_id: &'static str,
    pub(crate) expanded_item_id: &'static str,
    pub(crate) expanded_definition_id: &'static str,
    pub(crate) expected_state: &'static str,
    pub(crate) expected_blocker: Option<&'static str>,
    pub(crate) callsite_label: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct ExternalSummaryCase {
    pub(crate) path: &'static [&'static str],
    pub(crate) records: fn(Uuid) -> Vec<serde_json::Value>,
    pub(crate) summary_id: &'static str,
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
        status: CallStatusKind::Resolved,
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
        label: "axum/src/handler/service.rs:174 IntoServiceFuture::new generated constructor",
        item: "call",
        path: &["super", "future", "IntoServiceFuture", "new"],
        status: CallStatusKind::Resolved,
        corpus: DynamicToolCorpus::Axum,
        owner: PathOwner::Method {
            trait_name: "Service<Request>",
            type_name: "HandlerService",
            module_path: &["crate", "handler", "service"],
            file_suffix: "axum/src/handler/service.rs",
            body: "super::future::IntoServiceFuture::new(future)",
        },
    }];

    pub(crate) const ROUTING_POST: [Self; 1] = [Self {
        label: "axum/src/json.rs:248 generated routing::post handler",
        item: "deserialize_body",
        path: &["post"],
        status: CallStatusKind::Resolved,
        corpus: DynamicToolCorpus::Axum,
        owner: PathOwner::Function {
            module_path: &["crate", "json", "tests"],
            file_suffix: "axum/src/json.rs",
            body: "post(|input: Json<Input>| async { input.0.foo })",
        },
    }];

    pub(crate) const ROUTING_GET_SERVICE: [Self; 1] = [Self {
        label: "axum/src/routing/tests/get_to_head.rs:46 generated routing::get_service",
        item: "get_handles_head",
        path: &["get_service"],
        status: CallStatusKind::Resolved,
        corpus: DynamicToolCorpus::Axum,
        owner: PathOwner::Function {
            module_path: &["crate", "routing", "tests", "get_to_head", "for_services"],
            file_suffix: "axum/src/routing/tests/get_to_head.rs",
            body: "get_service(service_fn(|_req: Request| async move",
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

    pub(crate) fn db_status(&self) -> DbCallStatusKind {
        match &self.status {
            CallStatusKind::Resolved => DbCallStatusKind::Resolved,
            CallStatusKind::Unresolved => DbCallStatusKind::Unresolved,
            CallStatusKind::Ambiguous => DbCallStatusKind::Ambiguous,
            CallStatusKind::External => DbCallStatusKind::External,
            CallStatusKind::Unsupported => DbCallStatusKind::Unsupported,
        }
    }

    pub(crate) fn expected_resolved_relation(&self) -> CallTargetKind {
        if (self.path == ["post"] || self.path == ["get_service"])
            && matches!(
                self.owner,
                PathOwner::Function {
                    file_suffix: "axum/src/json.rs" | "axum/src/routing/tests/get_to_head.rs",
                    ..
                }
            )
        {
            return CallTargetKind::Function;
        }

        CallTargetKind::AssociatedFunction
    }

    pub(crate) fn admitted_external_summary(&self) -> Option<ExternalSummaryCase> {
        if self.path == ["std", "mem", "replace"]
            && matches!(
                self.owner,
                PathOwner::Method {
                    file_suffix: "axum/src/response/sse.rs",
                    ..
                }
            )
        {
            return Some(ExternalSummaryCase {
                path: &["std", "mem", "replace"],
                records: axum_std_mem_replace_summary_records,
                summary_id: AXUM_STD_MEM_REPLACE_SUMMARY_ID,
            });
        }

        if self.path == ["Request", "builder"]
            && matches!(
                self.owner,
                PathOwner::Function {
                    file_suffix: "axum/src/middleware/from_fn.rs",
                    ..
                }
            )
        {
            return Some(ExternalSummaryCase {
                path: &["Request", "builder"],
                records: axum_request_builder_summary_records,
                summary_id: AXUM_REQUEST_BUILDER_SUMMARY_ID,
            });
        }

        None
    }

    pub(crate) fn admitted_macro_boundary_summary(&self) -> Option<MacroBoundaryCase> {
        if self.path == ["super", "future", "IntoServiceFuture", "new"]
            && matches!(
                self.owner,
                PathOwner::Method {
                    file_suffix: "axum/src/handler/service.rs",
                    ..
                }
            )
        {
            return Some(MacroBoundaryCase {
                boundary_id: axum_opaque_future_boundary_id,
                records: axum_opaque_future_macro_summary_records,
                summary_id: AXUM_OPAQUE_FUTURE_SUMMARY_ID,
                expanded_item_id: "expanded:item:axum-opaque-future-new",
                expanded_definition_id: "def:axum::future::IntoServiceFuture::new",
                expected_state: "resolved",
                expected_blocker: None,
                callsite_label: "generated constructor",
            });
        }

        if self.path == ["post"]
            && matches!(
                self.owner,
                PathOwner::Function {
                    file_suffix: "axum/src/json.rs",
                    ..
                }
            )
        {
            return Some(MacroBoundaryCase {
                boundary_id: axum_routing_post_boundary_id,
                records: axum_routing_post_macro_summary_records,
                summary_id: AXUM_ROUTING_POST_SUMMARY_ID,
                expanded_item_id: "expanded:item:axum-routing-post",
                expanded_definition_id: "def:axum::routing::method_routing::post",
                expected_state: "resolved",
                expected_blocker: None,
                callsite_label: "generated routing::post",
            });
        }

        if self.path == ["get_service"]
            && matches!(
                self.owner,
                PathOwner::Function {
                    file_suffix: "axum/src/routing/tests/get_to_head.rs",
                    ..
                }
            )
        {
            return Some(MacroBoundaryCase {
                boundary_id: axum_routing_get_service_boundary_id,
                records: axum_routing_get_service_macro_summary_records,
                summary_id: AXUM_ROUTING_GET_SERVICE_SUMMARY_ID,
                expanded_item_id: "expanded:item:axum-routing-get-service",
                expanded_definition_id: "def:axum::routing::method_routing::get_service",
                expected_state: "resolved",
                expected_blocker: None,
                callsite_label: "generated routing::get_service",
            });
        }

        None
    }

    pub(crate) fn expects_runtime_dispatch_blocker(&self) -> bool {
        matches!(self.corpus, DynamicToolCorpus::Memchr)
            && matches!(self.path, ["fwd"] | ["rev"])
            && matches!(
                self.owner,
                PathOwner::Method {
                    type_name: "Runner",
                    file_suffix: "src/tests/substring/mod.rs",
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

impl AmbiguousDynamicToolFixture {
    pub(crate) async fn new(case: AmbiguousDynamicToolCase) -> Self {
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
        let candidates = ambiguous_dynamic_candidates(&db, &case);
        assert!(
            db.project_call_proof_facts_for_node(owner.id, case.build_domain())
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label))
                >= 2,
            "{} should project ambiguous dynamic call-site proof rows",
            case.label
        );
        let state = axum_state_for_target(Arc::clone(&db), &owner, case.label).await;

        Self {
            state,
            case,
            file_path: owner.file_path,
            module_path: owner.module_path,
            owner: owner.id,
            candidates,
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
            "{} should project receiver proof rows",
            case.label
        );
        attach_receiver_admitted_external_summary_if_needed(&db, owner.id, &case);
        attach_runtime_dispatch_blocker_if_needed(&db, owner.id, &case);
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
        .id;

    db.upsert_proof_fact_values(&[ploke_test_utils::axum_dyn_future_poll_blocker(site)])
        .unwrap_or_else(|err| panic!("{} runtime dispatch blocker insert: {err}", case.label));
}

fn attach_receiver_admitted_external_summary_if_needed(
    db: &Database,
    owner: Uuid,
    case: &ReceiverToolCase,
) {
    let Some(summary) = case.admitted_external_summary() else {
        return;
    };
    let site = db
        .call_context_for_owner(owner)
        .unwrap_or_else(|err| panic!("{} call context lookup: {err}", case.label))
        .into_iter()
        .find(|row| {
            row.site.method.as_deref() == Some(case.callee) && row.status.status == case.db_status()
        })
        .unwrap_or_else(|| {
            panic!(
                "{} should expose the external receiver callsite before summary insertion",
                case.label
            )
        })
        .site
        .id;

    db.upsert_proof_fact_values(&(summary.records)(site))
        .unwrap_or_else(|err| panic!("{} external summary insert: {err}", case.label));
}

pub(crate) fn request_parts_extract_target(db: &Database) -> Uuid {
    method_target_by_body_and_file(
        db,
        "extract_with_state",
        "E::from_request_parts(self, state)",
        "axum-core/src/ext_traits/request_parts.rs",
    )
    .id
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
        attach_admitted_macro_boundary_summary_if_needed(&db, owner.id, &case);
        attach_path_runtime_dispatch_blocker_if_needed(&db, owner.id, &case);
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
    let Some(summary) = case.admitted_external_summary() else {
        return;
    };
    let site = db
        .call_context_for_owner(owner)
        .unwrap_or_else(|err| panic!("{} call context lookup: {err}", case.label))
        .into_iter()
        .find(|row| {
            row.site.path.as_ref().is_some_and(|path| {
                path.iter()
                    .map(String::as_str)
                    .eq(summary.path.iter().copied())
            }) && row.status.status == case.db_status()
        })
        .unwrap_or_else(|| {
            panic!(
                "{} should expose the external callsite before summary insertion",
                case.label,
            )
        })
        .site
        .id;

    db.upsert_proof_fact_values(&(summary.records)(site))
        .unwrap_or_else(|err| panic!("{} admitted external summary insert: {err}", case.label));
}

fn attach_admitted_macro_boundary_summary_if_needed(
    db: &Database,
    owner: Uuid,
    case: &PathToolCase,
) {
    let Some(boundary) = case.admitted_macro_boundary_summary() else {
        return;
    };
    let rows = db
        .call_context_for_owner(owner)
        .unwrap_or_else(|err| panic!("{} call context lookup: {err}", case.label));
    let site = rows
        .iter()
        .find(|row| {
            row.site.path.as_ref().is_some_and(|path| {
                path.iter()
                    .map(String::as_str)
                    .eq(case.path.iter().copied())
            }) && row.status.status == case.db_status()
        })
        .unwrap_or_else(|| {
            panic!(
                "{} should expose the generated macro-boundary callsite with {:?} before summary insertion: {rows:#?}",
                case.label,
                case.db_status(),
            )
        })
        .site
        .id;

    db.upsert_proof_fact_values(&(boundary.records)(site))
        .unwrap_or_else(|err| panic!("{} admitted macro summary insert: {err}", case.label));
}

fn attach_path_runtime_dispatch_blocker_if_needed(db: &Database, owner: Uuid, case: &PathToolCase) {
    if !case.expects_runtime_dispatch_blocker() {
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
                    .eq(case.path.iter().copied())
            }) && row.status.status == case.db_status()
        })
        .unwrap_or_else(|| {
            panic!(
                "{} should expose the boxed dyn FnMut path callsite before blocker insertion",
                case.label,
            )
        })
        .site
        .id;

    db.upsert_proof_fact_values(&[
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_blocker(site),
    ])
    .unwrap_or_else(|err| panic!("{} runtime dispatch blocker insert: {err}", case.label));
}

pub(crate) fn assert_dynamic_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    expected_path: Option<&[&str]>,
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
    if let Some(expected) = expected_path {
        assert!(
            call.path
                .as_ref()
                .is_some_and(|path| path.iter().map(String::as_str).eq(expected.iter().copied())),
            "{tool} should preserve dynamic callee path for {label}: {call:#?}"
        );
    }
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

pub(crate) fn assert_resolved_method_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    target: Uuid,
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
        "{tool} should return exactly one resolved method row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    if let Some(expected) = generic_arg_count {
        assert_eq!(
            call.generic_arg_count,
            Some(expected),
            "{tool} should preserve method generic argument count for {label}: {call:#?}"
        );
    }
    assert_eq!(
        call.targets.len(),
        1,
        "{tool} target rows for {label}: {call:#?}"
    );
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);
    call.site_id
}

pub(crate) fn assert_resolved_method_target_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    target: Uuid,
    relation: CallTargetKind,
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
        "{tool} should return exactly one resolved method row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    if let Some(expected) = generic_arg_count {
        assert_eq!(
            call.generic_arg_count,
            Some(expected),
            "{tool} should preserve method generic argument count for {label}: {call:#?}"
        );
    }
    assert_eq!(
        call.targets.len(),
        1,
        "{tool} target rows for {label}: {call:#?}"
    );
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, relation);
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

pub(crate) fn assert_ambiguous_path_candidates(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    expected: &[Uuid],
    label: &str,
    tool: &str,
) -> Uuid {
    let matching = matching_path_context(calls, owner, callee);
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one ambiguous path row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::Ambiguous);
    assert_eq!(call.resolution, None);
    assert_eq!(
        call.targets.len(),
        expected.len(),
        "{tool} should expose every candidate for {label}: {call:#?}"
    );
    assert!(
        call.targets
            .iter()
            .all(|target| target.relation == CallTargetKind::Function),
        "{tool} should expose only function candidates for {label}: {call:#?}"
    );
    let mut actual = call
        .targets
        .iter()
        .map(|target| target.target_id)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{tool} candidate targets for {label}");
    call.site_id
}

pub(crate) fn assert_ambiguous_dynamic_candidates(
    calls: &[serde_json::Value],
    owner: Uuid,
    expected: &[Uuid],
    label: &str,
    tool: &str,
) -> Uuid {
    assert_ambiguous_dynamic_candidates_with_relation(
        calls,
        owner,
        None,
        None,
        expected,
        CallTargetKind::DynamicFunction,
        label,
        tool,
    )
}

pub(crate) fn assert_ambiguous_dynamic_candidates_with_relation(
    calls: &[serde_json::Value],
    owner: Uuid,
    expected_path: Option<&[&str]>,
    expected_arg_count: Option<u32>,
    expected: &[Uuid],
    expected_relation: CallTargetKind,
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
        "{tool} should return exactly one ambiguous dynamic row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    if let Some(expected) = expected_path {
        assert!(
            call.path
                .as_ref()
                .is_some_and(|path| path.iter().map(String::as_str).eq(expected.iter().copied())),
            "{tool} should preserve dynamic callee path for {label}: {call:#?}"
        );
    }
    if let Some(expected) = expected_arg_count {
        assert_eq!(
            call.arg_count,
            Some(expected),
            "{tool} should preserve dynamic arg count for {label}: {call:#?}"
        );
    }
    assert_eq!(call.status, CallStatusKind::Ambiguous);
    assert_eq!(call.resolution, None);
    assert_eq!(
        call.targets.len(),
        expected.len(),
        "{tool} should expose every candidate for {label}: {call:#?}"
    );
    assert!(
        call.targets
            .iter()
            .all(|target| target.relation == expected_relation),
        "{tool} should expose only {expected_relation:?} candidates for {label}: {call:#?}"
    );
    let mut actual = call
        .targets
        .iter()
        .map(|target| target.target_id)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{tool} candidate targets for {label}");
    call.site_id
}

pub(crate) fn assert_resolved_path_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    target: Uuid,
    relation: CallTargetKind,
    label: &str,
    tool: &str,
) -> Uuid {
    let (site_id, actual) =
        assert_resolved_path_context_target(calls, owner, callee, relation, label, tool);
    assert_eq!(actual, target, "{tool} target for {label}");
    site_id
}

pub(crate) fn assert_resolved_path_context_target(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    relation: CallTargetKind,
    label: &str,
    tool: &str,
) -> (Uuid, Uuid) {
    let matching = matching_path_context(calls, owner, callee);
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one resolved path row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(
        call.targets.len(),
        1,
        "{tool} target rows for {label}: {call:#?}"
    );
    assert_eq!(call.targets[0].relation, relation);
    (call.site_id, call.targets[0].target_id)
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

pub(crate) fn assert_resolved_path_context_count(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    relation: CallTargetKind,
    expected_count: usize,
    label: &str,
    tool: &str,
) -> Vec<Uuid> {
    let matching = matching_path_context(calls, owner, callee);
    assert_eq!(
        matching.len(),
        expected_count,
        "{tool} should return exactly {expected_count} resolved path rows for {label}: {calls:#?}"
    );
    for call in &matching {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(
            call.targets.len(),
            1,
            "{tool} target rows for {label}: {call:#?}"
        );
        assert_eq!(call.targets[0].relation, relation);
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

pub(crate) fn assert_resolved_method_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    target: Uuid,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let site = site_id.to_string();
    let target = target.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.build_domain_id.as_deref() == Some("bd:corpus-axum-call-graph")
        }),
        "{tool} should return the resolved method call_site proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_edge"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.callee_def_id.as_deref() == Some(target.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
        }),
        "{tool} should return the resolved method call_edge proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
                && proof.resolved_def_id.as_deref() == Some(target.as_str())
        }),
        "{tool} should return the resolved method call_resolution proof row for {label}: {proofs:#?}"
    );
}

pub(crate) fn assert_resolved_path_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    target: Uuid,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let site = site_id.to_string();
    let target = target.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.build_domain_id.as_deref() == Some("bd:corpus-axum-call-graph")
        }),
        "{tool} should return the resolved path call_site proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_edge"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.callee_def_id.as_deref() == Some(target.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
        }),
        "{tool} should return the resolved path call_edge proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
                && proof.resolved_def_id.as_deref() == Some(target.as_str())
                && proof.blocker_reason.is_none()
        }),
        "{tool} should return the resolved path call_resolution proof row for {label}: {proofs:#?}"
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
    summary: ExternalSummaryCase,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let site_id = site_id.to_string();
    const BUILD_DOMAIN: &str = "bd:corpus-axum-call-graph";
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
                && proof.external_summary_id.as_deref() == Some(summary.summary_id)
                && proof.blocker_reason.is_none()
        }),
        "{tool} should return the externally summarized call_resolution for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary.summary_id)
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

pub(crate) fn assert_admitted_external_summary_effect(
    effects: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    summary: ExternalSummaryCase,
    label: &str,
    tool: &str,
) {
    let rows = effects
        .iter()
        .filter_map(|effect| serde_json::from_value::<CallReachEffectInfo>(effect.clone()).ok())
        .collect::<Vec<_>>();
    let effect_id = format!(
        "summary-effect:{}:external_summary_boundary",
        summary.summary_id
    );
    let effect = rows
        .iter()
        .find(|effect| effect.effect_seed_id == effect_id)
        .unwrap_or_else(|| {
            panic!("{tool} should return the admitted external summary effect for {label}: {effects:#?}")
        });
    assert_eq!(effect.effect_class, "external_summary_boundary");
    assert_eq!(effect.confidence.as_deref(), Some("source-oracle-review"));
    assert_eq!(effect.blocker_if_unresolved, Some(false));
    assert_eq!(effect.call_site.site_id, site_id);
    assert_eq!(effect.call_site.owner_id, owner);
    assert_eq!(effect.call_site.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "{tool} direct external summary effect should not need an intermediate path for {label}: {effect:#?}"
    );
    assert!(
        effect.blocker_reasons.is_empty(),
        "{tool} should not retain missing-summary blockers on admitted summary effect for {label}: {effect:#?}"
    );
    assert!(
        effect.call_site.targets.is_empty(),
        "{tool} summary-derived effects must not fabricate target rows for {label}: {effect:#?}"
    );
}

pub(crate) fn assert_admitted_macro_boundary_summary_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    boundary: MacroBoundaryCase,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let site = site_id.to_string();
    let boundary_id = (boundary.boundary_id)(site_id);
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.build_domain_id.as_deref() == Some("bd:corpus-axum-call-graph")
        }),
        "{tool} should return the {} call_site proof row for {label}: {proofs:#?}",
        boundary.callsite_label
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some(boundary.expected_state)
                && proof.blocker_reason.as_deref() == boundary.expected_blocker
        }),
        "{tool} should preserve the {} call_resolution state for {label}: {proofs:#?}",
        boundary.callsite_label
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "expansion_boundary"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.boundary_id.as_deref() == Some(boundary_id.as_str())
                && proof.external_summary_id.as_deref() == Some(boundary.summary_id)
                && proof.status.as_deref() == Some("externally_summarized")
                && proof.blocker_reason.is_none()
        }),
        "{tool} should return the admitted expansion boundary for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(boundary.summary_id)
                && proof.summary_class.as_deref() == Some("audited_no_process_effects")
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == vec!["external_summary_boundary".to_string()]
        }),
        "{tool} should return the admitted macro-boundary summary artifact for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "expanded_item"
                && proof.expanded_item_id.as_deref() == Some(boundary.expanded_item_id)
                && proof.boundary_id.as_deref() == Some(boundary_id.as_str())
                && proof.definition_id.as_deref() == Some(boundary.expanded_definition_id)
        }),
        "{tool} should return the expanded generated-item linkage for {label}: {proofs:#?}"
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

pub(crate) fn assert_ambiguous_candidate_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    build_domain: &str,
    expected_candidates: &[Uuid],
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let site = site_id.to_string();
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
        "{tool} should return the ambiguous call_site proof row for {label}: {proofs:#?}"
    );
    let resolution = rows
        .iter()
        .find(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("ambiguous")
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should return the ambiguous call_resolution proof row for {label}: {proofs:#?}"
            )
        });
    let mut actual = resolution.candidate_def_ids.clone();
    actual.sort();
    let mut expected = expected_candidates
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        actual, expected,
        "{tool} should preserve every ambiguous candidate for {label}"
    );
    assert!(
        rows.iter().all(|proof| {
            proof.kind != "call_edge" || proof.call_site_id.as_deref() != Some(site.as_str())
        }),
        "{tool} should not fabricate a call_edge for ambiguous candidate row {label}: {proofs:#?}"
    );
}

fn axum_layer_dynamic_candidates(db: &Database) -> Vec<Uuid> {
    let layer = owner_by_body(
        db,
        "layer",
        "MethodRouter",
        Some(&["crate", "routing", "method_routing"]),
        "axum/src/routing/method_routing.rs",
        "let layer_fn = move |route: Route<E>| route.layer(layer.clone());",
        "MethodRouter::layer layer_fn closure",
    )
    .id;
    let route_layer = owner_by_body(
        db,
        "route_layer",
        "MethodRouter",
        Some(&["crate", "routing", "method_routing"]),
        "axum/src/routing/method_routing.rs",
        "let layer_fn = move |svc| Route::new(layer.layer(svc));",
        "MethodRouter::route_layer layer_fn closure",
    )
    .id;
    let mut candidates = vec![
        closure_owner_for_method_parent(db, layer),
        closure_owner_for_method_parent(db, route_layer),
    ];
    candidates.sort_unstable();
    candidates
}

fn ambiguous_dynamic_candidates(db: &Database, case: &AmbiguousDynamicToolCase) -> Vec<Uuid> {
    if case.candidate_names.is_empty() {
        return axum_layer_dynamic_candidates(db);
    }

    let mut candidates = case
        .candidate_names
        .iter()
        .map(|name| function_id_by_exact_name(db, name))
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates
}

fn function_id_by_exact_name(db: &Database, name: &str) -> Uuid {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    let rows = db
        .raw_query_params(
            r#"?[id] :=
                *function { id, name: $name @ 'NOW' }"#,
            params,
        )
        .unwrap_or_else(|err| panic!("query function {name}: {err}"));
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one function named {name:?}: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0]).unwrap_or_else(|err| panic!("{name} uuid: {err}"))
}

fn closure_owner_for_method_parent(db: &Database, parent: Uuid) -> Uuid {
    let rows = db
        .raw_query(&format!(
            r#"?[id, kind, parent_kind, name] :=
                parent = to_uuid("{parent}"),
                *call_body_owner {{
                    id,
                    owner_kind: kind,
                    parent_id: parent,
                    parent_kind,
                    label: name @ 'NOW'
                }},
                kind = "Closure",
                name = "closure""#
        ))
        .unwrap_or_else(|err| panic!("query closure owner for method parent {parent}: {err}"));
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one closure call_body_owner row for method parent {parent}: {:#?}",
        rows.rows
    );
    assert_eq!(
        data_str(&rows.rows[0][1], "call_body_owner.owner_kind"),
        "Closure"
    );
    assert_eq!(
        data_str(&rows.rows[0][2], "call_body_owner.parent_kind"),
        "Method"
    );
    assert_eq!(
        data_str(&rows.rows[0][3], "call_body_owner.label"),
        "closure"
    );
    to_uuid(&rows.rows[0][0])
        .unwrap_or_else(|err| panic!("closure call_body_owner uuid for {parent}: {err}"))
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
