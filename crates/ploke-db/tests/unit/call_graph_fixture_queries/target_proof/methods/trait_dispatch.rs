use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_trait_dispatch_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let initialized_owner = function_id_by_name(&db, "call_initialized_local_trait_method")?;
    let chained_owner =
        function_id_by_name(&db, "call_reference_chain_trait_object_binding_method")?;
    let receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TraitDispatchTarget"]),
    };
    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 3, "target-centered trait dispatch")?;
    let expected = assert_proof_method_cases(
        &db,
        &callers,
        &[
            ProofMethodCase::method(initialized_owner, "trait_value", &receiver),
            ProofMethodCase::method(chained_owner, "trait_value", &receiver),
        ],
    )?;

    assert_target_proof_projection(
        &db,
        "target-centered trait dispatch",
        "bd:fixture-call-graph",
        target,
        &callers,
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}
