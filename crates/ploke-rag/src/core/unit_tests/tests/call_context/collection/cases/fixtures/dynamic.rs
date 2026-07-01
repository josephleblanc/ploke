use super::super::super::super::super::*;
use super::super::super::helpers::*;
#[tokio::test]
async fn call_context_collection_reads_real_fixture_dynamic_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let resolved_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_aliased_indexed_named_field_function_binding",
        ),
    )?;
    let branch_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_parenthesized_match_initialized_function_item_binding",
        ),
    )?;
    let block_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_parenthesized_block_initialized_function_item_binding",
        ),
    )?;
    let guarded_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_match_guarded_function_item"),
    )?;
    let unsupported_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_dereferenced_closure_binding"),
    )?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable dynamic call context collection"
    );

    let call_context = rag.collect_call_context(&[
        (resolved_owner, 1.0),
        (branch_owner, 1.0),
        (block_owner, 1.0),
        (guarded_owner, 1.0),
        (unsupported_owner, 1.0),
    ])?;
    let resolved_context = call_context
        .get(&resolved_owner)
        .expect("resolved dynamic owner should receive outgoing call context");
    let resolved_call = resolved_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("resolved dynamic function call should stay visible in RAG call context");
    assert_eq!(resolved_call.status, CallStatusKind::Resolved);
    assert_eq!(
        resolved_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(resolved_call.targets.len(), 1);
    assert_eq!(resolved_call.targets[0].target_id, target);
    assert_eq!(
        resolved_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    for (label, owner) in [
        ("branch-initialized", branch_owner),
        ("block-initialized", block_owner),
        ("guarded-match", guarded_owner),
    ] {
        let context = call_context.get(&owner).unwrap_or_else(|| {
            panic!("{label} dynamic owner should receive outgoing call context")
        });
        assert_eq!(
            context.len(),
            1,
            "{label} dynamic owner context: {context:#?}"
        );
        let call = &context[0];
        assert_eq!(call.kind, CallSiteKind::Dynamic, "{label}");
        assert_eq!(call.callee, CallCalleeInfo::Dynamic, "{label}");
        assert_eq!(call.status, CallStatusKind::Resolved, "{label}");
        assert_eq!(
            call.resolution,
            Some(CallResolutionKind::LocalExact),
            "{label}"
        );
        assert_eq!(call.targets.len(), 1, "{label}");
        assert_eq!(call.targets[0].target_id, target, "{label}");
        assert_eq!(
            call.targets[0].relation,
            CallTargetKind::DynamicFunction,
            "{label}"
        );
    }

    let unsupported_context = call_context
        .get(&unsupported_owner)
        .expect("unsupported dynamic owner should receive outgoing call context");
    assert_eq!(
        unsupported_context.len(),
        1,
        "unsupported dynamic owner context: {unsupported_context:#?}"
    );
    let unsupported_call = &unsupported_context[0];
    assert_eq!(unsupported_call.kind, CallSiteKind::Dynamic);
    assert_eq!(unsupported_call.callee, CallCalleeInfo::Dynamic);
    assert_eq!(unsupported_call.status, CallStatusKind::Unsupported);
    assert!(unsupported_call.resolution.is_none());
    assert!(
        unsupported_call.targets.is_empty(),
        "unsupported dynamic calls must not fabricate RAG targets: {unsupported_call:#?}"
    );

    Ok(())
}
