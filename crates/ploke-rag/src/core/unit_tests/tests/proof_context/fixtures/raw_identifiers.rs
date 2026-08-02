use super::super::super::*;
use super::super::helpers::assert_projected_owner_rows;

#[tokio::test]
async fn proof_context_collection_preserves_projected_raw_identifier_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let raw_function_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_raw_identifier_function"),
    )?;
    let raw_method_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_raw_identifier_method"),
    )?;
    let raw_function_target = single_target_for_owner(&db, raw_function_owner)?;
    let raw_method_target = single_target_for_owner(&db, raw_method_owner)?;
    let cases = [
        (
            "raw identifier function",
            raw_function_owner,
            raw_function_target,
        ),
        ("raw identifier method", raw_method_owner, raw_method_target),
    ];

    for (label, owner, _) in cases {
        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(
            count, 3,
            "{label} should project call_site, call_edge, and call_resolution facts"
        );
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected raw identifier proof facts should enable RAG proof context"
    );

    for (label, owner, target) in cases {
        let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
        let rows = proof_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{label} owner seed should receive proof rows"));
        assert_projected_owner_rows(rows, owner, target);
    }

    Ok(())
}

fn single_target_for_owner(db: &Database, owner: Uuid) -> Result<Uuid, Error> {
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "raw identifier owner should have one DB context row: {context:#?}"
    );
    assert_eq!(
        context[0].targets.len(),
        1,
        "raw identifier owner should have one DB target: {context:#?}"
    );
    Ok(context[0].targets[0].target_id)
}
