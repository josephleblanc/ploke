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
        ResolvedProofCase {
            label: "call_self_field_instance_method",
            owner: method_id_by_impl_self_type_name(
                &db,
                "SelfFieldAssocOwner",
                "call_self_field_instance_method",
            )?,
            rows: 1,
            calls: vec![ResolvedProofCall::method(
                "instance_value",
                CallReceiver::SelfField {
                    path: path(&["value"]),
                },
                target,
            )],
        },
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
        method_case(
            "call_typed_double_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        )?,
    ];
    assert_fixture_resolved_proofs(&db, "local receiver method", &cases)?;

    Ok(())
}
