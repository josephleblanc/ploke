use super::super::super::*;
#[tokio::test]
async fn call_context_sparse_get_context_excludes_closure_async_outer_owners_for_local_target()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let path_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
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
    let query = "pub fn local_target";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 1024;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public local_target expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with local_target only"
    );
    assert_eq!(
        sparse_hits[0].0, target,
        "test query should seed get_context with local_target only; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 65_536,
                per_file_max: 65_536,
                per_part_max: 4096,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    let path_part = assembled
        .parts
        .iter()
        .find(|part| part.id == path_owner)
        .expect("public get_context should materialize the ordinary local_target path caller");
    let path_call = path_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["crate".to_string(), "local_target".to_string()],
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("path caller part should retain outgoing call context to local_target");
    assert_eq!(path_call.status, CallStatusKind::Resolved);
    assert_eq!(path_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path_call.targets.len(), 1);
    assert_eq!(path_call.targets[0].target_id, target);
    assert_eq!(path_call.targets[0].relation, CallTargetKind::Function);
    assert_incoming_expansion(path_part, path_call, target);

    let dynamic_part = assembled
        .parts
        .iter()
        .find(|part| part.id == dynamic_owner)
        .expect("public get_context should materialize a real dynamic local_target caller");
    let dynamic_call = dynamic_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("dynamic caller part should retain outgoing call context to local_target");
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
    assert_incoming_expansion(dynamic_part, dynamic_call, target);

    for forbidden in forbidden_owners {
        assert!(
            assembled.parts.iter().all(|part| part.id != forbidden),
            "public local_target expansion leaked closure/async outer owner {forbidden}: {:#?}",
            assembled.parts
        );
    }

    Ok(())
}
