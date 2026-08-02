use super::super::super::*;
#[tokio::test]
async fn call_context_sparse_get_context_expands_associated_function_target_hits_to_fixture_callers()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &trait_method_query("LocalAssocFunctionTrait", "trait_make"),
    )?;
    let caller_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_trait_associated_function"),
    )?;
    let query = "233 trait_make";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public associated-function call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the associated-function target only"
    );
    assert_eq!(
        sparse_hits[0].0, target,
        "test query should seed get_context with the LocalAssocFunctionTrait::trait_make target only; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 20_000,
                per_file_max: 20_000,
                per_part_max: 4_096,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    let caller_part = assembled
        .parts
        .iter()
        .find(|part| part.id == caller_owner)
        .expect("public get_context should materialize the trait associated-function caller owner");
    let call = caller_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: vec![
                                "LocalAssocFunctionTrait".to_string(),
                                "trait_make".to_string(),
                            ],
                        }
            })
            .expect("caller part should retain outgoing trait associated-function context to the seed target");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::AssociatedFunction);
    assert_incoming_expansion(caller_part, call, target);

    Ok(())
}
#[tokio::test]
async fn call_context_sparse_get_context_expands_imported_trait_associated_function_target_hits_to_fixture_callers()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &trait_method_query("ImportedAssocFunctionTrait", "imported_trait_make"),
    )?;
    let callers = [
        (
            &["crate", "trait_assoc_function_scope", "with_direct_import"][..],
            "call_direct_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_alias_import"][..],
            "call_alias_imported_trait_associated_function",
            &["VisibleAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_glob_import"][..],
            "call_glob_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_reexport_scope"][..],
            "call_reexported_trait_associated_function",
            &["ReexportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "grouped_trait_assoc_function_scope"][..],
            "call_grouped_imported_trait_associated_function",
            &["GroupedAssocFunctionTrait", "imported_trait_make"][..],
        ),
    ];
    let caller_owners = callers
        .iter()
        .map(|(module_path, name, expected_path)| {
            Ok((
                one_uuid(&db, &function_in_module_query(module_path, name))?,
                *expected_path,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let query = "987 imported_trait_make";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public imported trait associated-function call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the imported trait associated-function target only"
    );
    assert_eq!(
        sparse_hits[0].0, target,
        "test query should seed get_context with the ImportedAssocFunctionTrait::imported_trait_make target only; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 24_000,
                per_file_max: 24_000,
                per_part_max: 4_096,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    for (caller_owner, expected_path) in caller_owners {
        let caller_part = assembled
                .parts
                .iter()
                .find(|part| part.id == caller_owner)
                .expect(
                    "public get_context should materialize the imported trait associated-function caller owner",
                );
        let call = caller_part
                .call_context
                .iter()
                .find(|call| {
                    call.kind == CallSiteKind::Path
                        && call.callee
                            == CallCalleeInfo::Path {
                                path: expected_path
                                    .iter()
                                    .map(|segment| (*segment).to_string())
                                    .collect(),
                            }
                })
                .expect("caller part should retain outgoing imported trait associated-function context to the seed target");
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::AssociatedFunction);
        assert_incoming_expansion(caller_part, call, target);
    }

    Ok(())
}
