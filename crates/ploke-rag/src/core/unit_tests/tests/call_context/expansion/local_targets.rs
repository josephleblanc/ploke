use super::super::super::*;
#[tokio::test]
async fn call_context_expansion_excludes_closure_async_outer_owners_for_local_target()
-> Result<(), Error> {
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
    let forbidden_owners = [
        one_uuid(
            &db,
            &function_in_module_query(&["crate"], "closure_body_call_is_not_outer_call_site"),
        )?,
        one_uuid(
            &db,
            &function_in_module_query(&["crate"], "async_block_call_is_not_outer_call_site"),
        )?,
        one_uuid(
            &db,
            &function_in_module_query(&["crate"], "call_move_closure_literal_with_body_call"),
        )?,
        one_uuid(
            &db,
            &function_in_module_query(&["crate"], "call_async_closure_literal_with_body_call"),
        )?,
    ];

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 128;
    rag.cfg.call_context.max_caller_hits = 1024;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable local_target caller expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming local_target expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&dynamic_owner),
        "local_target expansion should still materialize real dynamic callers; expanded: {expanded:#?}"
    );
    assert!(
        forbidden_owners
            .iter()
            .all(|owner| !expanded_ids.contains(owner)),
        "local_target expansion leaked closure/async body outer owners: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    for owner in forbidden_owners {
        assert!(
            !call_context.contains_key(&owner),
            "closure/async outer owner received RAG call context after target expansion: {call_context:#?}"
        );
    }

    Ok(())
}
