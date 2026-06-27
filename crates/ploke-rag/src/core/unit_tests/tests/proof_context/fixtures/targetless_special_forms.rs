use super::super::super::*;
use super::super::helpers::{assert_blocked_resolution, assert_resolved_call};

#[tokio::test]
async fn proof_context_collection_preserves_targetless_special_form_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let extern_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_extern_c_function"),
    )?;
    let chained_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_chained_returned_function"),
    )?;
    let chained_target = one_uuid(&db, &function_in_module_query(&["crate"], "make_unary_fn"))?;

    assert_eq!(
        db.project_call_proof_facts_for_owner(extern_owner, "bd:fixture-call-graph")?,
        2,
        "extern C targetless call should project call_site and call_resolution facts"
    );
    assert_eq!(
        db.project_call_proof_facts_for_owner(chained_owner, "bd:fixture-call-graph")?,
        5,
        "chained returned-function call should project resolved and blocked proof facts"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected targetless special-form proof facts should enable RAG proof context"
    );

    let extern_context = rag.collect_proof_context(&[(extern_owner, 1.0)])?;
    let extern_rows = extern_context
        .get(&extern_owner)
        .expect("extern C owner seed should receive proof rows");
    assert_eq!(
        extern_rows.len(),
        2,
        "extern C proof rows: {extern_rows:#?}"
    );
    assert_blocked_resolution(
        extern_rows,
        extern_owner,
        "external_dependency_summary_missing",
    );

    let chained_context = rag.collect_proof_context(&[(chained_owner, 1.0)])?;
    let chained_rows = chained_context
        .get(&chained_owner)
        .expect("chained returned-function owner seed should receive proof rows");
    assert_eq!(
        chained_rows.len(),
        5,
        "chained returned-function proof rows: {chained_rows:#?}"
    );
    assert_resolved_call(chained_rows, chained_owner, chained_target);
    assert_blocked_resolution(chained_rows, chained_owner, "dynamic_dispatch_unbounded");

    Ok(())
}
