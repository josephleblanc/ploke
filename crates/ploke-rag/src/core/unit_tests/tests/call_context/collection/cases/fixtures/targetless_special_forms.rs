use ploke_db::{CallPathOptions, ProofGraphStore};
use serde_json::json;

use super::super::super::super::super::*;
use super::super::super::helpers::*;

fn process_create_effect_seed(
    call_site_id: impl ToString,
    effect_seed_id: &str,
) -> serde_json::Value {
    json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": effect_seed_id,
        "call_site_id": call_site_id.to_string(),
        "effect_class": "operating_system_process_create",
        "confidence": "fixture-source-oracle",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })
}

#[tokio::test]
async fn call_context_collection_reads_real_targetless_special_form_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let extern_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_extern_c_function"),
    )?;
    let chained_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_chained_returned_function"),
    )?;
    let qself_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_qualified_dyn_any_downcast_mut"),
    )?;
    let chained_target = one_uuid(&db, &function_in_module_query(&["crate"], "make_unary_fn"))?;
    let returned_target = one_uuid(&db, &function_in_module_query(&["crate"], "unary_target"))?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable targetless special-form call context"
    );

    let call_context = rag.collect_call_context(&[
        (extern_owner, 1.0),
        (chained_owner, 1.0),
        (qself_owner, 1.0),
    ])?;

    let extern_context = call_context
        .get(&extern_owner)
        .expect("extern C owner should receive outgoing call context");
    assert_eq!(
        extern_context.len(),
        1,
        "extern C owner context: {extern_context:#?}"
    );
    let extern_call = &extern_context[0];
    assert_eq!(extern_call.kind, CallSiteKind::Path);
    assert_eq!(
        extern_call.callee,
        CallCalleeInfo::Path {
            path: vec!["abs".to_string()],
        }
    );
    assert_eq!(extern_call.status, CallStatusKind::External);
    assert!(extern_call.resolution.is_none());
    assert!(
        extern_call.targets.is_empty(),
        "extern C calls must not fabricate RAG targets: {extern_call:#?}"
    );

    let chained_context = call_context
        .get(&chained_owner)
        .expect("chained returned-function owner should receive outgoing call context");
    assert_eq!(
        chained_context.len(),
        2,
        "chained returned-function owner context: {chained_context:#?}"
    );
    let path_call = chained_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["make_unary_fn".to_string()],
                    }
        })
        .expect("inner make_unary_fn path call should stay visible");
    assert_eq!(path_call.status, CallStatusKind::Resolved);
    assert_eq!(path_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path_call.targets.len(), 1);
    assert_eq!(path_call.targets[0].target_id, chained_target);
    assert_eq!(path_call.targets[0].relation, CallTargetKind::Function);

    let dynamic_call = chained_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == returned_target)
        })
        .expect("outer chained returned-function dynamic call should stay visible");
    assert_eq!(dynamic_call.callee, CallCalleeInfo::Dynamic);
    assert_eq!(dynamic_call.status, CallStatusKind::Resolved);
    assert_eq!(
        dynamic_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(dynamic_call.targets.len(), 1);
    assert_eq!(dynamic_call.targets[0].target_id, returned_target);
    assert_eq!(
        dynamic_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    let qself_context = call_context
        .get(&qself_owner)
        .expect("qualified dyn Any owner should receive outgoing call context");
    assert_eq!(
        qself_context.len(),
        1,
        "qualified dyn Any owner context: {qself_context:#?}"
    );
    let qself_call = &qself_context[0];
    assert_eq!(qself_call.kind, CallSiteKind::Path);
    assert_eq!(
        qself_call.callee,
        CallCalleeInfo::Path {
            path: vec![
                "std".to_string(),
                "any".to_string(),
                "Any".to_string(),
                "downcast_mut".to_string(),
            ],
        }
    );
    assert_eq!(qself_call.status, CallStatusKind::External);
    assert!(qself_call.resolution.is_none());
    assert!(
        qself_call.targets.is_empty(),
        "qualified dyn Any calls must not fabricate RAG targets: {qself_call:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_reach_exact_preserves_extern_c_external_frontier() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let extern_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_extern_c_function"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable exact extern C reach"
    );

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which call paths can reach unsafe blocks or FFI boundaries?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:844 declares
    //   `abs(value)` inside an `unsafe extern "C"` block and calls it from
    //   `call_extern_c_function`. RAG should expose the DB external frontier
    //   row without inventing a local callee.
    let report = rag
        .exact_call_reach_for_owner(
            extern_owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");

    assert_eq!(report.owner.id, extern_owner);
    assert_eq!(report.owner.name, "call_extern_c_function");
    assert!(
        report.paths.is_empty() && report.callees.is_empty(),
        "extern C calls should not fabricate RAG reach edges: {report:#?}"
    );
    let external_call = report
        .external_frontier_calls
        .iter()
        .find(|call| {
            call.owner_id == extern_owner
                && call.kind == CallSiteKind::Path
                && call.status == CallStatusKind::External
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["abs".to_string()],
                    }
        })
        .unwrap_or_else(|| {
            panic!("RAG extern C reach should expose abs(value) as external: {report:#?}")
        });
    assert_eq!(external_call.arg_count, Some(1));
    assert!(
        external_call.targets.is_empty(),
        "RAG extern C frontier must remain targetless: {external_call:#?}"
    );
    assert!(
        report
            .frontier_calls
            .iter()
            .any(|call| call.site_id == external_call.site_id),
        "full frontier list should contain the same extern C callsite: {report:#?}"
    );
    assert!(
        report.unsupported_frontier_calls.is_empty()
            && report.unresolved_frontier_calls.is_empty()
            && report.ambiguous_frontier_calls.is_empty(),
        "RAG extern C reach should classify the FFI boundary as external only: {report:#?}"
    );
    assert!(
        report
            .source_files
            .iter()
            .any(|file| file.as_ref().ends_with("fixture_call_graph/src/lib.rs")),
        "RAG extern C reach should point back to the fixture source file: {report:#?}"
    );

    assert!(
        db.project_call_proof_facts_for_owner(extern_owner, "bd:fixture-call-graph")? >= 2,
        "extern C owner should project targetless proof rows"
    );
    db.upsert_proof_fact_values(&[ploke_test_utils::fixture_extern_c_abs_effect_record(
        external_call.site_id,
    )])?;
    let effects = rag
        .exact_call_effects_reachable_from_owner(
            extern_owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == "effect:fixture-extern-c-abs")
        .unwrap_or_else(|| {
            panic!("RAG extern C reach should expose the FFI boundary effect seed: {effects:#?}")
        });
    assert_eq!(effect.effect_class, "ffi_boundary");
    assert_eq!(effect.confidence.as_deref(), Some("fixture-source-oracle"));
    assert_eq!(effect.blocker_if_unresolved, Some(true));
    assert_eq!(effect.call_site.site_id, external_call.site_id);
    assert_eq!(effect.call_site.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "direct extern C effect should not invent a self path to the owner: {effect:#?}"
    );
    assert!(
        effect
            .blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "RAG extern C effect should preserve the external-summary blocker reason: {effect:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_invariant_findings_exact_preserves_extern_c_process_obligation() -> Result<(), Error>
{
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let extern_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_extern_c_function"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable exact proof invariant findings"
    );

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/process lifetime review:
    //   "Which reachable process-spawn proof obligations are still blocked?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:844 declares
    //   `abs(value)` inside an `unsafe extern "C"` block and calls it from
    //   `call_extern_c_function`. The process-create proof row is explicit
    //   test evidence; the external callsite remains targetless.
    let report = rag
        .exact_call_reach_for_owner(
            extern_owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    let external_call = report
        .external_frontier_calls
        .iter()
        .find(|call| {
            call.owner_id == extern_owner
                && call.kind == CallSiteKind::Path
                && call.status == CallStatusKind::External
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["abs".to_string()],
                    }
        })
        .unwrap_or_else(|| {
            panic!("RAG extern C reach should expose abs(value) as external: {report:#?}")
        });
    assert!(
        external_call.targets.is_empty(),
        "extern C process frontier should remain targetless: {external_call:#?}"
    );
    assert!(
        db.project_call_proof_facts_for_owner(extern_owner, "bd:fixture-call-graph")? >= 2,
        "extern C owner should project targetless proof rows"
    );

    db.upsert_proof_fact_values(&[process_create_effect_seed(
        external_call.site_id,
        "effect:fixture-rag-extern-c-process-create",
    )])?;

    let findings = rag
        .exact_call_proof_invariant_findings_for_owner(
            extern_owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    let finding = findings
        .iter()
        .find(|finding| {
            finding.invariant == "detached_process_successor_handoff"
                && finding.call_site_id.as_deref()
                    == Some(external_call.site_id.to_string().as_str())
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose the fixture process invariant finding: {findings:#?}")
        });
    assert_eq!(finding.status, "blocked");
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "RAG invariant finding should preserve the external-summary blocker: {finding:#?}"
    );
    let Some(call_site) = finding.call_site.as_ref() else {
        panic!("RAG invariant finding should include the linked callsite: {finding:#?}");
    };
    assert_eq!(call_site.site_id, external_call.site_id);
    assert_eq!(call_site.owner_id, extern_owner);
    assert_eq!(call_site.status, CallStatusKind::External);
    assert!(
        call_site.targets.is_empty(),
        "proof invariant findings must not fabricate RAG target rows: {finding:#?}"
    );

    Ok(())
}
