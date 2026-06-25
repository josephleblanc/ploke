use super::super::*;

#[test]
fn fixture_projection_stores_real_local_receiver_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let method_case =
        |label: &'static str, receiver| -> Result<ResolvedProofCase<'static>, DbError> {
            Ok(ResolvedProofCase {
                label,
                owner: function_id_by_name(&db, label)?,
                rows: 1,
                calls: vec![ResolvedProofCall::method(
                    "instance_value",
                    receiver,
                    target,
                )],
            })
        };
    let cases = [
        method_case(
            "call_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        )?,
        method_case(
            "call_initialized_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        )?,
        method_case(
            "call_parenthesized_typed_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        )?,
        method_case(
            "call_type_alias_chain_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssocAliasChain"]),
            },
        )?,
        method_case(
            "call_imported_type_alias_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["ImportedLocalAssocAlias"]),
            },
        )?,
        method_case(
            "call_borrowed_typed_local_instance_method",
            CallReceiver::BorrowedTypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        )?,
        method_case(
            "call_dereferenced_local_instance_method",
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        )?,
        method_case(
            "call_dereferenced_param_instance_method",
            CallReceiver::DereferencedLocalBinding {
                name: "value".to_string(),
            },
        )?,
        method_case(
            "call_borrowed_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        )?,
        method_case(
            "call_referenced_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        )?,
        method_case(
            "call_typed_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        )?,
    ];
    let expected_edges = resolved_proof_edges(&db, "bd:fixture-call-graph", &cases)?;

    assert_owner_proof_edges(
        &db,
        "local receiver method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
