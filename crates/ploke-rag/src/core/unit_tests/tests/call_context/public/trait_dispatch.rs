use super::super::super::*;

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_trait_dispatch_target_hits_to_fixture_callers()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &method_by_impl_trait_self_query(
            "LocalDispatchTrait",
            "TraitDispatchTarget",
            "trait_value",
        ),
    )?;
    let initialized_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_initialized_local_trait_method"),
    )?;
    let chained_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_reference_chain_trait_object_binding_method",
        ),
    )?;
    let query = "144 trait_value";
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
        "fresh fixture call_graph schema should enable public trait-dispatch call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 5, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert!(
        sparse_hits.iter().any(|(id, _)| *id == target),
        "test query should seed get_context with the concrete trait-dispatch method target; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            5,
            &TokenBudget {
                max_total: 20_000,
                per_file_max: 20_000,
                per_part_max: 4_096,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    for owner in [initialized_owner, chained_owner] {
        let caller_part = assembled
            .parts
            .iter()
            .find(|part| part.id == owner)
            .expect("public get_context should materialize the trait-dispatch caller owner");
        let call = caller_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Method
                    && call.callee
                        == CallCalleeInfo::Method {
                            name: "trait_value".to_string(),
                            receiver: Some(CallReceiverInfo::InitializedLocalBinding {
                                name: "value".to_string(),
                                init_path: vec!["TraitDispatchTarget".to_string()],
                            }),
                        }
            })
            .expect("caller part should retain outgoing trait-dispatch context to the seed target");
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Method);
        assert_incoming_expansion(caller_part, call, target);
    }

    Ok(())
}
