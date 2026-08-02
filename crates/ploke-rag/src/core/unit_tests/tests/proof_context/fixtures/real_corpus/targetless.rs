use super::super::super::super::*;
use super::super::super::helpers::assert_blocked_resolution;
use super::helpers::{
    AXUM_DOMAIN, MEMCHR_DOMAIN, assert_site_blocker, assert_site_blocker_in_domain,
    await_result_unwrap_site, axum_db, conn_limiter_accept_owner, dynamic_site, function_id,
    memchr_db, method_id_by_file, method_id_by_name_and_body, targetless_method_site,
    targetless_method_site_with_status, targetless_path_site,
};
use ploke_db::ProofGraphStore;
use ploke_test_utils::{
    AXUM_BODY_SIZE_HINT_SUMMARY_ID, AXUM_ROUTE_ONESHOT_SUMMARY_ID, AXUM_STD_MEM_REPLACE_SUMMARY_ID,
    axum_body_size_hint_summary_records, axum_route_oneshot_summary_records,
    axum_std_mem_replace_summary_records,
};

struct DynamicCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    expected_path: &'static [&'static str],
}

struct MethodCase {
    label: &'static str,
    method: &'static str,
    body: &'static str,
    callee: CallCalleeInfo,
    status: CallStatusKind,
}

// Real-corpus source ledger. Full provenance is in
// docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md.
// Awaited unwrap: listener.rs:142-143; dyn Future poll:
// error_handling/mod.rs:238,251.
// Route::oneshot: route.rs:51,57; Body::size_hint: body.rs:127;
// Request::builder: from_fn.rs:411; std::mem::replace: sse.rs:449.
// Generated calls: handler/service.rs:174, json.rs:248, and
// routing/tests/get_to_head.rs:46.
// Ambiguous callables: axum-macros/src/lib.rs:737 and boxed.rs:159,163.
fn assert_admitted_external_summary(
    rows: &[ProofContextInfo],
    site_id: Uuid,
    summary_id: &str,
    label: &str,
) {
    let site = site_id.to_string();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("externally_summarized")
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.blocker_reason.is_none()
        }),
        "{label} should expose the discharged call_resolution: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.summary_class.as_deref() == Some("audited_no_process_effects")
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == ["external_summary_boundary".to_string()]
        }),
        "{label} should expose the admitted external summary: {rows:#?}"
    );
    assert!(
        rows.iter().all(|proof| {
            proof.call_site_id.as_deref() != Some(site.as_str())
                || proof.blocker_reason.as_deref() != Some("external_dependency_summary_missing")
        }),
        "{label} should not retain the discharged missing-summary blocker: {rows:#?}"
    );
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

    assert_blocked_resolution(rows, owner, "external_dependency_summary_missing");
    assert_site_blocker(
        rows,
        owner,
        site_id,
        "external_dependency_summary_missing",
        "ConnLimiter::accept AwaitMethodCallResult unwrap",
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_route_oneshot_blockers() -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let cases = [
        MethodCase {
            label: "Route::oneshot_inner method-result-field receiver",
            method: "oneshot_inner",
            body: "self.0.clone().oneshot(req)",
            callee: CallCalleeInfo::Method {
                name: "oneshot".to_string(),
                receiver: Some(CallReceiverInfo::MethodResultField {
                    method_name: "clone".to_string(),
                    method_span: (0, 0),
                    field_path: vec!["0".to_string()],
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

        assert_blocked_resolution(rows, owner, "external_dependency_summary_missing");
        assert_site_blocker(
            rows,
            owner,
            site_id,
            "external_dependency_summary_missing",
            case.label,
        );

        db.upsert_proof_fact_values(&axum_route_oneshot_summary_records(site_id))?;
        let admitted = rag.exact_proof_context(owner)?;
        assert_admitted_external_summary(
            &admitted,
            site_id,
            AXUM_ROUTE_ONESHOT_SUMMARY_ID,
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
            receiver: Some(CallReceiverInfo::MethodResultField {
                method_name: "project".to_string(),
                method_span: (0, 0),
                field_path: vec!["future".to_string()],
            }),
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
    let poll = calls
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.status == case.status
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Method {
                        name,
                        receiver: Some(CallReceiverInfo::MethodResultField {
                            method_name,
                            field_path,
                            ..
                        }),
                    } if name == "poll"
                        && method_name == "project"
                        && field_path == &vec!["future".to_string()]
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "{} should preserve project().future receiver evidence: {calls:#?}",
                case.label
            )
        });
    let site_id = poll.site_id;
    db.upsert_proof_fact_values(&[ploke_test_utils::axum_dyn_future_poll_blocker(site_id)])?;

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

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

    assert_blocked_resolution(rows, owner, "external_dependency_summary_missing");
    assert_site_blocker(
        rows,
        owner,
        site_id,
        "external_dependency_summary_missing",
        case.label,
    );

    db.upsert_proof_fact_values(&axum_body_size_hint_summary_records(site_id))?;
    let admitted = rag.exact_proof_context(owner)?;
    assert_admitted_external_summary(
        &admitted,
        site_id,
        AXUM_BODY_SIZE_HINT_SUMMARY_ID,
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
    assert_admitted_external_summary(
        &rows,
        site_id,
        ploke_test_utils::AXUM_REQUEST_BUILDER_SUMMARY_ID,
        "from_fn::tests::basic Request::builder",
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

    assert_admitted_external_summary(
        &rows,
        site_id,
        AXUM_STD_MEM_REPLACE_SUMMARY_ID,
        "EventDataWriter::write_buf std::mem::replace",
    );

    Ok(())
}

enum OwnerSpec {
    Method {
        name: &'static str,
        body: &'static str,
    },
    Function {
        module: &'static [&'static str],
        name: &'static str,
    },
}

struct GeneratedCase {
    label: &'static str,
    owner: OwnerSpec,
    min_facts: usize,
    path: &'static [&'static str],
    relation: CallTargetKind,
    summary_records: fn(Uuid) -> Vec<serde_json::Value>,
    boundary_id: fn(Uuid) -> String,
    summary_id: &'static str,
    expanded_item: &'static str,
    definition: &'static str,
}

const OPAQUE_FUTURE_CASE: GeneratedCase = GeneratedCase {
    label: "generated IntoServiceFuture::new",
    owner: OwnerSpec::Method {
        name: "call",
        body: "IntoServiceFuture::new(future)",
    },
    min_facts: 3,
    path: &["super", "future", "IntoServiceFuture", "new"],
    relation: CallTargetKind::AssociatedFunction,
    summary_records: ploke_test_utils::axum_opaque_future_macro_summary_records,
    boundary_id: ploke_test_utils::axum_opaque_future_boundary_id,
    summary_id: ploke_test_utils::AXUM_OPAQUE_FUTURE_SUMMARY_ID,
    expanded_item: "expanded:item:axum-opaque-future-new",
    definition: "def:axum::future::IntoServiceFuture::new",
};

const ROUTING_POST_CASE: GeneratedCase = GeneratedCase {
    label: "generated routing::post",
    owner: OwnerSpec::Function {
        module: &["crate", "json", "tests"],
        name: "deserialize_body",
    },
    min_facts: 2,
    path: &["post"],
    relation: CallTargetKind::Function,
    summary_records: ploke_test_utils::axum_routing_post_macro_summary_records,
    boundary_id: ploke_test_utils::axum_routing_post_boundary_id,
    summary_id: ploke_test_utils::AXUM_ROUTING_POST_SUMMARY_ID,
    expanded_item: "expanded:item:axum-routing-post",
    definition: "def:axum::routing::method_routing::post",
};

const GET_SERVICE_CASE: GeneratedCase = GeneratedCase {
    label: "generated routing::get_service",
    owner: OwnerSpec::Function {
        module: &["crate", "routing", "tests", "get_to_head", "for_services"],
        name: "get_handles_head",
    },
    min_facts: 2,
    path: &["get_service"],
    relation: CallTargetKind::Function,
    summary_records: ploke_test_utils::axum_routing_get_service_macro_summary_records,
    boundary_id: ploke_test_utils::axum_routing_get_service_boundary_id,
    summary_id: ploke_test_utils::AXUM_ROUTING_GET_SERVICE_SUMMARY_ID,
    expanded_item: "expanded:item:axum-routing-get-service",
    definition: "def:axum::routing::method_routing::get_service",
};

#[tokio::test]
async fn proof_context_collection_preserves_axum_generated_constructor_summary() -> Result<(), Error>
{
    run_generated_case(&OPAQUE_FUTURE_CASE)
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_generated_post_resolution() -> Result<(), Error> {
    run_generated_case(&ROUTING_POST_CASE)
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_generated_get_service_resolution()
-> Result<(), Error> {
    run_generated_case(&GET_SERVICE_CASE)
}

fn run_generated_case(case: &GeneratedCase) -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;
    let owner = match &case.owner {
        OwnerSpec::Method { name, body } => method_id_by_name_and_body(&db, name, body)?,
        OwnerSpec::Function { module, name } => function_id(&db, module, name)?,
    };
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= case.min_facts,
        "{} should project its call-site proof rows",
        case.label
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "{} projected facts should enable RAG proof context",
        case.label
    );
    let contexts = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = contexts
        .get(&owner)
        .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
    let callee = CallCalleeInfo::Path {
        path: case.path.iter().map(|part| (*part).to_string()).collect(),
    };
    let generated = calls
        .iter()
        .find(|call| {
            call.owner_id == owner && call.kind == CallSiteKind::Path && call.callee == callee
        })
        .unwrap_or_else(|| panic!("{} call context missing: {calls:#?}", case.label));
    assert_eq!(
        generated.status,
        CallStatusKind::Resolved,
        "{} status",
        case.label
    );
    assert_eq!(
        generated.resolution,
        Some(CallResolutionKind::LocalExact),
        "{} resolution",
        case.label
    );
    assert_eq!(
        generated.targets.len(),
        1,
        "{} target cardinality: {generated:#?}",
        case.label
    );
    assert_eq!(
        &generated.targets[0].relation, &case.relation,
        "{} target relation",
        case.label
    );

    let site_id = generated.site_id;
    let target = generated.targets[0].target_id;
    db.upsert_proof_fact_values(&(case.summary_records)(site_id))?;
    let rows = rag.exact_proof_context(owner)?;
    assert_generated_proof_rows(&rows, owner, site_id, target, case.label);
    let boundary_id = (case.boundary_id)(site_id);
    assert_generated_macro_boundary_rows(
        &rows,
        site_id,
        case.summary_id,
        &boundary_id,
        case.expanded_item,
        case.definition,
        case.label,
    );
    Ok(())
}

fn assert_generated_proof_rows(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site_id: Uuid,
    target: Uuid,
    label: &str,
) {
    let owner = owner.to_string();
    let site = site_id.to_string();
    let target = target.to_string();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.build_domain_id.as_deref() == Some(AXUM_DOMAIN)
        }),
        "{label} should expose the generated call_site: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_edge"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.callee_def_id.as_deref() == Some(target.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
        }),
        "{label} should expose the resolved call_edge: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
                && proof.resolved_def_id.as_deref() == Some(target.as_str())
                && proof.blocker_reason.is_none()
        }),
        "{label} should expose the resolved call_resolution: {rows:#?}"
    );
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_assert_eq_macro_blockers() -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let cases = [
        (
            "axum-core Body test_try_downcast assert_eq",
            &["crate", "body"][..],
            "test_try_downcast",
            2,
        ),
        (
            "axum util test_try_downcast assert_eq",
            &["crate", "util"][..],
            "test_try_downcast",
            2,
        ),
        (
            "axum routing tests shadowed get assert_eq",
            &["crate", "routing", "tests"][..],
            "what_matches_wildcard",
            11,
        ),
    ];

    let mut owners = Vec::new();
    for (label, module_path, function_name, expected_macros) in cases {
        let owner = function_id(&db, module_path, function_name)?;
        let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
        assert!(
            projected >= expected_macros * 2,
            "{label} should project unsupported macro call-site proof rows"
        );
        owners.push((label, owner, expected_macros));
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum assert_eq macro facts should enable RAG proof context"
    );

    for (label, owner, expected_macros) in owners {
        let db_context = db.call_context_for_owner(owner)?;
        let sites = db_context
            .iter()
            .filter(|row| {
                row.site.owner_id == owner
                    && row.site.kind == ploke_db::CallSiteKind::Macro
                    && row.site.macro_name.as_deref() == Some("assert_eq")
                    && row.status.status == ploke_db::CallStatusKind::Unsupported
                    && row.status.resolution.is_none()
                    && row.targets.is_empty()
            })
            .map(|row| row.site.id)
            .collect::<Vec<_>>();
        assert_eq!(
            sites.len(),
            expected_macros,
            "{label} should project exactly {expected_macros} targetless assert_eq! macro rows in DB call context: {db_context:#?}"
        );

        let rows = rag.exact_proof_context(owner)?;

        // Matrix: macro-bound try_downcast rows.
        // Source chain:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //   axum-core/src/body.rs:251-252 wraps `try_downcast` calls in
        //   `assert_eq!`.
        //   axum/src/util.rs:114-115 wraps `try_downcast` calls in
        //   `assert_eq!`.
        //   axum/src/routing/tests/mod.rs:423-434 wraps calls to the
        //   shadowed local `get` closure in `assert_eq!`.
        // Expected proof traversal: exact proof context must expose the two macro
        // call_site rows for the try_downcast owners and eleven macro rows for
        // the shadowed-closure owner, with no flattened path edge from macro
        // arguments.
        for site_id in sites {
            assert_site_blocker(
                &rows,
                owner,
                site_id,
                "macro_expansion_not_available",
                label,
            );
            let site = site_id.to_string();
            assert!(
                rows.iter().all(|row| {
                    row.kind != "call_edge" || row.call_site_id.as_deref() != Some(site.as_str())
                }),
                "{label} macro callsite {site_id} must not project a call_edge: {rows:#?}"
            );
        }
    }

    Ok(())
}

fn assert_generated_macro_boundary_rows(
    rows: &[ProofContextInfo],
    site_id: Uuid,
    summary_id: &str,
    boundary_id: &str,
    expanded_item_id: &str,
    definition_id: &str,
    label: &str,
) {
    let site = site_id.to_string();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "expansion_boundary"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.boundary_id.as_deref() == Some(boundary_id)
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.status.as_deref() == Some("externally_summarized")
                && proof.blocker_reason.is_none()
        }),
        "RAG proof context should expose the admitted {label} boundary summary linked to the generated function callsite: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == ["external_summary_boundary".to_string()]
        }),
        "RAG proof context should expose the admitted {label} summary artifact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "expanded_item"
                && proof.expanded_item_id.as_deref() == Some(expanded_item_id)
                && proof.boundary_id.as_deref() == Some(boundary_id)
                && proof.definition_id.as_deref() == Some(definition_id)
        }),
        "RAG proof context should expose the {label} generated-item linkage: {rows:#?}"
    );
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
async fn proof_context_collection_preserves_axum_callback_parameter_ambiguity() -> Result<(), Error>
{
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
        "expand_attr_with IIFE closure owner should project ambiguous callback proof rows"
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
    let callee = CallCalleeInfo::Path {
        path: vec!["f".to_string()],
    };
    let callback = calls
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee == callee
                && call.status == CallStatusKind::Ambiguous
                && call.resolution.is_none()
        })
        .unwrap_or_else(|| {
            panic!(
                "axum-macros/src/lib.rs:737 f(attr, input) should expose one ambiguous closure-candidate row: {calls:#?}"
            )
        });
    assert_eq!(
        callback.targets.len(),
        2,
        "axum callback parameter should expose both closure candidates: {callback:#?}"
    );
    assert!(
        callback
            .targets
            .iter()
            .all(|target| target.relation == CallTargetKind::Closure),
        "axum callback parameter candidates should all be closures: {callback:#?}"
    );
    let site_id = callback.site_id;

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .expect("expand_attr_with IIFE closure owner should receive projected proof rows");

    assert_ambiguous_proof_rows(
        rows,
        owner,
        site_id,
        CandidateContract::Count(2),
        "axum callback parameter",
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_memchr_callable_trait_object_blockers()
-> Result<(), Error> {
    init_tracing_once();
    let db = memchr_db()?;

    let owner = method_id_by_file(
        &db,
        "run",
        "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
        "src/tests/substring/mod.rs",
    )?;
    let projected = db.project_call_proof_facts_for_owner(owner, MEMCHR_DOMAIN)?;
    assert!(
        projected >= 4,
        "Runner::run should project targetless fwd/rev callable trait-object proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected memchr callable trait-object facts should enable RAG proof context"
    );
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("Runner::run should receive outgoing call context");
    let sites = [
        (
            targetless_path_site(
                calls,
                owner,
                &["fwd"],
                CallStatusKind::Unsupported,
                "Runner::run fwd boxed dyn FnMut",
            ),
            "Runner::run fwd boxed dyn FnMut",
        ),
        (
            targetless_path_site(
                calls,
                owner,
                &["rev"],
                CallStatusKind::Unsupported,
                "Runner::run rev boxed dyn FnMut",
            ),
            "Runner::run rev boxed dyn FnMut",
        ),
    ];
    db.upsert_proof_fact_values(&[
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_blocker(sites[0].0),
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_blocker(sites[1].0),
    ])?;

    let rows = rag.exact_proof_context(owner)?;

    // Matrix: callable trait-object local binding row.
    // Source chain:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   memchr/src/tests/substring/mod.rs:67-71 defines boxed `fwd`/`rev`
    //   `dyn FnMut` fields.
    //   memchr/src/tests/substring/mod.rs:94 calls
    //   `fwd(t.haystack.as_bytes(), t.needle.as_bytes())`.
    //   memchr/src/tests/substring/mod.rs:110 calls
    //   `rev(t.haystack.as_bytes(), t.needle.as_bytes())`.
    // Expected proof traversal: owner-seeded proof context must include the
    // structural path call_site and blocked call_resolution facts, plus an
    // explicit dynamic-dispatch blocker. There are zero callee edges until
    // callable trait-object value-flow proof is modeled.
    for (site_id, label) in sites {
        assert_site_blocker_in_domain(
            &rows,
            owner,
            site_id,
            MEMCHR_DOMAIN,
            "type_resolution_missing",
            label,
        );
        assert_explicit_proof_blocker(&rows, site_id, "dynamic_dispatch_unbounded", label);
    }

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

enum CandidateContract<'a> {
    Count(usize),
    Exact(&'a [Uuid]),
}

fn assert_ambiguous_proof_rows(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site_id: Uuid,
    contract: CandidateContract<'_>,
    label: &str,
) {
    let owner = owner.to_string();
    let site = site_id.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.build_domain_id.as_deref() == Some(AXUM_DOMAIN)
        }),
        "{label} should include the call_site fact: {rows:#?}"
    );
    let resolution = rows
        .iter()
        .find(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.resolution_state.as_deref() == Some("ambiguous")
        })
        .unwrap_or_else(|| panic!("{label} should include ambiguous resolution: {rows:#?}"));
    match contract {
        CandidateContract::Count(count) => assert_eq!(
            resolution.candidate_def_ids.len(),
            count,
            "{label} candidate cardinality"
        ),
        CandidateContract::Exact(expected) => {
            assert_eq!(
                resolution.candidate_def_ids.len(),
                expected.len(),
                "{label} candidate cardinality"
            );
            let mut actual = resolution.candidate_def_ids.clone();
            actual.sort();
            let mut expected = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
            expected.sort();
            assert_eq!(actual, expected, "{label} exact candidate identities");
        }
    }
    assert!(
        rows.iter().all(|row| {
            row.kind != "call_edge" || row.call_site_id.as_deref() != Some(site.as_str())
        }),
        "{label} should not fabricate a call_edge: {rows:#?}"
    );
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_dynamic_callable_blockers() -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let cases = [
        DynamicCase {
            label: "MakeErasedRouter::into_route callable field",
            method: "into_route",
            body: "(self.into_route)(self.router, state)",
            expected_path: &["self", "into_route"],
        },
        DynamicCase {
            label: "TapIo::accept callable field",
            method: "accept",
            body: "(self.tap_fn)(&mut io)",
            expected_path: &["self", "tap_fn"],
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
        let site_id = dynamic_site(calls, owner, case.expected_path, case.label);

        let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
        let rows = proof_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

        // Matrix: dynamic unsupported callable rows.
        // Source chain:
        //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
        //   axum/src/boxed.rs:120 calls `(self.into_route)(self.router, state)`.
        //   axum/src/serve/listener.rs:236 calls `(self.tap_fn)(&mut io)`.
        // Expected proof traversal: owner-seeded proof context must include the
        // call_site plus blocked call_resolution facts for each unsupported,
        // targetless dynamic call site. There are zero callee edges until
        // callable-field proof is modeled.
        assert_blocked_resolution(rows, owner, "dynamic_dispatch_unbounded");
        assert_site_blocker(
            rows,
            owner,
            site_id,
            "dynamic_dispatch_unbounded",
            case.label,
        );
        assert_self_field_evidence(
            rows,
            owner,
            site_id,
            case.expected_path,
            "blocked",
            case.label,
        );
    }

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_handler_dynamic_callable_resolution()
-> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let owner =
        method_id_by_name_and_body(&db, "into_route", "(self.into_route)(self.handler, state)")?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
    assert!(
        projected >= 3,
        "MakeErasedHandler::into_route should project resolved dynamic callable proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum handler dynamic facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("MakeErasedHandler::into_route should receive outgoing call context");
    let dynamic = calls
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call.status == CallStatusKind::Resolved
                && call.resolution == Some(CallResolutionKind::LocalExact)
                && call.path.as_ref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["self", "into_route"])
                })
        })
        .unwrap_or_else(|| {
            panic!(
                "MakeErasedHandler::into_route should expose the resolved dynamic function-pointer field row: {calls:#?}"
            )
        });
    assert_eq!(
        dynamic.targets.len(),
        1,
        "MakeErasedHandler::into_route should expose one closure initializer target: {dynamic:#?}"
    );
    assert_eq!(dynamic.targets[0].relation, CallTargetKind::DynamicClosure);

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .expect("MakeErasedHandler::into_route should receive projected proof rows");

    // Matrix: resolved dynamic handler callable row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/boxed.rs:85 calls `(self.into_route)(self.handler, state)`.
    //   `BoxedIntoRoute::from_handler` at boxed.rs:23-25 initializes the
    //   function-pointer field with the unique local closure.
    // Expected proof traversal: proof context exposes call_site, resolved
    // call_edge, and resolved call_resolution rows for the DynamicClosure
    // edge, with no blocker for this formerly targetless row.
    let owner_text = owner.to_string();
    let site = dynamic.site_id.to_string();
    let target = dynamic.targets[0].target_id.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.caller_def_id.as_deref() == Some(owner_text.as_str())
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.build_domain_id.as_deref() == Some(AXUM_DOMAIN)
        }),
        "MakeErasedHandler::into_route proof context should include the call_site fact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.caller_def_id.as_deref() == Some(owner_text.as_str())
                && row.callee_def_id.as_deref() == Some(target.as_str())
                && row.resolution_state.as_deref() == Some("resolved")
        }),
        "MakeErasedHandler::into_route proof context should include the resolved call_edge fact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.resolution_state.as_deref() == Some("resolved")
                && row.resolved_def_id.as_deref() == Some(target.as_str())
                && row.blocker_reason.is_none()
        }),
        "MakeErasedHandler::into_route proof context should include the resolved call_resolution fact: {rows:#?}"
    );
    assert_self_field_evidence(
        rows,
        owner,
        dynamic.site_id,
        &["self", "into_route"],
        "resolved",
        "MakeErasedHandler::into_route",
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_axum_layer_dynamic_callable_candidates()
-> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    let cases = [
        DynamicCase {
            label: "Map::into_route layer trait object",
            method: "into_route",
            body: "(self.layer)(self.inner.into_route(state))",
            expected_path: &["self", "layer"],
        },
        DynamicCase {
            label: "Map::call_with_state layer trait object",
            method: "call_with_state",
            body: "(self.layer)(self.inner.into_route(state)).call(request)",
            expected_path: &["self", "layer"],
        },
    ];

    let expected_candidates = axum_layer_dynamic_candidate_ids(&db)?;
    let mut owners = Vec::new();
    for case in cases {
        let owner = method_id_by_name_and_body(&db, case.method, case.body)?;
        let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
        assert!(
            projected >= 2,
            "{} should project ambiguous dynamic layer proof rows",
            case.label
        );
        owners.push((case, owner));
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum dynamic layer facts should enable RAG proof context"
    );

    for (case, owner) in owners {
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let calls = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let matching = calls
            .iter()
            .filter(|call| {
                call.owner_id == owner
                    && call.kind == CallSiteKind::Dynamic
                    && call.callee == CallCalleeInfo::Dynamic
                    && call.status == CallStatusKind::Ambiguous
                    && call.resolution.is_none()
                    && call.path.as_ref().is_some_and(|path| {
                        path.iter()
                            .map(String::as_str)
                            .eq(case.expected_path.iter().copied())
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "{} should expose one ambiguous dynamic layer row: {calls:#?}",
            case.label
        );
        let call = matching[0];
        assert_eq!(
            call.targets.len(),
            expected_candidates.len(),
            "{} should preserve every reviewed dynamic layer closure candidate: {call:#?}",
            case.label
        );
        assert!(
            call.targets
                .iter()
                .all(|target| target.relation == CallTargetKind::DynamicClosure),
            "{} should expose only dynamic-closure candidates: {call:#?}",
            case.label
        );
        let mut actual_targets = call
            .targets
            .iter()
            .map(|target| target.target_id)
            .collect::<Vec<_>>();
        actual_targets.sort_unstable();
        assert_eq!(
            actual_targets, expected_candidates,
            "{} call context should preserve the reviewed dynamic layer candidates",
            case.label
        );

        let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
        let rows = proof_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive projected proof rows", case.label));

        assert_ambiguous_proof_rows(
            rows,
            owner,
            call.site_id,
            CandidateContract::Exact(&expected_candidates),
            case.label,
        );
        assert_self_field_evidence(
            rows,
            owner,
            call.site_id,
            case.expected_path,
            "ambiguous",
            case.label,
        );
    }

    Ok(())
}

fn assert_self_field_evidence(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site_id: Uuid,
    expected_path: &[&str],
    expected_state: &str,
    label: &str,
) {
    let owner = owner.to_string();
    let site = site_id.to_string();
    let path = expected_path.join(".");
    assert!(
        rows.iter().any(|row| {
            row.kind == "binding_evidence"
                && row
                    .fact_id
                    .starts_with("binding-evidence:self-field-callable:")
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.resolution_state.as_deref() == Some(expected_state)
                && row.evidence_use.as_deref() == Some("proof_only")
                && row.detail.as_deref().is_some_and(|detail| {
                    detail.contains("callable self-field")
                        && detail.contains(path.as_str())
                        && match expected_state {
                            "resolved" => detail.contains("supports the admitted traversal edge"),
                            _ => {
                                detail.contains("field value flow is proven")
                                    || (detail.contains("storing parameter")
                                        && detail.contains("traversal remains targetless"))
                            }
                        }
                })
        }),
        "{label} proof context should include self-field binding evidence for site {site}: {rows:#?}"
    );
}

fn axum_layer_dynamic_candidate_ids(db: &Database) -> Result<Vec<Uuid>, Error> {
    let method_router_layer = method_id_by_name_and_body(
        db,
        "layer",
        "let layer_fn = move |route: Route<E>| route.layer(layer.clone());",
    )?;
    let method_router_route_layer = method_id_by_name_and_body(
        db,
        "route_layer",
        "let layer_fn = move |svc| Route::new(layer.layer(svc));",
    )?;
    let router_layer = method_id_by_file(
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
