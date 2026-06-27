use super::super::super::*;
use super::super::helpers::assert_projected_owner_rows;

#[tokio::test]
async fn proof_context_attaches_rows_to_required_expanded_callers() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_typed_local_instance_method"),
    )?;
    let nested_ref_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_typed_double_reference_local_instance_method",
        ),
    )?;
    let assoc_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_method_as_associated_function"),
    )?;
    let method_callers = [
        (owner, "method-call owner"),
        (nested_ref_owner, "nested-reference method owner"),
        (assoc_owner, "method-as-associated-function owner"),
    ];
    for &(owner, label) in &method_callers {
        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(
            count, 3,
            "{label} should project call_site, call_edge, and call_resolution facts"
        );
    }

    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 64;
    cfg.proof_context.max_seed_hits = 1;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.proof_context_degraded(),
        "projected proof facts should enable RAG proof context"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict("instance_value", 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.first().map(|(id, _)| *id),
        Some(target),
        "test query should start from the method target so the caller is expansion-only"
    );

    let assembled = rag
        .get_context(
            "instance_value",
            1,
            &TokenBudget {
                max_total: 4096,
                per_file_max: 4096,
                per_part_max: 1024,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;
    for &(owner, label) in &method_callers {
        let part = assembled
            .parts
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| panic!("call-context expansion should add the {label} part"));
        assert!(
            part.call_expansion.is_some(),
            "{label} should be present because call-context expansion required it"
        );
        assert_projected_owner_rows(&part.proof_context, owner, target);
    }

    Ok(())
}

#[tokio::test]
async fn proof_context_attaches_rows_to_trait_dispatch_callers() -> Result<(), Error> {
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
    let callers = [
        (initialized_owner, "initialized trait-dispatch caller"),
        (chained_owner, "reference-chain trait-object caller"),
    ];
    for &(owner, label) in &callers {
        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(
            count, 3,
            "{label} should project call_site, call_edge, and call_resolution facts"
        );
    }

    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 64;
    cfg.proof_context.max_seed_hits = 5;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.proof_context_degraded(),
        "projected proof facts should enable RAG proof context"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict("144 trait_value", 5, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert!(
        sparse_hits.iter().any(|(id, _)| *id == target),
        "test query should start from the trait-dispatch target; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            "144 trait_value",
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
    for &(owner, label) in &callers {
        let part = assembled
            .parts
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| panic!("call-context expansion should add the {label} part"));
        assert!(
            part.call_expansion.is_some(),
            "{label} should be present because call-context expansion required it"
        );
        assert_projected_owner_rows(&part.proof_context, owner, target);
    }

    Ok(())
}
