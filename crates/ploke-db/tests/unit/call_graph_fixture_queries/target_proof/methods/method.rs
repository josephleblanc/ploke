use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let method_owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let borrowed_owner = function_id_by_name(&db, "call_borrowed_value_param_instance_method")?;
    let borrowed_result_owner = function_id_by_name(
        &db,
        "call_borrowed_value_param_method_result_instance_method",
    )?;
    let self_field_owner = method_id_by_impl_self_type_name(
        &db,
        "SelfFieldAssocOwner",
        "call_self_field_instance_method",
    )?;
    let nested_self_field_owner = method_id_by_impl_self_type_name(
        &db,
        "NestedSelfFieldAssocOwner",
        "call_nested_self_field_instance_method",
    )?;
    let assoc_owner = function_id_by_name(&db, "call_method_as_associated_function")?;
    let method_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let borrowed_receiver = CallReceiver::BorrowedLocalBinding {
        name: "value".to_string(),
    };
    let method_result_receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let self_field_receiver = CallReceiver::SelfField {
        path: path(&["value"]),
    };
    let nested_self_field_receiver = CallReceiver::SelfField {
        path: path(&["inner", "value"]),
    };
    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 7, "target-centered method")?;
    let mut expected = assert_proof_method_cases(
        &db,
        &callers,
        &[
            ProofMethodCase::method(method_owner, "instance_value", &method_receiver),
            ProofMethodCase::method(borrowed_owner, "instance_value", &borrowed_receiver),
            ProofMethodCase::method(
                borrowed_result_owner,
                "instance_value",
                &method_result_receiver,
            ),
            ProofMethodCase::method(self_field_owner, "instance_value", &self_field_receiver),
            ProofMethodCase::method(
                nested_self_field_owner,
                "instance_value",
                &nested_self_field_receiver,
            ),
        ],
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
