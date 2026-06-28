use super::super::super::super::*;
use super::super::super::helpers::assert_blocked_resolution;
use super::helpers::{
    AXUM_DOMAIN, assert_site_blocker, assert_site_resolution_blocker, await_result_unwrap_site,
    axum_db, conn_limiter_accept_owner, dynamic_site, method_id_by_name_and_body,
    targetless_method_site, targetless_method_site_with_status,
};

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
    status: CallStatusKind,
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_await_result_receiver_blocker() -> Result<(), Error>
{
    init_tracing_once();
    let db = axum_db()?;

    let owner = conn_limiter_accept_owner(&db)?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "ConnLimiter::accept should project targetless call-site proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum call graph facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("ConnLimiter::accept should receive outgoing call context");
    let site_id = await_result_unwrap_site(calls, owner);

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .expect("ConnLimiter::accept should receive projected proof rows");

    // Matrix: awaited-result receiver row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/serve/listener.rs:142 owns `ConnLimiter<T>::accept`.
    //   axum/src/serve/listener.rs:143 calls
    //   `self.sem.clone().acquire_owned().await.unwrap()`.
    // Expected proof traversal: owner-seeded proof context must include the
    // call_site plus blocked call_resolution facts for the exact unsupported,
    // targetless AwaitResult `unwrap` site. There are zero callee edges for
    // this row until awaited-result receiver resolution is implemented.
    assert_blocked_resolution(rows, owner, "type_resolution_missing");
    assert_site_blocker(
        rows,
        owner,
        site_id,
        "type_resolution_missing",
        "ConnLimiter::accept AwaitResult unwrap",
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_route_oneshot_blockers() -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let cases = [
        MethodCase {
            label: "Route::oneshot_inner method-call-result receiver",
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
            label: "Route::oneshot_inner_owned tuple-field receiver",
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

    let mut owners = Vec::new();
    for case in cases {
        let owner = method_id_by_name_and_body(&db, case.method, case.body)?;
        let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
        assert!(
            projected >= 2,
            "{} should project targetless Route::oneshot proof rows",
            case.label
        );
        owners.push((case, owner));
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum Route::oneshot facts should enable RAG proof context"
    );

    for (case, owner) in owners {
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let calls = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let site_id = targetless_method_site(calls, owner, &case.callee, case.label);

        let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
        let rows = proof_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

        // Matrix: Route::oneshot receiver rows.
        // Source chain:
        //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
        //   axum/src/routing/route.rs:51 calls
        //   `self.0.clone().oneshot(req)`.
        //   axum/src/routing/route.rs:57 calls `self.0.oneshot(req)`.
        // Expected proof traversal: owner-seeded proof context must include the
        // call_site plus blocked call_resolution facts for both unsupported,
        // targetless Route::oneshot receiver shapes. There are zero callee
        // edges until external tower receiver dispatch and tuple-field receiver
        // proof are modeled.
        assert_blocked_resolution(rows, owner, "type_resolution_missing");
        assert_site_blocker(rows, owner, site_id, "type_resolution_missing", case.label);
    }

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_size_hint_self_field_blocker() -> Result<(), Error>
{
    init_tracing_once();
    let db = axum_db()?;

    let case = MethodCase {
        label: "Body::size_hint self-field receiver",
        method: "size_hint",
        body: "self.0.size_hint()",
        callee: CallCalleeInfo::Method {
            name: "size_hint".to_string(),
            receiver: Some(CallReceiverInfo::SelfField {
                path: vec!["0".to_string()],
            }),
        },
        status: CallStatusKind::Unsupported,
    };

    let owner = method_id_by_name_and_body(&db, case.method, case.body)?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "{} should project targetless self-field receiver proof rows",
        case.label
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum Body::size_hint facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
    let site_id = targetless_method_site(calls, owner, &case.callee, case.label);

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

    // Matrix: self-field size_hint receiver row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/body.rs:127 calls `self.0.size_hint()`.
    // Expected proof traversal: owner-seeded proof context must include the
    // call_site plus blocked call_resolution facts for the unsupported,
    // targetless tuple-field receiver row. There are zero callee edges until
    // tuple-field receiver proof and external http_body dispatch are modeled.
    assert_blocked_resolution(rows, owner, "type_resolution_missing");
    assert_site_blocker(rows, owner, site_id, "type_resolution_missing", case.label);

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_request_parts_local_receiver_blocker()
-> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let case = MethodCase {
        label: "RequestPartsExt custom extractor local receiver",
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

    let owner = method_id_by_name_and_body(&db, case.method, case.body)?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "{} should project targetless local receiver proof rows",
        case.label
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum request-parts local receiver facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
    let site_id =
        targetless_method_site_with_status(calls, owner, &case.callee, case.status, case.label);

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

    // Matrix: local-binding request-parts receiver row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/ext_traits/request_parts.rs:186 calls
    //   `parts.extract_with_state(state)`.
    // Expected proof traversal: owner-seeded proof context must include the
    // call_site plus unresolved call_resolution fact for this exact targetless
    // local-binding receiver row. There are zero callee edges until local
    // receiver type proof connects `parts` to RequestPartsExt.
    assert_site_resolution_blocker(
        rows,
        owner,
        site_id,
        "unresolved",
        "type_resolution_missing",
        case.label,
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_dynamic_callable_blockers() -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let cases = [
        DynamicCase {
            label: "MakeErasedHandler::into_route callable field",
            method: "into_route",
            body: "(self.into_route)(self.handler, state)",
        },
        DynamicCase {
            label: "MakeErasedRouter::into_route callable field",
            method: "into_route",
            body: "(self.into_route)(self.router, state)",
        },
        DynamicCase {
            label: "Map::into_route layer trait object",
            method: "into_route",
            body: "(self.layer)(self.inner.into_route(state))",
        },
        DynamicCase {
            label: "TapIo::accept callable field",
            method: "accept",
            body: "(self.tap_fn)(&mut io)",
        },
    ];

    let mut owners = Vec::new();
    for case in cases {
        let owner = method_id_by_name_and_body(&db, case.method, case.body)?;
        let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
        assert!(
            projected >= 2,
            "{} should project targetless dynamic call-site proof rows",
            case.label
        );
        owners.push((case, owner));
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum dynamic call graph facts should enable RAG proof context"
    );

    for (case, owner) in owners {
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let calls = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let site_id = dynamic_site(calls, owner, case.label);

        let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
        let rows = proof_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

        // Matrix: dynamic unsupported callable rows.
        // Source chain:
        //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
        //   axum/src/boxed.rs:85 calls `(self.into_route)(self.handler, state)`.
        //   axum/src/boxed.rs:120 calls `(self.into_route)(self.router, state)`.
        //   axum/src/boxed.rs:159 calls `(self.layer)(self.inner.into_route(state))`.
        //   axum/src/serve/listener.rs:236 calls `(self.tap_fn)(&mut io)`.
        // Expected proof traversal: owner-seeded proof context must include the
        // call_site plus blocked call_resolution facts for each unsupported,
        // targetless dynamic call site. There are zero callee edges until
        // callable-field, closure, and callable trait-object proof is modeled.
        assert_blocked_resolution(rows, owner, "dynamic_dispatch_unbounded");
        assert_site_blocker(
            rows,
            owner,
            site_id,
            "dynamic_dispatch_unbounded",
            case.label,
        );
    }

    Ok(())
}
