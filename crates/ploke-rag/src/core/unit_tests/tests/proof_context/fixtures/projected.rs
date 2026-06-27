use super::super::super::*;
use super::super::helpers::assert_projected_owner_rows;

#[tokio::test]
async fn proof_context_sparse_get_context_attaches_projected_owner_facts() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3, "single resolved path call should project 3 facts");

    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.enabled = false;
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

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .expect("owner seed should receive linked proof rows");
    assert_projected_owner_rows(rows, owner, target);

    let query = "call_crate_local_target";
    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the caller owner only"
    );
    assert_eq!(
        sparse_hits[0].0, owner,
        "test query should seed get_context with the caller owner only"
    );

    let assembled = rag
        .get_context(
            query,
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
    let owner_part = assembled
        .parts
        .iter()
        .find(|part| part.id == owner)
        .expect("public get_context should preserve the caller owner seed");
    assert_projected_owner_rows(&owner_part.proof_context, owner, target);

    Ok(())
}
