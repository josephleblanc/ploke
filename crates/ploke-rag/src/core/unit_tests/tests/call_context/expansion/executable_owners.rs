use super::super::super::*;

#[tokio::test]
async fn call_context_expansion_adds_closure_owner_for_local_target() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "closure_body_call_is_not_outer_call_site"),
    )?;
    let closure = closure_owner_for_parent(&db, outer)?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 128;
    rag.cfg.call_context.max_caller_hits = 1024;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable closure owner expansion"
    );

    let (expanded, expansion_info) = rag.expand_hits_with_call_context_info(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&closure),
        "local_target incoming expansion should include the closure executable owner: expanded={expanded:#?}; expansion_info={expansion_info:#?}"
    );
    assert!(
        !expanded_ids.contains(&outer),
        "local_target incoming expansion must not flatten the closure-body call into the outer function: expanded={expanded:#?}; expansion_info={expansion_info:#?}"
    );

    let info = expansion_info.get(&closure).unwrap_or_else(|| {
        panic!("closure executable owner should carry CallExpansionInfo: {expansion_info:#?}")
    });
    assert_eq!(info.seed_id, target);
    assert_eq!(info.relation, CallExpansionKind::IncomingCaller);
    assert_eq!(info.target_id, target);
    assert_eq!(info.distance, 1);

    Ok(())
}

#[tokio::test]
async fn call_context_expansion_adds_async_block_owner_for_local_target() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "async_block_call_is_not_outer_call_site"),
    )?;
    let async_body = async_block_owner_for_parent(&db, outer)?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 128;
    rag.cfg.call_context.max_caller_hits = 1024;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable async block owner expansion"
    );

    let (expanded, expansion_info) = rag.expand_hits_with_call_context_info(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&async_body),
        "local_target incoming expansion should include the async block owner: expanded={expanded:#?}; expansion_info={expansion_info:#?}"
    );
    assert!(
        !expanded_ids.contains(&outer),
        "local_target incoming expansion must not flatten the async-block call into the outer function: expanded={expanded:#?}; expansion_info={expansion_info:#?}"
    );

    let info = expansion_info.get(&async_body).unwrap_or_else(|| {
        panic!("async block owner should carry CallExpansionInfo: {expansion_info:#?}")
    });
    assert_eq!(info.seed_id, target);
    assert_eq!(info.relation, CallExpansionKind::IncomingCaller);
    assert_eq!(info.target_id, target);
    assert_eq!(info.distance, 1);

    Ok(())
}

#[tokio::test]
async fn call_context_expansion_adds_async_closure_owner_for_local_target() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_async_closure_literal_with_body_call"),
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 128;
    rag.cfg.call_context.max_caller_hits = 1024;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable async closure owner expansion"
    );

    let (expanded, expansion_info) = rag.expand_hits_with_call_context_info(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&async_closure),
        "local_target incoming expansion should include the async closure owner: expanded={expanded:#?}; expansion_info={expansion_info:#?}"
    );
    assert!(
        !expanded_ids.contains(&outer),
        "local_target incoming expansion must not flatten the async-closure call into the outer function: expanded={expanded:#?}; expansion_info={expansion_info:#?}"
    );

    let info = expansion_info.get(&async_closure).unwrap_or_else(|| {
        panic!("async closure owner should carry CallExpansionInfo: {expansion_info:#?}")
    });
    assert_eq!(info.seed_id, target);
    assert_eq!(info.relation, CallExpansionKind::IncomingCaller);
    assert_eq!(info.target_id, target);
    assert_eq!(info.distance, 1);

    Ok(())
}
