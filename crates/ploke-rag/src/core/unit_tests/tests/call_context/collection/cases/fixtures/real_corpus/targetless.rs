use super::*;

struct DynamicCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
}

struct MemchrDynamicCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
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
        },
        DynamicCase {
            label: "axum/src/boxed.rs:120 MakeErasedRouter::into_route callable field",
            method: "into_route",
            body: "(self.into_route)(self.router, state)",
        },
        DynamicCase {
            label: "axum/src/boxed.rs:159 Map::into_route layer trait object",
            method: "into_route",
            body: "(self.layer)(self.inner.into_route(state))",
        },
        DynamicCase {
            label: "axum/src/serve/listener.rs:236 TapIo::accept callable field",
            method: "accept",
            body: "(self.tap_fn)(&mut io)",
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
            status: CallStatusKind::Unsupported,
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
            status: CallStatusKind::Unsupported,
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
        // method callsites, but have zero traversable targets until external
        // tower receiver dispatch and tuple-field receiver proof are modeled.
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
async fn call_context_collection_reads_axum_request_parts_local_receiver_gap() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let case = MethodCase {
        label: "axum-core/src/ext_traits/request_parts.rs:186 parts.extract_with_state",
        method: "from_request_parts",
        body: "parts.extract_with_state(state)",
        callee: CallCalleeInfo::Method {
            name: "extract_with_state".to_string(),
            receiver: Some(CallReceiverInfo::LocalBinding {
                name: "parts".to_string(),
            }),
        },
        status: CallStatusKind::Unresolved,
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
        "{} should expose one targetless local receiver row: {context:#?}",
        case.label
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum-core/src/ext_traits/request_parts.rs:186 calls
    //   `parts.extract_with_state(state)`.
    // Expected traversal: the local-binding receiver is structurally visible,
    // but has zero traversable targets until local receiver type proof can
    // connect `parts: &mut Parts` to RequestPartsExt::extract_with_state.
    // The source-oracle turbofish row at :164 remains absent in this fixture.
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
