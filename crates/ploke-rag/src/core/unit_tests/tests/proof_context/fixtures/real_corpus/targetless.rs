use super::super::super::super::*;
use super::super::super::helpers::assert_blocked_resolution;
use super::helpers::{
    AXUM_DOMAIN, assert_site_blocker, assert_site_resolution_blocker, await_result_unwrap_site,
    axum_db, conn_limiter_accept_owner, dynamic_site, function_id, method_id_by_file,
    method_id_by_name_and_body, targetless_method_site, targetless_method_site_with_status,
    targetless_path_site,
};
use ploke_db::ProofGraphStore;
use ploke_test_utils::{
    AXUM_STD_MEM_REPLACE_SUMMARY_ID, axum_callback_parameter_blocker,
    axum_std_mem_replace_summary_records,
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
            status: CallStatusKind::External,
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
            status: CallStatusKind::External,
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
        let site_id =
            targetless_method_site_with_status(calls, owner, &case.callee, case.status, case.label);

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
        // call_site plus blocked call_resolution facts for both external,
        // targetless Route::oneshot receiver shapes. There are zero callee
        // edges until external tower receiver dispatch is modeled.
        assert_blocked_resolution(rows, owner, "external_dependency_summary_missing");
        assert_site_blocker(
            rows,
            owner,
            site_id,
            "external_dependency_summary_missing",
            case.label,
        );
    }

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_future_poll_trait_object_blocker()
-> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let case = MethodCase {
        label: "HandleErrorFuture::poll dyn Future dispatch",
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
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "{} should project targetless dyn Future poll proof rows",
        case.label
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum dyn Future poll facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
    let site_id =
        targetless_method_site_with_status(calls, owner, &case.callee, case.status, case.label);
    db.upsert_proof_fact_values(&[ploke_test_utils::axum_dyn_future_poll_blocker(site_id)])?;

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

    // Matrix: dyn Future poll dispatch row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/error_handling/mod.rs:238 defines `HandleErrorFuture`.
    //   axum/src/error_handling/mod.rs:251 calls
    //   `self.project().future.poll(cx)`.
    // Expected proof traversal: owner-seeded proof context must include the
    // call_site plus blocked call_resolution fact for the unsupported,
    // targetless dyn Future dispatch row. There are zero callee edges until
    // async poll/resume and runtime trait-object dispatch proof are modeled.
    assert_site_blocker(rows, owner, site_id, "type_resolution_missing", case.label);
    let site = site_id.to_string();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "proof_blocker"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
                && proof.status.as_deref() == Some("blocked")
        }),
        "{} should preserve the explicit runtime-dispatch proof blocker: {rows:#?}",
        case.label
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_size_hint_self_field_frontier() -> Result<(), Error>
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
        status: CallStatusKind::External,
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
    let site_id =
        targetless_method_site_with_status(calls, owner, &case.callee, case.status, case.label);

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

    // Matrix: self-field size_hint receiver row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/body.rs:127 calls `self.0.size_hint()`.
    // Expected proof traversal: owner-seeded proof context must include the
    // call_site plus blocked call_resolution facts for the targetless external
    // frontier row. There are zero local callee edges for external
    // http-body-util dispatch.
    assert_blocked_resolution(rows, owner, "external_dependency_summary_missing");
    assert_site_blocker(
        rows,
        owner,
        site_id,
        "external_dependency_summary_missing",
        case.label,
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_request_builder_alias_frontier()
-> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let owner = function_id(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "from_fn::tests::basic should project Request::builder alias-frontier proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum Request::builder facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("from_fn::tests::basic should receive outgoing call context");
    let site_id = targetless_path_site(
        calls,
        owner,
        &["Request", "builder"],
        CallStatusKind::External,
        "from_fn::tests::basic Request::builder",
    );

    let initial_rows = rag.exact_proof_context(owner)?;

    // Matrix: associated path through workspace type alias to external root.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/middleware/from_fn.rs:411 calls
    //   `Request::builder().uri("/").body(Body::empty()).unwrap()`.
    // Expected proof traversal: owner-seeded proof context must include the
    // call_site plus blocked call_resolution facts for the targetless
    // external frontier row. After admission, the linked external summary
    // discharges the blocker without creating a local callee edge because
    // `Request = http::Request` leaves the local workspace.
    assert_site_blocker(
        &initial_rows,
        owner,
        site_id,
        "external_dependency_summary_missing",
        "from_fn::tests::basic Request::builder",
    );
    db.upsert_proof_fact_values(&ploke_test_utils::axum_request_builder_summary_records(
        site_id,
    ))?;

    let rows = rag.exact_proof_context(owner)?;
    let site = site_id.to_string();
    let summary_id = ploke_test_utils::AXUM_REQUEST_BUILDER_SUMMARY_ID;
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("externally_summarized")
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.blocker_reason.is_none()
        }),
        "RAG proof context should expose the discharged Request::builder call_resolution: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.summary_class.as_deref() == Some("audited_no_process_effects")
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == ["external_summary_boundary".to_string()]
        }),
        "RAG proof context should expose the admitted Request::builder summary artifact: {rows:#?}"
    );
    assert!(
        rows.iter().all(|proof| {
            proof.call_site_id.as_deref() != Some(site.as_str())
                || proof.blocker_reason.as_deref() != Some("external_dependency_summary_missing")
        }),
        "RAG proof context should not retain the missing-summary blocker after Request::builder admission: {rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_std_mem_replace_admitted_summary()
-> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let owner = method_id_by_name_and_body(
        &db,
        "write_buf",
        "std::mem::replace(&mut self.data_written, true)",
    )?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "EventDataWriter::write_buf should project std::mem::replace frontier proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum std::mem::replace facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("EventDataWriter::write_buf should receive outgoing call context");
    let site_id = targetless_path_site(
        calls,
        owner,
        &["std", "mem", "replace"],
        CallStatusKind::External,
        "EventDataWriter::write_buf std::mem::replace",
    );

    db.upsert_proof_fact_values(&axum_std_mem_replace_summary_records(site_id))?;

    let rows = rag.exact_proof_context(owner)?;

    // Matrix: proof-authoritative external summary over a real external
    // frontier.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/response/sse.rs:449 calls
    //   `std::mem::replace(&mut self.data_written, true)`.
    // Expected proof traversal: the projected external frontier starts
    // fail-closed, then the linked admitted summary is model-visible through
    // RAG proof context without turning the frontier into a local edge.
    let site = site_id.to_string();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("externally_summarized")
                && proof.external_summary_id.as_deref() == Some(AXUM_STD_MEM_REPLACE_SUMMARY_ID)
                && proof.blocker_reason.is_none()
        }),
        "RAG proof context should expose the discharged std::mem::replace call_resolution: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(AXUM_STD_MEM_REPLACE_SUMMARY_ID)
                && proof.summary_class.as_deref() == Some("audited_no_process_effects")
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == ["external_summary_boundary".to_string()]
        }),
        "RAG proof context should expose the admitted std::mem::replace summary artifact: {rows:#?}"
    );
    assert!(
        rows.iter().all(|proof| {
            proof.call_site_id.as_deref() != Some(site.as_str())
                || proof.blocker_reason.as_deref() != Some("external_dependency_summary_missing")
        }),
        "RAG proof context should not retain the missing-summary blocker after admission: {rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_generated_constructor_frontier()
-> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let owner = method_id_by_name_and_body(&db, "call", "IntoServiceFuture::new(future)")?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "HandlerService::call should project IntoServiceFuture::new frontier proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum IntoServiceFuture::new facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("HandlerService::call should receive outgoing call context");
    let site_id = targetless_path_site(
        calls,
        owner,
        &["super", "future", "IntoServiceFuture", "new"],
        CallStatusKind::Unresolved,
        "HandlerService::call IntoServiceFuture::new",
    );
    db.upsert_proof_fact_values(&ploke_test_utils::axum_opaque_future_macro_summary_records(
        site_id,
    ))?;

    let rows = rag.exact_proof_context(owner)?;

    // Matrix: macro-generated inherent constructor row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/handler/service.rs:155 binds
    //   `type Future = super::future::IntoServiceFuture<H::Future>`.
    //   axum/src/handler/service.rs:174 calls
    //   `super::future::IntoServiceFuture::new(future)`.
    //   axum/src/handler/future.rs:11-18 and axum/src/macros.rs:19-20
    //   generate the concrete inherent `new`.
    // Expected proof traversal: proof context must include the call_site plus
    // unresolved call_resolution fact for this exact targetless path row, plus
    // the callsite-linked admitted macro-boundary summary. There are zero local
    // callee edges until macro-generated inherent items are modeled as source
    // items.
    assert_site_resolution_blocker(
        &rows,
        owner,
        site_id,
        "unresolved",
        "type_resolution_missing",
        "HandlerService::call IntoServiceFuture::new",
    );
    let site = site_id.to_string();
    let boundary_id = ploke_test_utils::axum_opaque_future_boundary_id(site_id);
    let summary_id = ploke_test_utils::AXUM_OPAQUE_FUTURE_SUMMARY_ID;
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "expansion_boundary"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.boundary_id.as_deref() == Some(boundary_id.as_str())
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.status.as_deref() == Some("externally_summarized")
                && proof.blocker_reason.is_none()
        }),
        "RAG proof context should expose the admitted opaque_future boundary summary linked to the generated constructor callsite: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == ["external_summary_boundary".to_string()]
        }),
        "RAG proof context should expose the admitted opaque_future summary artifact: {rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_generated_post_frontier() -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let owner = function_id(&db, &["crate", "json", "tests"], "deserialize_body")?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "deserialize_body should project generated routing::post proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum routing::post facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("deserialize_body should receive outgoing call context");
    let site_id = targetless_path_site(
        calls,
        owner,
        &["post"],
        CallStatusKind::Unsupported,
        "deserialize_body generated routing::post",
    );
    db.upsert_proof_fact_values(&ploke_test_utils::axum_routing_post_macro_summary_records(
        site_id,
    ))?;

    let rows = rag.exact_proof_context(owner)?;

    // Matrix: macro-generated routing helper row.
    // Source chain:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-case-matrix.md
    //   axum/src/routing/method_routing.rs:165 is the `post` template.
    //   axum/src/routing/method_routing.rs:445 invokes the macro for `post`.
    //   axum/src/json.rs:237 imports `routing::post`.
    //   axum/src/json.rs:248 calls `post(echo_json)`.
    // Expected proof traversal: proof context must include the call_site plus
    // blocked call_resolution fact for this exact targetless path row, plus
    // the callsite-linked admitted macro-boundary summary. There are zero local
    // callee edges until generated routing functions are modeled as source
    // items.
    assert_site_resolution_blocker(
        &rows,
        owner,
        site_id,
        "blocked",
        "type_resolution_missing",
        "deserialize_body generated routing::post",
    );
    let site = site_id.to_string();
    let boundary_id = ploke_test_utils::axum_routing_post_boundary_id(site_id);
    let summary_id = ploke_test_utils::AXUM_ROUTING_POST_SUMMARY_ID;
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "expansion_boundary"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.boundary_id.as_deref() == Some(boundary_id.as_str())
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.status.as_deref() == Some("externally_summarized")
                && proof.blocker_reason.is_none()
        }),
        "RAG proof context should expose the admitted routing::post boundary summary linked to the generated function callsite: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == ["external_summary_boundary".to_string()]
        }),
        "RAG proof context should expose the admitted routing::post summary artifact: {rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_handler_async_block_owner_blockers()
-> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let parent = method_id_by_file(
        &db,
        "call",
        "self().await.into_response()",
        "axum/src/handler/mod.rs",
    )?;
    let owner = async_block_owner_for_method_parent(&db, parent)?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 4,
        "Handler::call async block should project targetless self()/into_response() proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum handler async-block facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("Handler::call async block should receive outgoing call context");
    let self_site = targetless_path_site(
        calls,
        owner,
        &["self"],
        CallStatusKind::Unsupported,
        "Handler::call async-block self()",
    );
    let into_response_callee = CallCalleeInfo::Method {
        name: "into_response".to_string(),
        receiver: Some(CallReceiverInfo::AwaitPathCallResult {
            path: vec!["self".to_string()],
        }),
    };
    let into_response_site = targetless_method_site(
        calls,
        owner,
        &into_response_callee,
        "Handler::call async-block into_response()",
    );

    db.upsert_proof_fact_values(&[
        ploke_test_utils::axum_handler_async_block_poll_resume_blocker(self_site, "self"),
        ploke_test_utils::axum_handler_async_block_poll_resume_blocker(
            into_response_site,
            "into_response",
        ),
    ])?;

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .expect("Handler::call async block should receive projected proof rows");

    // Matrix: async block body boundary.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/handler/mod.rs:217 calls
    //   `Box::pin(async move { self().await.into_response() })`.
    // Expected proof traversal: owner-seeded proof context must include the
    // call_site plus blocked call_resolution facts for the nested async-block
    // `self()` and awaited `into_response()` rows. There are zero callee edges
    // until callable binding and awaited receiver proof can resolve these
    // targetless sites.
    assert_site_blocker(
        rows,
        owner,
        self_site,
        "type_resolution_missing",
        "Handler::call async-block self()",
    );
    assert_explicit_proof_blocker(
        rows,
        self_site,
        "dynamic_dispatch_unbounded",
        "Handler::call async-block self() async poll/resume",
    );
    assert_site_blocker(
        rows,
        owner,
        into_response_site,
        "type_resolution_missing",
        "Handler::call async-block into_response()",
    );
    assert_explicit_proof_blocker(
        rows,
        into_response_site,
        "dynamic_dispatch_unbounded",
        "Handler::call async-block into_response() async poll/resume",
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_callback_parameter_blocker() -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let parent = function_id(&db, &["crate"], "expand_attr_with")?;
    let parent_rag = init_test_rag_mock(Arc::clone(&db));
    let parent_context = parent_rag.collect_call_context(&[(parent, 1.0)])?;
    let parent_calls = parent_context
        .get(&parent)
        .expect("expand_attr_with should receive outgoing call context");
    let iife = parent_calls
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.relation == CallTargetKind::DynamicClosure)
        })
        .expect("expand_attr_with should expose the resolved IIFE closure target");
    let owner = iife.targets[0].target_id;

    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 2,
        "expand_attr_with IIFE closure owner should project targetless callback proof rows"
    );
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected callback proof rows should enable RAG proof context"
    );
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("expand_attr_with IIFE closure owner should receive outgoing call context");
    let site_id = targetless_path_site(
        calls,
        owner,
        &["f"],
        CallStatusKind::Unsupported,
        "axum-macros/src/lib.rs:737 f(attr, input)",
    );
    db.upsert_proof_fact_values(&[axum_callback_parameter_blocker(site_id)])?;

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .expect("expand_attr_with IIFE closure owner should receive projected proof rows");

    // Matrix: captured callable parameter row.
    // Source chain:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-macros/src/lib.rs:727 defines `f: F`.
    //   axum-macros/src/lib.rs:729 bounds `F: FnOnce(A, I) -> K`.
    //   axum-macros/src/lib.rs:734-738 immediately invokes a closure.
    //   axum-macros/src/lib.rs:737 calls `f(attr, input)` inside that closure.
    // Expected proof traversal: proof context exposes the fail-closed
    // call_resolution row plus an explicit runtime-dispatch blocker, while
    // preserving zero local traversal edges for the captured callback.
    assert_site_blocker(
        rows,
        owner,
        site_id,
        "type_resolution_missing",
        "axum callback parameter f(attr, input)",
    );
    assert_explicit_proof_blocker(
        rows,
        site_id,
        "dynamic_dispatch_unbounded",
        "axum callback parameter f(attr, input)",
    );

    Ok(())
}

fn assert_explicit_proof_blocker(
    rows: &[ProofContextInfo],
    site_id: Uuid,
    reason: &str,
    label: &str,
) {
    let site_id = site_id.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "proof_blocker"
                && row.call_site_id.as_deref() == Some(site_id.as_str())
                && row.blocker_reason.as_deref() == Some(reason)
                && row.status.as_deref() == Some("blocked")
        }),
        "{label} proof context should include the explicit proof_blocker fact: {rows:#?}"
    );
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
