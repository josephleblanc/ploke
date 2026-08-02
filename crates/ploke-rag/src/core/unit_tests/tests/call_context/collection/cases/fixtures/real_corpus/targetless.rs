use super::*;

struct DynamicCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    expected_path: &'static [&'static str],
}

struct MemchrPathCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    path: &'static [&'static str],
    expected_arg_count: u32,
}

enum CaseOwner {
    Method {
        name: &'static str,
        body: &'static str,
    },
    MethodFile {
        name: &'static str,
        body: &'static str,
        file: &'static str,
    },
    Function {
        module: &'static [&'static str],
        name: &'static str,
    },
    MethodsFile {
        name: &'static str,
        body: &'static str,
        file: &'static str,
        count: usize,
    },
}

enum ReceiverCase {
    AwaitPathResult {
        path: &'static [&'static str],
    },
    PathResult {
        path: &'static [&'static str],
    },
    MethodField {
        method: &'static str,
        field: &'static [&'static str],
    },
    SelfField {
        path: &'static [&'static str],
    },
}

enum CalleeCase {
    Dynamic {
        path: &'static [&'static str],
    },
    Path {
        path: &'static [&'static str],
    },
    Method {
        name: &'static str,
        receiver: ReceiverCase,
    },
}

enum TargetCase {
    Empty,
    Relations(&'static [(CallTargetKind, usize)]),
    FunctionCandidates {
        names: &'static [&'static str],
        relation: CallTargetKind,
    },
    AxumLayerCandidates {
        relation: CallTargetKind,
    },
}

struct CallCase {
    label: &'static str,
    owner: CaseOwner,
    callee: CalleeCase,
    count: usize,
    args: Option<u32>,
    status: CallStatusKind,
    resolution: Option<CallResolutionKind>,
    targets: TargetCase,
}

const AXUM_CALLABLE_FIELD_CASES: [DynamicCase; 2] = [
    DynamicCase {
        label: "axum/src/boxed.rs:120 MakeErasedRouter::into_route callable field",
        method: "into_route",
        body: "(self.into_route)(self.router, state)",
        expected_path: &["self", "into_route"],
    },
    DynamicCase {
        label: "axum/src/serve/listener.rs:236 TapIo::accept callable field",
        method: "accept",
        body: "(self.tap_fn)(&mut io)",
        expected_path: &["self", "tap_fn"],
    },
];

const MEMCHR_CALLABLE_TRAIT_OBJECT_CASES: [MemchrPathCase; 2] = [
    MemchrPathCase {
        label: "memchr/src/tests/substring/mod.rs:94 Runner.fwd boxed dyn FnMut",
        method: "run",
        body: "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
        path: &["fwd"],
        expected_arg_count: 2,
    },
    MemchrPathCase {
        label: "memchr/src/tests/substring/mod.rs:110 Runner.rev boxed dyn FnMut",
        method: "run",
        body: "rev(t.haystack.as_bytes(), t.needle.as_bytes())",
        path: &["rev"],
        expected_arg_count: 2,
    },
];

// Exact read-only source-oracle ledger. Labels retain the reviewed source
// locations; the remaining fields encode each traversal contract.
const AXUM_READ_CASES: &[CallCase] = &[
    CallCase {
        label: "axum/src/boxed.rs:85 MakeErasedHandler::into_route callable field",
        owner: CaseOwner::Method {
            name: "into_route",
            body: "(self.into_route)(self.handler, state)",
        },
        callee: CalleeCase::Dynamic {
            path: &["self", "into_route"],
        },
        count: 1,
        args: Some(2),
        status: CallStatusKind::Resolved,
        resolution: Some(CallResolutionKind::LocalExact),
        targets: TargetCase::Relations(&[(CallTargetKind::DynamicClosure, 1)]),
    },
    CallCase {
        label: "axum/src/boxed.rs:159 Map::into_route layer trait object",
        owner: CaseOwner::Method {
            name: "into_route",
            body: "(self.layer)(self.inner.into_route(state))",
        },
        callee: CalleeCase::Dynamic {
            path: &["self", "layer"],
        },
        count: 1,
        args: Some(1),
        status: CallStatusKind::Ambiguous,
        resolution: None,
        targets: TargetCase::AxumLayerCandidates {
            relation: CallTargetKind::DynamicClosure,
        },
    },
    CallCase {
        label: "axum/src/boxed.rs:163 Map::call_with_state layer trait object",
        owner: CaseOwner::Method {
            name: "call_with_state",
            body: "(self.layer)(self.inner.into_route(state)).call(request)",
        },
        callee: CalleeCase::Dynamic {
            path: &["self", "layer"],
        },
        count: 1,
        args: Some(1),
        status: CallStatusKind::Ambiguous,
        resolution: None,
        targets: TargetCase::AxumLayerCandidates {
            relation: CallTargetKind::DynamicClosure,
        },
    },
    CallCase {
        label: "axum-macros/src/lib.rs:724 expand_with and_then callback",
        owner: CaseOwner::Function {
            module: &["crate"],
            name: "expand_with",
        },
        callee: CalleeCase::Method {
            name: "and_then",
            receiver: ReceiverCase::PathResult {
                path: &["syn", "parse"],
            },
        },
        count: 1,
        args: Some(1),
        status: CallStatusKind::Ambiguous,
        resolution: None,
        targets: TargetCase::Relations(&[
            (CallTargetKind::MethodCallbackFunction, 1),
            (CallTargetKind::MethodCallbackClosure, 3),
        ]),
    },
    CallCase {
        label: "axum/src/handler/service.rs:174 IntoServiceFuture::new",
        owner: CaseOwner::Method {
            name: "call",
            body: "IntoServiceFuture::new(future)",
        },
        callee: CalleeCase::Path {
            path: &["super", "future", "IntoServiceFuture", "new"],
        },
        count: 1,
        args: Some(1),
        status: CallStatusKind::Resolved,
        resolution: Some(CallResolutionKind::LocalExact),
        targets: TargetCase::Relations(&[(CallTargetKind::AssociatedFunction, 1)]),
    },
    CallCase {
        label: "axum/src/routing/route.rs:51 Route::oneshot_inner",
        owner: CaseOwner::Method {
            name: "oneshot_inner",
            body: "self.0.clone().oneshot(req)",
        },
        callee: CalleeCase::Method {
            name: "oneshot",
            receiver: ReceiverCase::MethodField {
                method: "clone",
                field: &["0"],
            },
        },
        count: 1,
        args: None,
        status: CallStatusKind::External,
        resolution: None,
        targets: TargetCase::Empty,
    },
    CallCase {
        label: "axum/src/routing/route.rs:57 Route::oneshot_inner_owned",
        owner: CaseOwner::Method {
            name: "oneshot_inner_owned",
            body: "self.0.oneshot(req)",
        },
        callee: CalleeCase::Method {
            name: "oneshot",
            receiver: ReceiverCase::SelfField { path: &["0"] },
        },
        count: 1,
        args: None,
        status: CallStatusKind::External,
        resolution: None,
        targets: TargetCase::Empty,
    },
    CallCase {
        label: "axum/src/error_handling/mod.rs:251 HandleErrorFuture::poll dyn Future",
        owner: CaseOwner::MethodFile {
            name: "poll",
            body: "self.project().future.poll(cx)",
            file: "axum/src/error_handling/mod.rs",
        },
        callee: CalleeCase::Method {
            name: "poll",
            receiver: ReceiverCase::MethodField {
                method: "project",
                field: &["future"],
            },
        },
        count: 1,
        args: None,
        status: CallStatusKind::Unsupported,
        resolution: None,
        targets: TargetCase::Empty,
    },
    CallCase {
        label: "axum-core/src/body.rs:127 Body::size_hint self field",
        owner: CaseOwner::Method {
            name: "size_hint",
            body: "self.0.size_hint()",
        },
        callee: CalleeCase::Method {
            name: "size_hint",
            receiver: ReceiverCase::SelfField { path: &["0"] },
        },
        count: 1,
        args: None,
        status: CallStatusKind::External,
        resolution: None,
        targets: TargetCase::Empty,
    },
    CallCase {
        label: "axum/src/middleware/from_fn.rs:411 Request::builder alias",
        owner: CaseOwner::Function {
            module: &["crate", "middleware", "from_fn", "tests"],
            name: "basic",
        },
        callee: CalleeCase::Path {
            path: &["Request", "builder"],
        },
        count: 1,
        args: Some(0),
        status: CallStatusKind::External,
        resolution: None,
        targets: TargetCase::Empty,
    },
    CallCase {
        label: "axum/src/routing/tests/mod.rs:412-413 imported get boundary",
        owner: CaseOwner::Function {
            module: &["crate", "routing", "tests"],
            name: "what_matches_wildcard",
        },
        callee: CalleeCase::Path { path: &["get"] },
        count: 2,
        args: Some(1),
        status: CallStatusKind::Resolved,
        resolution: Some(CallResolutionKind::LocalExact),
        targets: TargetCase::Relations(&[(CallTargetKind::Function, 1)]),
    },
    CallCase {
        label: "axum/src/response/sse.rs:449 EventDataWriter::write_buf",
        owner: CaseOwner::Method {
            name: "write_buf",
            body: "std::mem::replace(&mut self.data_written, true)",
        },
        callee: CalleeCase::Path {
            path: &["std", "mem", "replace"],
        },
        count: 1,
        args: Some(2),
        status: CallStatusKind::External,
        resolution: None,
        targets: TargetCase::Empty,
    },
    CallCase {
        label: "from_fn all_the_tuples!(impl_service)",
        owner: CaseOwner::MethodsFile {
            name: "call",
            body: "std::mem::replace(&mut self.inner, not_ready_inner)",
            file: "axum/src/middleware/from_fn.rs",
            count: 16,
        },
        callee: CalleeCase::Path {
            path: &["std", "mem", "replace"],
        },
        count: 1,
        args: Some(2),
        status: CallStatusKind::External,
        resolution: None,
        targets: TargetCase::Empty,
    },
    CallCase {
        label: "map_request all_the_tuples!(impl_service)",
        owner: CaseOwner::MethodsFile {
            name: "call",
            body: "std::mem::replace(&mut self.inner, not_ready_inner)",
            file: "axum/src/middleware/map_request.rs",
            count: 16,
        },
        callee: CalleeCase::Path {
            path: &["std", "mem", "replace"],
        },
        count: 1,
        args: Some(2),
        status: CallStatusKind::External,
        resolution: None,
        targets: TargetCase::Empty,
    },
    CallCase {
        label: "map_response direct impl_service!(...) arities",
        owner: CaseOwner::MethodsFile {
            name: "call",
            body: "std::mem::replace(&mut self.inner, not_ready_inner)",
            file: "axum/src/middleware/map_response.rs",
            count: 17,
        },
        callee: CalleeCase::Path {
            path: &["std", "mem", "replace"],
        },
        count: 1,
        args: Some(2),
        status: CallStatusKind::External,
        resolution: None,
        targets: TargetCase::Empty,
    },
];

const MEMCHR_READ_CASES: &[CallCase] = &[
    CallCase {
        label: "memchr/src/memmem/searcher.rs:222 Searcher.call",
        owner: CaseOwner::Method {
            name: "find",
            body: "(self.call)(self, prestate, haystack, needle)",
        },
        callee: CalleeCase::Dynamic {
            path: &["self", "call"],
        },
        count: 1,
        args: Some(4),
        status: CallStatusKind::Ambiguous,
        resolution: None,
        targets: TargetCase::FunctionCandidates {
            names: &[
                "searcher_kind_empty",
                "searcher_kind_one_byte",
                "searcher_kind_two_way",
                "searcher_kind_two_way_with_prefilter",
                "searcher_kind_sse2",
                "searcher_kind_avx2",
            ],
            relation: CallTargetKind::DynamicFunction,
        },
    },
    CallCase {
        label: "memchr/src/memmem/searcher.rs:718 Prefilter.call",
        owner: CaseOwner::Method {
            name: "find",
            body: "(self.call)(self, haystack)",
        },
        callee: CalleeCase::Dynamic {
            path: &["self", "call"],
        },
        count: 1,
        args: Some(2),
        status: CallStatusKind::Ambiguous,
        resolution: None,
        targets: TargetCase::FunctionCandidates {
            names: &[
                "prefilter_kind_fallback",
                "prefilter_kind_sse2",
                "prefilter_kind_avx2",
            ],
            relation: CallTargetKind::DynamicFunction,
        },
    },
];

#[tokio::test]
async fn call_context_collection_reads_axum_targetless_matrix() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    for source in AXUM_CALLABLE_FIELD_CASES {
        let case = CallCase {
            label: source.label,
            owner: CaseOwner::Method {
                name: source.method,
                body: source.body,
            },
            callee: CalleeCase::Dynamic {
                path: source.expected_path,
            },
            count: 1,
            args: None,
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: TargetCase::Empty,
        };
        assert_call_case(&db, &rag, &case)?;
    }

    for case in AXUM_READ_CASES {
        assert_call_case(&db, &rag, case)?;
    }
    assert_axum_nested_cases(&db, &rag)?;

    Ok(())
}

fn assert_axum_nested_cases(db: &Database, rag: &RagService) -> Result<(), Error> {
    let parent = function_id_by_name_in_module(db, &["crate"], "expand_attr_with")?;
    let owner = closure_owner_for_parent(db, parent)?;
    let call_context = rag.collect_call_context(&[(parent, 1.0), (owner, 1.0)])?;
    if let Some(context) = call_context.get(&parent) {
        assert!(
            context
                .iter()
                .all(|call| call.callee != CallCalleeInfo::Path { path: path(&["f"]) }),
            "expand_attr_with must not absorb the closure-owned f(attr, input) row: {context:#?}"
        );
    }
    let context = call_context
        .get(&owner)
        .expect("expand_attr_with IIFE closure owner should receive outgoing call context");
    assert_context_case(
        db,
        owner,
        context,
        &CallCase {
            label: "axum-macros/src/lib.rs:737 captured callback f(attr, input)",
            owner: CaseOwner::Function {
                module: &["crate"],
                name: "expand_attr_with",
            },
            callee: CalleeCase::Path { path: &["f"] },
            count: 1,
            args: Some(2),
            status: CallStatusKind::Ambiguous,
            resolution: None,
            targets: TargetCase::Relations(&[(CallTargetKind::Closure, 2)]),
        },
    )?;

    let parent = method_id_by_file(
        db,
        "call",
        "self().await.into_response()",
        "axum/src/handler/mod.rs",
    )?;
    let owner = async_block_owner_for_method_parent(db, parent)?;
    let call_context = rag.collect_call_context(&[(parent, 1.0), (owner, 1.0)])?;
    if let Some(context) = call_context.get(&parent) {
        assert!(
            context.iter().all(|call| {
                call.callee
                    != CallCalleeInfo::Path {
                        path: path(&["self"]),
                    }
            }),
            "Handler::call must not absorb the async-block self() row: {context:#?}"
        );
        assert!(
            context.iter().all(|call| {
                !matches!(&call.callee, CallCalleeInfo::Method { name, .. } if name == "into_response")
            }),
            "Handler::call must not absorb the async-block into_response() row: {context:#?}"
        );
    }
    let context = call_context
        .get(&owner)
        .expect("Handler::call async block owner should receive outgoing call context");
    for case in [
        CallCase {
            label: "axum/src/handler/mod.rs:217 async-block self()",
            owner: CaseOwner::MethodFile {
                name: "call",
                body: "self().await.into_response()",
                file: "axum/src/handler/mod.rs",
            },
            callee: CalleeCase::Path { path: &["self"] },
            count: 1,
            args: None,
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: TargetCase::Empty,
        },
        CallCase {
            label: "axum/src/handler/mod.rs:217 awaited into_response()",
            owner: CaseOwner::MethodFile {
                name: "call",
                body: "self().await.into_response()",
                file: "axum/src/handler/mod.rs",
            },
            callee: CalleeCase::Method {
                name: "into_response",
                receiver: ReceiverCase::AwaitPathResult { path: &["self"] },
            },
            count: 1,
            args: None,
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: TargetCase::Empty,
        },
    ] {
        assert_context_case(db, owner, context, &case)?;
    }

    Ok(())
}

#[tokio::test]
async fn runtime_dispatch_needs_exact_respects_axum_callable_field_summaries() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;
    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 16,
    };

    // Source oracles:
    //   axum/src/boxed.rs:120 `(self.into_route)(self.router, state)`
    //   axum/src/serve/listener.rs:236 `(self.tap_fn)(&mut io)`
    //
    // Expected traversal: exact RAG exposes the proof-authoring need while the
    // dynamic callable field is blocked, then drops that need after an
    // admitted runtime-dispatch summary. The callsite remains targetless; this
    // is not callable-field value-flow proof.
    for case in AXUM_CALLABLE_FIELD_CASES {
        let owner = method_id_by_name_and_body_substring(&db, case.method, case.body)?;
        db.project_call_proof_facts_for_owner(owner, "bd:corpus-axum-call-graph")?;
        let context = db.call_context_for_owner(owner)?;
        let row = context
            .iter()
            .find(|row| {
                row.site.kind == DbCallSiteKind::Dynamic
                    && row.status.status == DbCallStatusKind::Unsupported
                    && row.site.path.as_ref().is_some_and(|path| {
                        path.iter()
                            .map(String::as_str)
                            .eq(case.expected_path.iter().copied())
                    })
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should expose the unsupported dynamic callable field callsite: {context:#?}",
                    case.label
                )
            });
        assert!(
            row.targets.is_empty(),
            "{} should stay targetless before and after summary admission: {row:#?}",
            case.label
        );
        let field = case
            .expected_path
            .last()
            .copied()
            .expect("callable field path");

        db.upsert_proof_fact_values(&[
            ploke_test_utils::axum_callable_field_runtime_dispatch_blocker(
                row.site.id,
                field,
                case.label,
            ),
        ])?;
        let needs = rag
            .exact_runtime_dispatch_needs_for_owner(owner, options)?
            .expect("call context enabled");
        assert!(
            needs.iter().any(|need| {
                need.call_site.site_id == row.site.id
                    && need
                        .blocker_reasons
                        .iter()
                        .any(|reason| reason == "dynamic_dispatch_unbounded")
            }),
            "{} should be visible as an exact RAG runtime-dispatch need before summary admission: {needs:#?}",
            case.label
        );

        db.upsert_proof_fact_values(&[
            ploke_test_utils::axum_callable_field_runtime_dispatch_summary(
                row.site.id,
                field,
                case.label,
            ),
        ])?;
        let after = rag
            .exact_runtime_dispatch_needs_for_owner(owner, options)?
            .expect("call context enabled");
        assert!(
            after
                .iter()
                .all(|need| need.call_site.site_id != row.site.id),
            "{} admitted runtime-dispatch summary should remove the exact RAG authoring need: {after:#?}",
            case.label
        );
        assert!(
            db.call_context_for_owner(owner)?
                .into_iter()
                .find(|candidate| candidate.site.id == row.site.id)
                .is_some_and(|candidate| candidate.targets.is_empty()),
            "{} summary admission must not fabricate a local callable-field edge",
            case.label
        );
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_memchr_targetless_matrix() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_memchr_call_graph_rag()?;

    for case in MEMCHR_READ_CASES {
        assert_call_case(&db, &rag, case)?;
    }

    for source in MEMCHR_CALLABLE_TRAIT_OBJECT_CASES {
        let case = CallCase {
            label: source.label,
            owner: CaseOwner::Method {
                name: source.method,
                body: source.body,
            },
            callee: CalleeCase::Path { path: source.path },
            count: 1,
            args: Some(source.expected_arg_count),
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: TargetCase::Empty,
        };
        assert_call_case(&db, &rag, &case)?;
    }

    Ok(())
}

#[tokio::test]
async fn runtime_dispatch_needs_exact_respects_memchr_callable_trait_object_summaries()
-> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_memchr_call_graph_rag()?;
    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 16,
    };
    let owner = method_id_by_name_and_body_substring(
        &db,
        "run",
        "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
    )?;
    db.project_call_proof_facts_for_owner(owner, "bd:corpus-memchr-call-graph")?;

    // Same memchr source oracle as the call-context test above. Exact RAG
    // should expose proof-authoring needs while boxed dyn FnMut dispatch is
    // blocked, then omit them after admitted runtime-dispatch summaries.
    for case in MEMCHR_CALLABLE_TRAIT_OBJECT_CASES {
        let context = db.call_context_for_owner(owner)?;
        let row = context
            .iter()
            .find(|row| {
                row.site.kind == DbCallSiteKind::Path
                    && row.status.status == DbCallStatusKind::Unsupported
                    && row.site.path.as_ref().is_some_and(|path| {
                        path.iter()
                            .map(String::as_str)
                            .eq(case.path.iter().copied())
                    })
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should expose the unsupported boxed dyn FnMut path callsite: {context:#?}",
                    case.label
                )
            });
        assert!(
            row.targets.is_empty(),
            "{} should stay targetless before and after summary admission: {row:#?}",
            case.label
        );

        db.upsert_proof_fact_values(&[
            ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_blocker(row.site.id),
        ])?;
        let needs = rag
            .exact_runtime_dispatch_needs_for_owner(owner, options)?
            .expect("call context enabled");
        assert!(
            needs.iter().any(|need| {
                need.call_site.site_id == row.site.id
                    && need
                        .blocker_reasons
                        .iter()
                        .any(|reason| reason == "dynamic_dispatch_unbounded")
            }),
            "{} should be visible as an exact RAG runtime-dispatch need before summary admission: {needs:#?}",
            case.label
        );

        db.upsert_proof_fact_values(&[
            ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_summary(row.site.id),
        ])?;
        let after = rag
            .exact_runtime_dispatch_needs_for_owner(owner, options)?
            .expect("call context enabled");
        assert!(
            after
                .iter()
                .all(|need| need.call_site.site_id != row.site.id),
            "{} admitted runtime-dispatch summary should remove the exact RAG authoring need: {after:#?}",
            case.label
        );
        assert!(
            db.call_context_for_owner(owner)?
                .into_iter()
                .find(|candidate| candidate.site.id == row.site.id)
                .is_some_and(|candidate| candidate.targets.is_empty()),
            "{} summary admission must not fabricate a local boxed dyn FnMut edge",
            case.label
        );
    }

    Ok(())
}

fn function_ids_by_names(db: &Database, names: &[&str]) -> Result<Vec<Uuid>, Error> {
    let mut ids = names
        .iter()
        .map(|name| function_id_by_exact_name(db, name))
        .collect::<Result<Vec<_>, _>>()?;
    ids.sort_unstable();
    Ok(ids)
}

fn axum_layer_dynamic_candidate_ids(db: &Database) -> Result<Vec<Uuid>, Error> {
    let method_router_layer = method_id_by_name_and_body_substring(
        db,
        "layer",
        "let layer_fn = move |route: Route<E>| route.layer(layer.clone());",
    )?;
    let method_router_route_layer = method_id_by_name_and_body_substring(
        db,
        "route_layer",
        "let layer_fn = move |svc| Route::new(layer.layer(svc));",
    )?;
    let router_layer = method_id_by_name_body_and_file_suffix(
        db,
        "layer",
        "catch_all_fallback: this.catch_all_fallback.map(|route| route.layer(layer))",
        "axum/src/routing/mod.rs",
    )?;

    let mut ids = vec![
        closure_owner_for_method_parent(db, method_router_layer)?,
        closure_owner_for_method_parent(db, method_router_route_layer)?,
        closure_owner_for_method_parent(db, router_layer)?,
    ];
    ids.sort_unstable();
    Ok(ids)
}

fn function_id_by_exact_name(db: &Database, name: &str) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    let rows = db.raw_query_params(
        r#"?[id] :=
            *function { id, name: $name @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one function named {name:?}; rows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

fn assert_call_case(db: &Database, rag: &RagService, case: &CallCase) -> Result<(), Error> {
    for owner in case_owners(db, case)? {
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} owner {owner}: missing context", case.label));
        assert_context_case(db, owner, context, case)?;
    }

    Ok(())
}

fn assert_context_case(
    db: &Database,
    owner: Uuid,
    context: &[CallContextInfo],
    case: &CallCase,
) -> Result<(), Error> {
    let callee = case_callee(&case.callee);
    let matching = context
        .iter()
        .filter(|call| {
            call.kind == case_kind(&case.callee) && callee_shape_matches(&call.callee, &callee)
        })
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), case.count, "{}: {context:#?}", case.label);

    for call in matching {
        assert_eq!(call.owner_id, owner, "{}", case.label);
        if let CalleeCase::Dynamic { path: expected } = &case.callee {
            let actual = call
                .path
                .as_ref()
                .map(|parts| parts.iter().map(String::as_str).collect::<Vec<_>>());
            assert_eq!(actual.as_deref(), Some(*expected), "{}", case.label);
        }
        if let Some(args) = case.args {
            assert_eq!(call.arg_count, Some(args), "{}: {call:#?}", case.label);
        }
        assert_eq!(&call.status, &case.status, "{}: {call:#?}", case.label);
        assert_eq!(&call.resolution, &case.resolution, "{}", case.label);
        assert_case_targets(db, call, &case.targets, case.label)?;
    }

    Ok(())
}

fn case_owners(db: &Database, case: &CallCase) -> Result<Vec<Uuid>, Error> {
    let owners = match &case.owner {
        CaseOwner::Method { name, body } => {
            vec![method_id_by_name_and_body_substring(db, name, body)?]
        }
        CaseOwner::MethodFile { name, body, file } => {
            vec![method_id_by_file(db, name, body, file)?]
        }
        CaseOwner::Function { module, name } => {
            vec![function_id_by_name_in_module(db, module, name)?]
        }
        CaseOwner::MethodsFile {
            name,
            body,
            file,
            count,
        } => {
            let owners = method_ids_by_file(db, name, body, file)?;
            assert_eq!(owners.len(), *count, "{} generated owners", case.label);
            owners
        }
    };
    Ok(owners)
}

fn case_kind(callee: &CalleeCase) -> CallSiteKind {
    match callee {
        CalleeCase::Dynamic { .. } => CallSiteKind::Dynamic,
        CalleeCase::Path { .. } => CallSiteKind::Path,
        CalleeCase::Method { .. } => CallSiteKind::Method,
    }
}

fn case_callee(callee: &CalleeCase) -> CallCalleeInfo {
    match callee {
        CalleeCase::Dynamic { .. } => CallCalleeInfo::Dynamic,
        CalleeCase::Path { path: parts } => CallCalleeInfo::Path { path: path(parts) },
        CalleeCase::Method { name, receiver } => CallCalleeInfo::Method {
            name: (*name).to_string(),
            receiver: Some(case_receiver(receiver)),
        },
    }
}

fn case_receiver(receiver: &ReceiverCase) -> CallReceiverInfo {
    match receiver {
        ReceiverCase::AwaitPathResult { path: parts } => {
            CallReceiverInfo::AwaitPathCallResult { path: path(parts) }
        }
        ReceiverCase::PathResult { path: parts } => {
            CallReceiverInfo::PathCallResult { path: path(parts) }
        }
        ReceiverCase::MethodField { method, field } => CallReceiverInfo::MethodResultField {
            method_name: (*method).to_string(),
            method_span: (0, 0),
            field_path: path(field),
        },
        ReceiverCase::SelfField { path: parts } => {
            CallReceiverInfo::SelfField { path: path(parts) }
        }
    }
}

fn assert_case_targets(
    db: &Database,
    call: &CallContextInfo,
    expected: &TargetCase,
    label: &str,
) -> Result<(), Error> {
    match expected {
        TargetCase::Empty => assert!(
            call.targets.is_empty(),
            "{label} should remain targetless in RAG call context: {call:#?}"
        ),
        TargetCase::Relations(relations) => assert_target_relations(call, relations, label),
        TargetCase::FunctionCandidates { names, relation } => {
            let ids = function_ids_by_names(db, names)?;
            assert_candidate_targets(call, &ids, relation, label);
        }
        TargetCase::AxumLayerCandidates { relation } => {
            let ids = axum_layer_dynamic_candidate_ids(db)?;
            assert_candidate_targets(call, &ids, relation, label);
        }
    }
    Ok(())
}

fn assert_target_relations(
    call: &CallContextInfo,
    expected: &[(CallTargetKind, usize)],
    label: &str,
) {
    let total = expected.iter().map(|(_, count)| count).sum::<usize>();
    assert_eq!(call.targets.len(), total, "{label}: {call:#?}");
    for (relation, count) in expected {
        let actual = call
            .targets
            .iter()
            .filter(|target| &target.relation == relation)
            .count();
        assert_eq!(
            actual, *count,
            "{label} should expose exactly {count} {relation:?} target(s): {call:#?}"
        );
    }
}

fn assert_candidate_targets(
    call: &CallContextInfo,
    expected: &[Uuid],
    relation: &CallTargetKind,
    label: &str,
) {
    assert_eq!(call.targets.len(), expected.len(), "{label}: {call:#?}");
    assert!(
        call.targets
            .iter()
            .all(|target| &target.relation == relation),
        "{label} should expose only {relation:?} candidates: {call:#?}"
    );

    let mut actual = call
        .targets
        .iter()
        .map(|target| target.target_id)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, expected, "{label} candidate targets");
}
