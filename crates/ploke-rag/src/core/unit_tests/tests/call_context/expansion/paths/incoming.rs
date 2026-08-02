use super::*;

#[tokio::test]
async fn call_context_expansion_adds_incoming_fixture_callers() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let caller_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming caller expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&caller_owner),
        "target-centered expansion should materialize the caller owner; expanded: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    let caller_context = call_context
        .get(&caller_owner)
        .expect("caller owner should receive outgoing call context");
    let path_call = caller_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["try_local_assoc".to_string()],
                    }
        })
        .expect("caller should preserve the call edge to try_local_assoc");
    assert_eq!(path_call.status, CallStatusKind::Resolved);
    assert_eq!(path_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path_call.targets.len(), 1);
    assert_eq!(path_call.targets[0].target_id, target);
    assert_eq!(path_call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_expansion_respects_max_caller_hits_by_score() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let high_target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let high_caller = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;
    let low_target = one_uuid(&db, &method_by_impl_self_query("LocalAssoc", "make"))?;
    let low_self_caller = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "call_self_make"),
    )?;
    let low_qualified_caller = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_qualified_local_assoc_make"),
    )?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 2;
    rag.cfg.call_context.max_caller_hits = 1;
    rag.cfg.call_context.caller_factor = 0.5;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(high_target, 1.0), (low_target, 0.2)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert_eq!(
        expanded_ids.len(),
        3,
        "max_caller_hits=1 should add exactly one caller to the two seed hits: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&high_target) && expanded_ids.contains(&low_target),
        "incoming caller expansion must preserve seed hits: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&high_caller),
        "higher-scored target should contribute the sole caller hit: {expanded:#?}"
    );
    assert!(
        !expanded_ids.contains(&low_self_caller) && !expanded_ids.contains(&low_qualified_caller),
        "lower-scored associated-function callers should be truncated by max_caller_hits=1: {expanded:#?}"
    );

    let high_score = expanded
        .iter()
        .find(|(id, _)| *id == high_caller)
        .map(|(_, score)| *score)
        .expect("high caller should be present");
    assert_eq!(high_score, 0.5);

    Ok(())
}
