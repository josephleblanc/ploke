use super::super::*;

#[test]
fn fixture_projection_stores_real_result_and_field_receiver_method_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let try_clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "try_clone_assoc")?;
    let try_instance_target =
        method_id_by_impl_self_type_name(&db, "LocalAssoc", "try_instance_value")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let tuple_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let cases = [
        ResolvedProofCase {
            label: "call_path_result_instance_method",
            owner: function_id_by_name(&db, "call_path_result_instance_method")?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::path(
                    &["make_local_assoc"],
                    make_target,
                    CallRelationKind::Function,
                    CallTargetKind::Function,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::PathCallResult {
                        path: path(&["make_local_assoc"]),
                    },
                    method_target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_method_result_instance_method",
            owner: function_id_by_name(&db, "call_method_result_instance_method")?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::method(
                    "clone_assoc",
                    CallReceiver::TypedLocalBinding {
                        name: "value".to_string(),
                        type_path: path(&["LocalAssoc"]),
                    },
                    clone_target,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::MethodCallResult {
                        method_name: "clone_assoc".to_string(),
                    },
                    method_target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_method_result_binding_instance_method",
            owner: function_id_by_name(&db, "call_method_result_binding_instance_method")?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::method(
                    "clone_assoc",
                    CallReceiver::TypedLocalBinding {
                        name: "value".to_string(),
                        type_path: path(&["LocalAssoc"]),
                    },
                    clone_target,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::MethodResultLocalBinding {
                        name: "cloned".to_string(),
                        method_name: "clone_assoc".to_string(),
                        method_span: (49201, 49220),
                    },
                    method_target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_borrowed_value_param_method_result_instance_method",
            owner: function_id_by_name(
                &db,
                "call_borrowed_value_param_method_result_instance_method",
            )?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::method(
                    "clone_assoc",
                    CallReceiver::BorrowedLocalBinding {
                        name: "value".to_string(),
                    },
                    clone_target,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::MethodCallResult {
                        method_name: "clone_assoc".to_string(),
                    },
                    method_target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_self_field_method_result_instance_method",
            owner: method_id_by_impl_self_type_name(
                &db,
                "SelfFieldAssocOwner",
                "call_self_field_method_result_instance_method",
            )?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::method(
                    "clone_assoc",
                    CallReceiver::SelfField {
                        path: path(&["value"]),
                    },
                    clone_target,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::MethodCallResult {
                        method_name: "clone_assoc".to_string(),
                    },
                    method_target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_await_result_instance_method",
            owner: function_id_by_name(&db, "call_await_result_instance_method")?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::path(
                    &["make_ready_local_assoc"],
                    ready_target,
                    CallRelationKind::Function,
                    CallTargetKind::Function,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::AwaitPathCallResult {
                        path: path(&["make_ready_local_assoc"]),
                    },
                    method_target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_try_method_result_instance_method",
            owner: function_id_by_name(&db, "call_try_method_result_instance_method")?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::method(
                    "try_clone_assoc",
                    CallReceiver::TypedLocalBinding {
                        name: "value".to_string(),
                        type_path: path(&["LocalAssoc"]),
                    },
                    try_clone_target,
                ),
                ResolvedProofCall::method(
                    "try_instance_value",
                    CallReceiver::TryMethodCallResult {
                        method_name: "try_clone_assoc".to_string(),
                    },
                    try_instance_target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_tuple_field_instance_method",
            owner: function_id_by_name(&db, "call_tuple_field_instance_method")?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::path(
                    &["TupleFieldMethodReceiver"],
                    tuple_target,
                    CallRelationKind::TupleStructConstructor,
                    CallTargetKind::Struct,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::FieldInitializedLocalBinding {
                        name: "value".to_string(),
                        init_path: path(&["TupleFieldMethodReceiver"]),
                        field_path: path(&["0"]),
                    },
                    method_target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_param_field_instance_method",
            owner: function_id_by_name(&db, "call_param_field_instance_method")?,
            rows: 1,
            calls: vec![ResolvedProofCall::method(
                "instance_value",
                CallReceiver::FieldLocalBinding {
                    name: "holder".to_string(),
                    field_path: path(&["value"]),
                },
                method_target,
            )],
        },
    ];
    assert_fixture_resolved_proofs(&db, "result/field receiver method", &cases)?;

    Ok(())
}
