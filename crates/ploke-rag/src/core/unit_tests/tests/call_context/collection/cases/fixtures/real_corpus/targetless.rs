use super::*;

struct DynamicCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
}

struct MethodCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    callee: CallCalleeInfo,
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
