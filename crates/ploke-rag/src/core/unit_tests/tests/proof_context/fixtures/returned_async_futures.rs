use super::super::super::*;
use super::super::helpers::assert_resolved_call;
use ploke_db::{CallSiteKind as DbCallSiteKind, CallStatusKind as DbCallStatusKind};

struct ResolvedCase {
    label: &'static str,
    owner: Uuid,
    dynamic_site: Uuid,
    closure: Uuid,
}

#[tokio::test]
async fn proof_context_collection_preserves_returned_async_future_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let maker = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_returned_async_closure"),
    )?;
    let resolved_cases = [
        resolved_case(
            &db,
            "awaited returned async closure",
            "call_awaited_returned_async_closure",
        )?,
        resolved_case(
            &db,
            "stored returned async closure future",
            "call_stored_returned_async_closure",
        )?,
    ];

    for case in &resolved_cases {
        let count = db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 7, "{} proof fact count", case.label);
    }

    let producer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_forwarded_returned_async_future"),
    )?;
    let producer_context = db.call_context_for_owner(producer)?;
    let producer_dynamic = producer_context
        .iter()
        .find(|row| {
            row.site.kind == DbCallSiteKind::Dynamic
                && row.site.path.as_ref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["make_returned_async_closure"])
                })
        })
        .unwrap_or_else(|| {
            panic!("forwarded returned async future producer dynamic row: {producer_context:#?}")
        });
    assert_eq!(
        producer_dynamic.status.status,
        DbCallStatusKind::Unsupported
    );
    assert!(
        producer_dynamic.targets.is_empty(),
        "forwarded returned async future producer must stay targetless: {producer_dynamic:#?}"
    );
    let producer_site = producer_dynamic.site.id;
    assert_eq!(
        db.project_call_proof_facts_for_owner(producer, "bd:fixture-call-graph")?,
        7,
        "forwarded returned async future producer proof fact count"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected returned async future proof facts should enable RAG proof context"
    );

    for case in &resolved_cases {
        let proof_context = rag.collect_proof_context(&[(case.owner, 1.0)])?;
        let rows = proof_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner seed should receive proof rows", case.label));
        assert_eq!(rows.len(), 7, "{} proof rows: {rows:#?}", case.label);
        assert_resolved_call(rows, case.owner, maker);
        assert_resolved_call(rows, case.owner, case.closure);
        assert_returned_callable_evidence(rows, case.owner, case.dynamic_site, "resolved");
    }

    let proof_context = rag.collect_proof_context(&[(producer, 1.0)])?;
    let rows = proof_context
        .get(&producer)
        .expect("forwarded returned async future producer should receive proof rows");
    assert_eq!(
        rows.len(),
        7,
        "forwarded returned async future proof rows: {rows:#?}"
    );
    assert_resolved_call(rows, producer, maker);
    assert_returned_callable_evidence(rows, producer, producer_site, "blocked");
    assert_poll_resume_blocker(rows, producer_site);
    let producer_site_text = producer_site.to_string();
    assert!(
        rows.iter().all(|row| {
            row.kind != "call_edge"
                || row.call_site_id.as_deref() != Some(producer_site_text.as_str())
        }),
        "blocked returned future dynamic row must not project a call_edge: {rows:#?}"
    );

    Ok(())
}

fn resolved_case(
    db: &Database,
    label: &'static str,
    owner_name: &'static str,
) -> Result<ResolvedCase, Error> {
    let owner = one_uuid(db, &function_in_module_query(&["crate"], owner_name))?;
    let context = db.call_context_for_owner(owner)?;
    let dynamic = context
        .iter()
        .find(|row| {
            row.site.kind == DbCallSiteKind::Dynamic
                && row.site.path.as_ref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["make_returned_async_closure"])
                })
        })
        .unwrap_or_else(|| panic!("{label} dynamic row: {context:#?}"));
    assert_eq!(dynamic.status.status, DbCallStatusKind::Resolved);
    assert_eq!(
        dynamic.targets.len(),
        1,
        "{label} should resolve exactly one returned async closure target"
    );
    Ok(ResolvedCase {
        label,
        owner,
        dynamic_site: dynamic.site.id,
        closure: dynamic.targets[0].target_id,
    })
}

fn assert_returned_callable_evidence(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site: Uuid,
    state: &str,
) {
    let owner = owner.to_string();
    let site = site.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "binding_evidence"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.resolution_state.as_deref() == Some(state)
                && row.evidence_use.as_deref() == Some("proof_only")
                && row
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("callable returned by"))
        }),
        "proof context should include {state} returned-callable binding evidence for {site}: {rows:#?}"
    );
}

fn assert_poll_resume_blocker(rows: &[ProofContextInfo], site: Uuid) {
    let site = site.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "proof_blocker"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
                && row.status.as_deref() == Some("blocked")
                && row.detail.as_deref().is_some_and(|detail| {
                    detail.contains("returned future") && detail.contains("async poll/resume")
                })
        }),
        "proof context should include returned-future poll/resume blocker for {site}: {rows:#?}"
    );
}
