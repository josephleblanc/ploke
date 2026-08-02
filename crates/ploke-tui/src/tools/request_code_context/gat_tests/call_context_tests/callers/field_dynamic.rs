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
async fn request_code_context_returns_field_dynamic_call_context() -> color_eyre::Result<()> {
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
                    panic!("{} owner should retain field-dynamic context", case.label)
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

fn dynamic_case(
    db: &Database,
    label: &'static str,
    owner: &'static str,
    target: Uuid,
) -> color_eyre::Result<Case> {
    Ok(Case {
        label,
        search_term: owner,
        call_id: "field_dynamic_call_context",
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
) -> color_eyre::Result<Case> {
    Ok(Case {
        label,
        search_term: owner,
        call_id: "field_dynamic_tuple_call_context",
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

fn path_call(segments: &[&str]) -> CallCalleeInfo {
    CallCalleeInfo::Path {
        path: path(segments),
    }
}
