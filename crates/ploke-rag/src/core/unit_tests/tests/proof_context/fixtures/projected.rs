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

#[tokio::test]
async fn proof_context_collection_preserves_projected_path_resolution_family() -> Result<(), Error>
{
    struct Case {
        label: &'static str,
        owner: Uuid,
        target: Uuid,
    }

    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let nested_target = one_uuid(
        &db,
        &function_in_module_query(&["crate", "local_mod"], "nested_target"),
    )?;
    let imported_target = one_uuid(
        &db,
        &function_in_module_query(&["crate", "import_targets"], "imported_target"),
    )?;
    let globbed_target = one_uuid(
        &db,
        &function_in_module_query(&["crate", "import_targets"], "globbed_target"),
    )?;
    let cases = [
        Case {
            label: "self nested path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate", "local_mod"], "call_self_nested_target"),
            )?,
            target: nested_target,
        },
        Case {
            label: "imported alias path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_imported_alias_target"),
            )?,
            target: imported_target,
        },
        Case {
            label: "glob imported path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_glob_imported_target"),
            )?,
            target: globbed_target,
        },
        Case {
            label: "re-exported path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_reexported_target"),
            )?,
            target: imported_target,
        },
        Case {
            label: "module alias path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_imported_module_target"),
            )?,
            target: globbed_target,
        },
        Case {
            label: "grouped imported alias path",
            owner: one_uuid(
                &db,
                &function_in_module_query(
                    &["crate", "grouped_function_import_scope"],
                    "call_grouped_imported_alias_target",
                ),
            )?,
            target: imported_target,
        },
    ];

    for case in &cases {
        let count = db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?;
        assert_eq!(
            count, 3,
            "{} should project call_site, call_edge, and call_resolution facts",
            case.label
        );
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected path-family proof facts should enable RAG proof context"
    );

    for case in &cases {
        let proof_context = rag.collect_proof_context(&[(case.owner, 1.0)])?;
        let rows = proof_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner seed should receive proof rows", case.label));
        assert_projected_owner_rows(rows, case.owner, case.target);
    }

    Ok(())
}
