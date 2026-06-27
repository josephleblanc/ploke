use super::super::super::super::super::*;
use super::super::super::helpers::*;
use super::expected::{CallCase, ExpectedCall, assert_expected_call, method_call, path, path_call};

#[tokio::test]
async fn call_context_collection_reads_real_result_field_receiver_rows() -> Result<(), Error> {
    init_tracing_once();
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
        CallCase {
            label: "path-call result receiver",
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
        CallCase {
            label: "method-call result receiver",
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
        CallCase {
            label: "await path-call result receiver",
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
        CallCase {
            label: "tuple-field method receiver",
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
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable result/field receiver call context"
    );

    let seeds = cases
        .iter()
        .map(|case| (case.owner, 1.0))
        .collect::<Vec<_>>();
    let call_context = rag.collect_call_context(&seeds)?;

    for case in &cases {
        let context = call_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner should receive outgoing call context", case.label));
        assert_eq!(
            context.len(),
            case.calls.len(),
            "{} owner context: {context:#?}",
            case.label
        );
        for call in &case.calls {
            assert_expected_call(context, call, case.label);
        }
    }

    Ok(())
}
