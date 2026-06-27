use super::super::super::super::super::*;
use super::super::super::helpers::*;
use super::expected::{CallCase, ExpectedCall, assert_expected_call, path_call};

#[tokio::test]
async fn call_context_collection_reads_real_field_dynamic_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let dynamic_target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let tuple_target = one_uuid(
        &db,
        &struct_in_module_query(&["crate"], "TupleCallbackArrayHolder"),
    )?;
    let cases = vec![
        dynamic_case(
            &db,
            "named-field function",
            "call_named_field_function_binding",
            dynamic_target,
        )?,
        dynamic_case(
            &db,
            "aliased named-field function",
            "call_aliased_named_field_function_binding",
            dynamic_target,
        )?,
        dynamic_case(
            &db,
            "indexed named-field function",
            "call_indexed_named_field_function_binding",
            dynamic_target,
        )?,
        dynamic_case(
            &db,
            "indexed named-field array alias",
            "call_indexed_named_field_array_alias_binding",
            dynamic_target,
        )?,
        dynamic_case(
            &db,
            "aliased indexed named-field function",
            "call_aliased_indexed_named_field_function_binding",
            dynamic_target,
        )?,
        tuple_case(
            &db,
            "indexed tuple-field function",
            "call_indexed_tuple_field_function_binding",
            tuple_target,
            dynamic_target,
        )?,
        tuple_case(
            &db,
            "indexed tuple-field array alias",
            "call_indexed_tuple_field_array_alias_binding",
            tuple_target,
            dynamic_target,
        )?,
        tuple_case(
            &db,
            "aliased indexed tuple-field function",
            "call_aliased_indexed_tuple_field_function_binding",
            tuple_target,
            dynamic_target,
        )?,
    ];
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable field-dynamic call context"
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
        for expected in &case.calls {
            assert_expected_call(context, expected, case.label);
        }
    }

    Ok(())
}

fn dynamic_case(
    db: &Database,
    label: &'static str,
    owner: &'static str,
    target: Uuid,
) -> Result<CallCase, Error> {
    Ok(CallCase {
        label,
        owner: one_uuid(db, &function_in_module_query(&["crate"], owner))?,
        calls: vec![dynamic_call(target)],
    })
}

fn tuple_case(
    db: &Database,
    label: &'static str,
    owner: &'static str,
    tuple_target: Uuid,
    dynamic_target: Uuid,
) -> Result<CallCase, Error> {
    Ok(CallCase {
        label,
        owner: one_uuid(db, &function_in_module_query(&["crate"], owner))?,
        calls: vec![
            ExpectedCall {
                kind: CallSiteKind::Path,
                callee: path_call(&["TupleCallbackArrayHolder"]),
                target: tuple_target,
                relation: CallTargetKind::TupleStructConstructor,
            },
            dynamic_call(dynamic_target),
        ],
    })
}

fn dynamic_call(target: Uuid) -> ExpectedCall {
    ExpectedCall {
        kind: CallSiteKind::Dynamic,
        callee: CallCalleeInfo::Dynamic,
        target,
        relation: CallTargetKind::DynamicFunction,
    }
}
