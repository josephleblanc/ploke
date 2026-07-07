use ploke_db::CallPathOptions;

use super::super::super::super::super::*;
use super::super::super::helpers::*;

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

    Ok(())
}
