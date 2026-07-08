use super::super::*;

const METHOD_TUPLE_RETURN_PATTERN_LOCAL_INIT_CALL_SPAN: (usize, usize) = (40981, 40999);

#[test]
fn fixture_projection_stores_real_local_receiver_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let pair_target = function_id_by_name(&db, "make_local_assoc_pair")?;
    let tuple_pair_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "tuple_pair")?;
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
            "call_initialized_local_alias_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        )?,
        method_case(
            "call_tuple_pattern_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        )?,
        ResolvedProofCase {
            label: "call_typed_tuple_pattern_local_instance_method",
            owner: function_id_by_name(&db, "call_typed_tuple_pattern_local_instance_method")?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::path(
                    &["make_local_assoc_pair"],
                    pair_target,
                    CallRelationKind::Function,
                    CallTargetKind::Function,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::TypedLocalBinding {
                        name: "value".to_string(),
                        type_path: path(&["LocalAssoc"]),
                    },
                    target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_tuple_return_pattern_local_instance_method",
            owner: function_id_by_name(&db, "call_tuple_return_pattern_local_instance_method")?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::path(
                    &["make_local_assoc_pair"],
                    pair_target,
                    CallRelationKind::Function,
                    CallTargetKind::Function,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::TupleReturnBinding {
                        name: "value".to_string(),
                        path: path(&["make_local_assoc_pair"]),
                        index: 0,
                    },
                    target,
                ),
            ],
        },
        ResolvedProofCase {
            label: "call_method_tuple_return_pattern_local_instance_method",
            owner: function_id_by_name(
                &db,
                "call_method_tuple_return_pattern_local_instance_method",
            )?,
            rows: 2,
            calls: vec![
                ResolvedProofCall::method(
                    "tuple_pair",
                    CallReceiver::InitializedLocalBinding {
                        name: "value".to_string(),
                        init_path: path(&["LocalAssoc"]),
                    },
                    tuple_pair_target,
                ),
                ResolvedProofCall::method(
                    "instance_value",
                    CallReceiver::TupleMethodReturn {
                        name: "next".to_string(),
                        method_name: "tuple_pair".to_string(),
                        method_span: METHOD_TUPLE_RETURN_PATTERN_LOCAL_INIT_CALL_SPAN,
                        index: 0,
                    },
                    target,
                ),
            ],
        },
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
        ResolvedProofCase {
            label: "call_nested_self_field_instance_method",
            owner: method_id_by_impl_self_type_name(
                &db,
                "NestedSelfFieldAssocOwner",
                "call_nested_self_field_instance_method",
            )?,
            rows: 1,
            calls: vec![ResolvedProofCall::method(
                "instance_value",
                CallReceiver::SelfField {
                    path: path(&["inner", "value"]),
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
            "call_if_expression_receiver_method",
            CallReceiver::IfBranchPaths {
                paths: vec![path(&["LocalAssoc"]), path(&["LocalAssoc"])],
            },
        )?,
        method_case(
            "call_match_expression_receiver_method",
            CallReceiver::IfBranchPaths {
                paths: vec![path(&["LocalAssoc"]), path(&["LocalAssoc"])],
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
            "call_borrowed_initialized_local_instance_method",
            CallReceiver::BorrowedInitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
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
            "call_borrowed_value_param_instance_method",
            CallReceiver::BorrowedLocalBinding {
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

#[test]
fn fixture_projection_stores_match_arm_initialized_receiver_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_match_arm_initialized_receiver_method")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["LocalAssoc"]),
    };

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "match-arm initialized receiver proof owner rows: {context:#?}"
    );
    let expected = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some("instance_value")
                && row.site.receiver.as_ref() == Some(&receiver)
        })
        .map(|row| {
            assert_resolved_target(
                row,
                target,
                CallRelationKind::Method,
                CallSiteKind::Method,
                CallTargetKind::Method,
            );
            OwnerProofEdge {
                owner,
                site: row.site.id,
                span: row.site.span,
                target,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected.len(),
        2,
        "match-arm initialized receiver proof rows: {context:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(
        count, 6,
        "two resolved method rows should project six proof facts"
    );
    assert_owner_proof_edges(
        &db,
        "match-arm initialized receiver",
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )
}
