use super::*;

struct DynamicCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    expected_path: &'static [&'static str],
}

struct MemchrDynamicCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    expected_arg_count: u32,
}

struct MemchrPathCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    path: &'static [&'static str],
    expected_arg_count: u32,
}

struct MethodCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    callee: CallCalleeInfo,
    status: CallStatusKind,
}

#[tokio::test]
async fn call_context_collection_reads_axum_dynamic_callable_field_gaps() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chains:
    //   axum/src/boxed.rs:85  `(self.into_route)(self.handler, state)`
    //   axum/src/boxed.rs:120 `(self.into_route)(self.router, state)`
    //   axum/src/boxed.rs:159 `(self.layer)(self.inner.into_route(state))`
    //   axum/src/serve/listener.rs:236 `(self.tap_fn)(&mut io)`
    //
    // Expected traversal: these are visible structural dynamic callsites, but
    // they have zero traversable edges until callable-field/closure/dyn-trait
    // proof is modeled. DB tests own argument counts and source-line fanout;
    // RAG must preserve the targetless unsupported rows without guessing.
    let cases = [
        DynamicCase {
            label: "axum/src/boxed.rs:85 MakeErasedHandler::into_route callable field",
            method: "into_route",
            body: "(self.into_route)(self.handler, state)",
            expected_path: &["self", "into_route"],
        },
        DynamicCase {
            label: "axum/src/boxed.rs:120 MakeErasedRouter::into_route callable field",
            method: "into_route",
            body: "(self.into_route)(self.router, state)",
            expected_path: &["self", "into_route"],
        },
        DynamicCase {
            label: "axum/src/boxed.rs:159 Map::into_route layer trait object",
            method: "into_route",
            body: "(self.layer)(self.inner.into_route(state))",
            expected_path: &["self", "layer"],
        },
        DynamicCase {
            label: "axum/src/serve/listener.rs:236 TapIo::accept callable field",
            method: "accept",
            body: "(self.tap_fn)(&mut io)",
            expected_path: &["self", "tap_fn"],
        },
    ];

    for case in cases {
        let owner = method_id_by_name_and_body_substring(&db, case.method, case.body)?;
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let dynamic = context
            .iter()
            .filter(|call| {
                call.kind == CallSiteKind::Dynamic && call.callee == CallCalleeInfo::Dynamic
            })
            .collect::<Vec<_>>();
        assert_eq!(
            dynamic.len(),
            1,
            "{} should expose one dynamic callable field row: {context:#?}",
            case.label
        );

        let call = dynamic[0];
        assert_eq!(call.owner_id, owner);
        assert!(
            call.path.as_ref().is_some_and(|path| path
                .iter()
                .map(String::as_str)
                .eq(case.expected_path.iter().copied())),
            "{} should preserve the dynamic self-field callee path: {call:#?}",
            case.label
        );
        assert_eq!(call.status, CallStatusKind::Unsupported);
        assert_eq!(call.resolution, None);
        assert!(
            call.targets.is_empty(),
            "{} should remain targetless in RAG call context: {call:#?}",
            case.label
        );
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_memchr_function_pointer_field_gaps() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_memchr_call_graph_rag()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chains:
    //   memchr/src/memmem/searcher.rs:33-35 defines `Searcher.call`.
    //   memchr/src/memmem/searcher.rs:222 calls
    //   `(self.call)(self, prestate, haystack, needle)`.
    //   memchr/src/memmem/searcher.rs:604-605 defines `Prefilter.call`.
    //   memchr/src/memmem/searcher.rs:718 calls `(self.call)(self, haystack)`.
    //
    // Expected traversal: these function-pointer fields are structurally
    // visible dynamic callsites, but have zero traversable edges until field
    // binding and function-pointer dispatch proof is modeled. DB tests own
    // source-line fanout; RAG must preserve the targetless unsupported rows
    // and argument counts without guessing.
    let cases = [
        MemchrDynamicCase {
            label: "memchr/src/memmem/searcher.rs:222 Searcher.call",
            method: "find",
            body: "(self.call)(self, prestate, haystack, needle)",
            expected_arg_count: 4,
        },
        MemchrDynamicCase {
            label: "memchr/src/memmem/searcher.rs:718 Prefilter.call",
            method: "find",
            body: "(self.call)(self, haystack)",
            expected_arg_count: 2,
        },
    ];

    for case in cases {
        let owner = method_id_by_name_and_body_substring(&db, case.method, case.body)?;
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let dynamic = context
            .iter()
            .filter(|call| {
                call.kind == CallSiteKind::Dynamic && call.callee == CallCalleeInfo::Dynamic
            })
            .collect::<Vec<_>>();
        assert_eq!(
            dynamic.len(),
            1,
            "{} should expose one function-pointer field row: {context:#?}",
            case.label
        );

        let call = dynamic[0];
        assert_eq!(call.owner_id, owner);
        assert_eq!(call.arg_count, Some(case.expected_arg_count));
        assert_eq!(call.status, CallStatusKind::Unsupported);
        assert_eq!(call.resolution, None);
        assert!(
            call.targets.is_empty(),
            "{} should remain targetless in RAG call context: {call:#?}",
            case.label
        );
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_memchr_callable_trait_object_path_gaps() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_memchr_call_graph_rag()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chains:
    //   memchr/src/tests/substring/mod.rs:73-77 defines `Runner` with
    //   `fwd` and `rev` boxed `dyn FnMut` fields.
    //   memchr/src/tests/substring/mod.rs:94 calls
    //   `fwd(t.haystack.as_bytes(), t.needle.as_bytes())`.
    //   memchr/src/tests/substring/mod.rs:110 calls
    //   `rev(t.haystack.as_bytes(), t.needle.as_bytes())`.
    //
    // Expected traversal: these callable trait-object fields are structurally
    // visible path callsites in `Runner::run`, but have zero traversable edges
    // until local binding and callable trait-object dispatch proof is modeled.
    // RAG must preserve the targetless unsupported rows without guessing.
    let cases = [
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

    for case in cases {
        let owner = method_id_by_name_and_body_substring(&db, case.method, case.body)?;
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let callee = CallCalleeInfo::Path {
            path: path(case.path),
        };
        let matching = context
            .iter()
            .filter(|call| call.kind == CallSiteKind::Path && call.callee == callee)
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "{} should expose one callable trait-object path row: {context:#?}",
            case.label
        );

        let call = matching[0];
        assert_eq!(call.owner_id, owner);
        assert_eq!(call.arg_count, Some(case.expected_arg_count));
        assert_eq!(call.status, CallStatusKind::Unsupported);
        assert_eq!(call.resolution, None);
        assert!(
            call.targets.is_empty(),
            "{} should remain targetless in RAG call context: {call:#?}",
            case.label
        );
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_expand_with_method_callback_candidates()
-> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("expand_with should receive outgoing RAG call context");
    let and_then = context
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "and_then".to_string(),
                        receiver: Some(CallReceiverInfo::PathCallResult {
                            path: path(&["syn", "parse"]),
                        }),
                    }
        })
        .collect::<Vec<_>>();

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum-macros/src/lib.rs:715 passes `from_ref::expand` to
    //   `expand_with(item, from_ref::expand)`.
    //   axum-macros/src/lib.rs:377,426,665 pass closures to `expand_with`.
    //   axum-macros/src/lib.rs:724 calls
    //   `expand(syn::parse(input).and_then(f))` inside `expand_with`.
    // Expected traversal: the `and_then(f)` method call exposes the finite
    // caller-derived callback candidate set for RAG payloads, but remains
    // ambiguous and does not become a resolved traversal edge.
    assert_eq!(
        and_then.len(),
        1,
        "expand_with should expose one syn::parse(...).and_then(f) call row: {context:#?}"
    );
    assert_eq!(and_then[0].arg_count, Some(1));
    assert_eq!(and_then[0].status, CallStatusKind::Ambiguous);
    assert_eq!(and_then[0].resolution, None);
    assert_eq!(
        and_then[0].targets.len(),
        4,
        "and_then(f) should expose four finite method-callback candidates: {and_then:#?}"
    );
    assert!(
        and_then
            .iter()
            .flat_map(|call| call.targets.iter())
            .any(|target| target.relation == CallTargetKind::MethodCallbackFunction),
        "and_then(f) should include the from_ref::expand function candidate: {and_then:#?}"
    );
    assert_eq!(
        and_then
            .iter()
            .flat_map(|call| call.targets.iter())
            .filter(|target| target.relation == CallTargetKind::MethodCallbackClosure)
            .count(),
        3,
        "and_then(f) should include the three closure candidates from expand_with callers: {and_then:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_captured_callback_parameter_gap() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let parent = function_id_by_name_in_module(&db, &["crate"], "expand_attr_with")?;
    let owner = closure_owner_for_parent(&db, parent)?;
    let call_context = rag.collect_call_context(&[(parent, 1.0), (owner, 1.0)])?;

    if let Some(parent_context) = call_context.get(&parent) {
        assert!(
            parent_context
                .iter()
                .all(|call| { call.callee != CallCalleeInfo::Path { path: path(&["f"]) } }),
            "expand_attr_with must not absorb the closure-owned f(attr, input) row: {parent_context:#?}"
        );
    }

    let context = call_context
        .get(&owner)
        .expect("expand_attr_with IIFE closure owner should receive outgoing call context");
    let callback = context
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee == CallCalleeInfo::Path { path: path(&["f"]) }
        })
        .collect::<Vec<_>>();

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum-macros/src/lib.rs:727 defines `f: F`.
    //   axum-macros/src/lib.rs:729 bounds `F: FnOnce(A, I) -> K`.
    //   axum-macros/src/lib.rs:734-738 immediately invokes an IIFE closure.
    //   axum-macros/src/lib.rs:737 calls `f(attr, input)` inside that closure.
    // Expected traversal: the closure-owned path call to captured callback
    // parameter `f` is visible for RAG as a finite ambiguous closure-candidate
    // set, but no resolved traversal edge is admitted until interprocedural
    // callable argument proof exists.
    assert_eq!(
        callback.len(),
        1,
        "expand_attr_with IIFE closure should expose one captured callback parameter row: {context:#?}"
    );
    assert_eq!(callback[0].arg_count, Some(2));
    assert_eq!(callback[0].status, CallStatusKind::Ambiguous);
    assert_eq!(callback[0].resolution, None);
    assert_eq!(
        callback[0].targets.len(),
        2,
        "captured callback parameter f(attr, input) should expose both closure candidates: {callback:#?}"
    );
    assert!(
        callback[0]
            .targets
            .iter()
            .all(|target| target.relation == CallTargetKind::Closure),
        "captured callback parameter f(attr, input) should only expose closure candidates: {callback:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_generated_constructor_edge() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner =
        method_id_by_name_and_body_substring(&db, "call", "IntoServiceFuture::new(future)")?;
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("HandlerService::call should receive outgoing call context");
    let generated = context
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["super", "future", "IntoServiceFuture", "new"]),
                    }
        })
        .collect::<Vec<_>>();

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/handler/future.rs:11-18 invokes `opaque_future!` for
    //   `IntoServiceFuture`.
    //   axum/src/macros.rs:19-20 is the macro template for generated `new`.
    //   axum/src/handler/service.rs:155 binds the associated future type.
    //   axum/src/handler/service.rs:174 calls
    //   `super::future::IntoServiceFuture::new(future)`.
    // Expected traversal: the bounded `opaque_future!` item invocation is
    // modeled as a generated struct plus inherent `new` method, so RAG sees a
    // one-target associated-function edge.
    assert_eq!(
        generated.len(),
        1,
        "HandlerService::call should expose one IntoServiceFuture::new row: {context:#?}"
    );
    assert_eq!(generated[0].arg_count, Some(1));
    assert_eq!(generated[0].status, CallStatusKind::Resolved);
    assert_eq!(
        generated[0].resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(
        generated[0].targets.len(),
        1,
        "generated IntoServiceFuture::new row should expose one RAG target: {generated:#?}"
    );
    assert_eq!(
        generated[0].targets[0].relation,
        CallTargetKind::AssociatedFunction
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_route_oneshot_receiver_gaps() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let cases = [
        MethodCase {
            label: "axum/src/routing/route.rs:51 Route::oneshot_inner",
            method: "oneshot_inner",
            body: "self.0.clone().oneshot(req)",
            callee: CallCalleeInfo::Method {
                name: "oneshot".to_string(),
                receiver: Some(CallReceiverInfo::MethodCallResult {
                    method_name: "clone".to_string(),
                }),
            },
            status: CallStatusKind::External,
        },
        MethodCase {
            label: "axum/src/routing/route.rs:57 Route::oneshot_inner_owned",
            method: "oneshot_inner_owned",
            body: "self.0.oneshot(req)",
            callee: CallCalleeInfo::Method {
                name: "oneshot".to_string(),
                receiver: Some(CallReceiverInfo::SelfField {
                    path: vec!["0".to_string()],
                }),
            },
            status: CallStatusKind::External,
        },
    ];

    for case in cases {
        let owner = method_id_by_name_and_body_substring(&db, case.method, case.body)?;
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let matching = context
            .iter()
            .filter(|call| call.kind == CallSiteKind::Method && call.callee == case.callee)
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "{} should expose one targetless Route::oneshot row: {context:#?}",
            case.label
        );

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        //   axum/src/routing/route.rs:51 calls
        //   `self.0.clone().oneshot(req)`.
        //   axum/src/routing/route.rs:57 calls `self.0.oneshot(req)`.
        // Expected traversal: both receiver shapes are structurally visible
        // external method frontiers, but have zero traversable targets until
        // external tower receiver dispatch is modeled.
        let call = matching[0];
        assert_eq!(call.owner_id, owner);
        assert_eq!(call.status, case.status);
        assert_eq!(call.resolution, None);
        assert!(
            call.targets.is_empty(),
            "{} should remain targetless in RAG call context: {call:#?}",
            case.label
        );
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_future_poll_trait_object_gap() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let case = MethodCase {
        label: "axum/src/error_handling/mod.rs:251 HandleErrorFuture::poll dyn Future",
        method: "poll",
        body: "self.project().future.poll(cx)",
        callee: CallCalleeInfo::Method {
            name: "poll".to_string(),
            receiver: Some(CallReceiverInfo::Unsupported),
        },
        status: CallStatusKind::Unsupported,
    };

    let owner = method_id_by_file(
        &db,
        case.method,
        case.body,
        "axum/src/error_handling/mod.rs",
    )?;
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
    let matching = context
        .iter()
        .filter(|call| call.kind == CallSiteKind::Method && call.callee == case.callee)
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{} should expose one targetless dyn Future poll row: {context:#?}",
        case.label
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/error_handling/mod.rs:238 defines `HandleErrorFuture`.
    //   axum/src/error_handling/mod.rs:251 calls
    //   `self.project().future.poll(cx)` on
    //   `Pin<Box<dyn Future<Output = Result<Response, Infallible>>>>`.
    // Expected traversal: the dyn Future dispatch row is structurally visible,
    // but remains unsupported and targetless until async poll/resume and
    // runtime trait-object dispatch proof are modeled.
    let call = matching[0];
    assert_eq!(call.owner_id, owner);
    assert_eq!(call.status, case.status);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{} should remain targetless in RAG call context: {call:#?}",
        case.label
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_size_hint_self_field_frontier() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let case = MethodCase {
        label: "axum-core/src/body.rs:127 Body::size_hint self field",
        method: "size_hint",
        body: "self.0.size_hint()",
        callee: CallCalleeInfo::Method {
            name: "size_hint".to_string(),
            receiver: Some(CallReceiverInfo::SelfField {
                path: vec!["0".to_string()],
            }),
        },
        status: CallStatusKind::External,
    };

    let owner = method_id_by_name_and_body_substring(&db, case.method, case.body)?;
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
    let matching = context
        .iter()
        .filter(|call| call.kind == CallSiteKind::Method && call.callee == case.callee)
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{} should expose one targetless self-field receiver row: {context:#?}",
        case.label
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum-core/src/body.rs:127 calls `self.0.size_hint()`.
    // Expected traversal: tuple-field receiver proof connects
    // `Body(BoxBody)` at axum-core/src/body.rs:42 to a local type alias for an
    // external http-body-util type. The row is an external frontier with zero
    // local traversal targets.
    let call = matching[0];
    assert_eq!(call.owner_id, owner);
    assert_eq!(call.status, case.status);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{} should remain targetless as an external frontier in RAG call context: {call:#?}",
        case.label
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_request_builder_alias_frontier() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("from_fn::tests::basic should receive outgoing call context");
    let callee = CallCalleeInfo::Path {
        path: path(&["Request", "builder"]),
    };
    let matching = context
        .iter()
        .filter(|call| call.kind == CallSiteKind::Path && call.callee == callee)
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "from_fn::tests::basic should expose one Request::builder alias frontier row: {context:#?}"
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/middleware/from_fn.rs:411 calls
    //   `Request::builder().uri("/").body(Body::empty()).unwrap()`.
    // Expected traversal: `Request` resolves through axum-core's
    // `Request = http::Request` alias, so the path call is an external
    // frontier with zero local traversal targets.
    let call = matching[0];
    assert_eq!(call.owner_id, owner);
    assert_eq!(call.arg_count, Some(0));
    assert_eq!(call.status, CallStatusKind::External);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "Request::builder alias frontier should remain targetless in RAG call context: {call:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_preserves_axum_shadowed_get_boundary() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "routing", "tests"],
        "what_matches_wildcard",
    )?;
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("what_matches_wildcard should receive outgoing call context");
    let callee = CallCalleeInfo::Path {
        path: path(&["get"]),
    };
    let matching = context
        .iter()
        .filter(|call| call.kind == CallSiteKind::Path && call.callee == callee)
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        2,
        "what_matches_wildcard should expose only the two setup get(...) rows: {context:#?}"
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/routing/tests/mod.rs:412-413 calls imported routing `get`.
    //   axum/src/routing/tests/mod.rs:418 binds a local closure named `get`.
    //   axum/src/routing/tests/mod.rs:423-434 calls that local closure inside
    //   `assert_eq!` macro arguments.
    // Expected traversal: the current fixture exposes the two setup path rows,
    // keeps them targetless, and does not fabricate edges from the later
    // shadowed macro-argument calls to the imported routing helper.
    for call in matching {
        assert_eq!(call.owner_id, owner);
        assert_eq!(call.arg_count, Some(1));
        assert_eq!(call.status, CallStatusKind::Unsupported);
        assert_eq!(call.resolution, None);
        assert!(
            call.targets.is_empty(),
            "shadowed get boundary should remain targetless in RAG call context: {call:#?}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_std_mem_replace_frontier() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner = method_id_by_name_and_body_substring(
        &db,
        "write_buf",
        "std::mem::replace(&mut self.data_written, true)",
    )?;
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("EventDataWriter::write_buf should receive outgoing call context");
    let callee = CallCalleeInfo::Path {
        path: path(&["std", "mem", "replace"]),
    };
    let matching = context
        .iter()
        .filter(|call| call.kind == CallSiteKind::Path && call.callee == callee)
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "EventDataWriter::write_buf should expose one std::mem::replace frontier row: {context:#?}"
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/response/sse.rs:449 calls
    //   `std::mem::replace(&mut self.data_written, true)`.
    // Expected traversal: std-root path calls are structurally visible
    // external frontiers, but have zero traversable local targets.
    let call = matching[0];
    assert_eq!(call.owner_id, owner);
    assert_eq!(call.arg_count, Some(2));
    assert_eq!(call.status, CallStatusKind::External);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "std::mem::replace frontier should remain targetless in RAG call context: {call:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_axum_handler_async_block_owner_gap() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let parent = method_id_by_file(
        &db,
        "call",
        "self().await.into_response()",
        "axum/src/handler/mod.rs",
    )?;
    let owner = async_block_owner_for_method_parent(&db, parent)?;
    let call_context = rag.collect_call_context(&[(parent, 1.0), (owner, 1.0)])?;

    if let Some(parent_context) = call_context.get(&parent) {
        assert!(
            parent_context.iter().all(|call| {
                call.callee
                    != CallCalleeInfo::Path {
                        path: path(&["self"]),
                    }
            }),
            "Handler::call must not absorb the async-block self() row: {parent_context:#?}"
        );
        assert!(
            parent_context.iter().all(|call| {
                !matches!(
                    &call.callee,
                    CallCalleeInfo::Method { name, .. } if name == "into_response"
                )
            }),
            "Handler::call must not absorb the async-block into_response() row: {parent_context:#?}"
        );
    }

    let context = call_context
        .get(&owner)
        .expect("Handler::call async block owner should receive outgoing call context");
    let self_call = context
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["self"]),
                    }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        self_call.len(),
        1,
        "Handler::call async block should expose one targetless self() path row: {context:#?}"
    );
    assert_eq!(self_call[0].status, CallStatusKind::Unsupported);
    assert_eq!(self_call[0].resolution, None);
    assert!(
        self_call[0].targets.is_empty(),
        "Handler::call async-block self() row should remain targetless: {self_call:#?}"
    );

    let expected_receiver = Some(CallReceiverInfo::AwaitPathCallResult {
        path: path(&["self"]),
    });
    let into_response = context
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "into_response".to_string(),
                        receiver: expected_receiver.clone(),
                    }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        into_response.len(),
        1,
        "Handler::call async block should expose one targetless awaited into_response row: {context:#?}"
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/handler/mod.rs:217 calls
    //   `Box::pin(async move { self().await.into_response() })`.
    // Expected traversal: the callable `self()` and awaited
    // `into_response()` callsites are structurally visible, but they are
    // owned by the nested async-block executable owner and have zero
    // traversable targets until callable binding and awaited receiver proof
    // are modeled.
    assert_eq!(into_response[0].status, CallStatusKind::Unsupported);
    assert_eq!(into_response[0].resolution, None);
    assert!(
        into_response[0].targets.is_empty(),
        "Handler::call async-block into_response row should remain targetless: {into_response:#?}"
    );

    Ok(())
}
