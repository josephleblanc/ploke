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
    let qself_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_qualified_dyn_any_downcast_mut"),
    )?;
    let chained_target = one_uuid(&db, &function_in_module_query(&["crate"], "make_unary_fn"))?;
    let returned_target = one_uuid(&db, &function_in_module_query(&["crate"], "unary_target"))?;

    assert_eq!(
        db.project_call_proof_facts_for_owner(extern_owner, "bd:fixture-call-graph")?,
        2,
        "extern C targetless call should project call_site and call_resolution facts"
    );
    assert_eq!(
        db.project_call_proof_facts_for_owner(chained_owner, "bd:fixture-call-graph")?,
        6,
        "chained returned-function call should project inner and outer resolved proof facts"
    );
    assert_eq!(
        db.project_call_proof_facts_for_owner(qself_owner, "bd:fixture-call-graph")?,
        2,
        "qualified dyn Any targetless call should project call_site and call_resolution facts"
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
        6,
        "chained returned-function proof rows: {chained_rows:#?}"
    );
    assert_resolved_call(chained_rows, chained_owner, chained_target);
    assert_resolved_call(chained_rows, chained_owner, returned_target);

    let qself_context = rag.collect_proof_context(&[(qself_owner, 1.0)])?;
    let qself_rows = qself_context
        .get(&qself_owner)
        .expect("qualified dyn Any owner seed should receive proof rows");
    assert_eq!(
        qself_rows.len(),
        2,
        "qualified dyn Any proof rows: {qself_rows:#?}"
    );
    assert_blocked_resolution(
        qself_rows,
        qself_owner,
        "external_dependency_summary_missing",
    );

    Ok(())
}
