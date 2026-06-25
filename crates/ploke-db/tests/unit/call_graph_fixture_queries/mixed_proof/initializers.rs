use super::*;

#[test]
fn fixture_projection_stores_real_const_and_static_initializer_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let cases = const_static_cases(&db)?;
    assert_initializer_proofs(
        &db,
        "const/static initializer",
        "bd:fixture-nodes",
        &cases,
        "fixture_nodes/src/const_static.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_associated_const_initializer_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = assoc_const_cases(&db)?;
    assert_initializer_proofs(
        &db,
        "associated const initializer",
        "bd:fixture-call-graph",
        &cases,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}
