use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let method_owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let assoc_owner = function_id_by_name(&db, "call_method_as_associated_function")?;
    let method_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 2, "target-centered method")?;
    let mut expected = assert_proof_method_cases(
        &db,
        &callers,
        &[ProofMethodCase::method(
            method_owner,
            "instance_value",
            &method_receiver,
        )],
    )?;
    expected.extend(assert_proof_site_cases(
        &db,
        &callers,
        &[ProofSiteCase::path(
            assoc_owner,
            &["LocalAssoc", "instance_value"],
            CallRelationKind::AssociatedFunction,
            CallTargetKind::Method,
        )],
    )?);

    assert_target_proof_projection(
        &db,
        "target-centered method",
        "bd:fixture-call-graph",
        target,
        &callers,
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}
