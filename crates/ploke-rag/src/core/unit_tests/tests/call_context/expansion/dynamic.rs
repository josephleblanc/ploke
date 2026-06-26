use super::super::super::*;

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_expansion_adds_incoming_fixture_dynamic_callers() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let dynamic_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_aliased_indexed_named_field_function_binding",
        ),
    )?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 128;
    rag.cfg.call_context.max_caller_hits = 1024;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable dynamic caller expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming dynamic caller expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&dynamic_owner),
        "function target expansion should materialize a dynamic-function caller owner; expanded: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    let dynamic_context = call_context
        .get(&dynamic_owner)
        .expect("dynamic caller owner should receive outgoing call context");
    let dynamic_call = dynamic_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("dynamic caller should preserve the DynamicFunction edge to local_target");
    assert_eq!(dynamic_call.status, CallStatusKind::Resolved);
    assert_eq!(
        dynamic_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(dynamic_call.targets.len(), 1);
    assert_eq!(dynamic_call.targets[0].target_id, target);
    assert_eq!(
        dynamic_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    Ok(())
}
