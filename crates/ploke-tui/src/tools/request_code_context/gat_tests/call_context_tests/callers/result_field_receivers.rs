use super::super::assertions::{assert_expansion, assert_resolved_target, path};
use super::super::*;

struct ExpectedCall {
    kind: CallSiteKind,
    callee: CallCalleeInfo,
    target: Uuid,
    relation: CallTargetKind,
}

struct Case {
    label: &'static str,
    search_term: &'static str,
    call_id: &'static str,
    owner: Uuid,
    calls: Vec<ExpectedCall>,
}

#[tokio::test]
async fn request_code_context_returns_result_field_receiver_call_context() -> color_eyre::Result<()>
{
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let method_target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let clone_target = one_uuid(&db, &method_by_impl_self_query("LocalAssoc", "clone_assoc"))?;
    let make_target = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_local_assoc"),
    )?;
    let ready_target = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_ready_local_assoc"),
    )?;
    let tuple_target = one_uuid(
        &db,
        &struct_in_module_query(&["crate"], "TupleFieldMethodReceiver"),
    )?;
    let cases = vec![
        Case {
            label: "path-call result receiver",
            search_term: "call_path_result_instance_method",
            call_id: "path_result_receiver_call_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_path_result_instance_method"),
            )?,
            calls: vec![
                ExpectedCall {
                    kind: CallSiteKind::Path,
                    callee: path_call(&["make_local_assoc"]),
                    target: make_target,
                    relation: CallTargetKind::Function,
                },
                ExpectedCall {
                    kind: CallSiteKind::Method,
                    callee: method_call(
                        "instance_value",
                        CallReceiverInfo::PathCallResult {
                            path: path(&["make_local_assoc"]),
                        },
                    ),
                    target: method_target,
                    relation: CallTargetKind::Method,
                },
            ],
        },
        Case {
            label: "method-call result receiver",
            search_term: "call_method_result_instance_method",
            call_id: "method_result_receiver_call_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_method_result_instance_method"),
            )?,
            calls: vec![
                ExpectedCall {
                    kind: CallSiteKind::Method,
                    callee: method_call(
                        "clone_assoc",
                        CallReceiverInfo::TypedLocalBinding {
                            name: "value".to_string(),
                            type_path: path(&["LocalAssoc"]),
                        },
                    ),
                    target: clone_target,
                    relation: CallTargetKind::Method,
                },
                ExpectedCall {
                    kind: CallSiteKind::Method,
                    callee: method_call(
                        "instance_value",
                        CallReceiverInfo::MethodCallResult {
                            method_name: "clone_assoc".to_string(),
                        },
                    ),
                    target: method_target,
                    relation: CallTargetKind::Method,
                },
            ],
        },
        Case {
            label: "await path-call result receiver",
            search_term: "call_await_result_instance_method",
            call_id: "await_result_receiver_call_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_await_result_instance_method"),
            )?,
            calls: vec![
                ExpectedCall {
                    kind: CallSiteKind::Path,
                    callee: path_call(&["make_ready_local_assoc"]),
                    target: ready_target,
                    relation: CallTargetKind::Function,
                },
                ExpectedCall {
                    kind: CallSiteKind::Method,
                    callee: method_call(
                        "instance_value",
                        CallReceiverInfo::AwaitPathCallResult {
                            path: path(&["make_ready_local_assoc"]),
                        },
                    ),
                    target: method_target,
                    relation: CallTargetKind::Method,
                },
            ],
        },
        Case {
            label: "tuple-field method receiver",
            search_term: "call_tuple_field_instance_method",
            call_id: "tuple_field_receiver_call_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_tuple_field_instance_method"),
            )?,
            calls: vec![
                ExpectedCall {
                    kind: CallSiteKind::Path,
                    callee: path_call(&["TupleFieldMethodReceiver"]),
                    target: tuple_target,
                    relation: CallTargetKind::TupleStructConstructor,
                },
                ExpectedCall {
                    kind: CallSiteKind::Method,
                    callee: method_call(
                        "instance_value",
                        CallReceiverInfo::FieldInitializedLocalBinding {
                            name: "value".to_string(),
                            init_path: path(&["TupleFieldMethodReceiver"]),
                            field_path: path(&["0"]),
                        },
                    ),
                    target: method_target,
                    relation: CallTargetKind::Method,
                },
            ],
        },
    ];

    for case in cases {
        let result = execute_fixture_request(&db, case.search_term, 1, case.call_id).await?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");

        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == case.owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} owner",
                    case.label
                )
            });
        assert_eq!(
            owner_part.call_context.len(),
            case.calls.len(),
            "{} call context: {owner_part:#?}",
            case.label
        );
        for expected in &case.calls {
            let target_part = result
                .context
                .iter()
                .find(|part| part.id == expected.target)
                .unwrap_or_else(|| {
                    panic!(
                        "request_code_context should materialize the {} target",
                        case.label
                    )
                });
            let call = owner_part
                .call_context
                .iter()
                .find(|call| {
                    call.kind == expected.kind
                        && call.callee == expected.callee
                        && call
                            .targets
                            .iter()
                            .any(|target| target.target_id == expected.target)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{} owner should retain expected result/field call context",
                        case.label
                    )
                });
            assert_resolved_target(call, expected.target, expected.relation.clone());
            assert_expansion(
                target_part,
                case.owner,
                expected.target,
                call.site_id,
                CallExpansionKind::OutgoingTarget,
            );
        }
    }

    Ok(())
}

fn path_call(segments: &[&str]) -> CallCalleeInfo {
    CallCalleeInfo::Path {
        path: path(segments),
    }
}

fn method_call(name: &str, receiver: CallReceiverInfo) -> CallCalleeInfo {
    CallCalleeInfo::Method {
        name: name.to_string(),
        receiver: Some(receiver),
    }
}
