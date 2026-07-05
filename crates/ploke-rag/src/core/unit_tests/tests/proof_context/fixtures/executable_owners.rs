use super::super::super::*;
use super::super::helpers::assert_projected_owner_rows;

#[tokio::test]
async fn proof_context_collection_preserves_closure_owner_projected_rows() -> Result<(), Error> {
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

    assert_eq!(
        db.project_call_proof_facts_for_owner(closure, "bd:fixture-call-graph")?,
        3,
        "closure executable owner should project call_site, call_edge, and call_resolution facts"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "closure owner proof projection should enable RAG proof context"
    );

    let proof_context = rag.collect_proof_context(&[(closure, 1.0)])?;
    let rows = proof_context
        .get(&closure)
        .expect("closure executable owner seed should receive linked proof rows");
    assert_projected_owner_rows(rows, closure, target);

    Ok(())
}

#[tokio::test]
async fn proof_context_collection_preserves_async_closure_owner_projected_rows() -> Result<(), Error>
{
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

    assert_eq!(
        db.project_call_proof_facts_for_owner(async_closure, "bd:fixture-call-graph")?,
        3,
        "async closure executable owner should project call_site, call_edge, and call_resolution facts"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "async closure owner proof projection should enable RAG proof context"
    );

    let proof_context = rag.collect_proof_context(&[(async_closure, 1.0)])?;
    let rows = proof_context
        .get(&async_closure)
        .expect("async closure executable owner seed should receive linked proof rows");
    assert_projected_owner_rows(rows, async_closure, target);

    Ok(())
}
