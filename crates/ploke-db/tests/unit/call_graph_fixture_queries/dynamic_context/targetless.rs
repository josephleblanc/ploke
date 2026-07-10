use super::*;

#[test]
fn fixture_context_reads_projected_targetless_dynamic_failures() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        TargetlessDynamicContextCase::unsupported_path(
            "call_parenthesized_function_pointer_param",
            &["f"],
        ),
        TargetlessDynamicContextCase::unsupported_path(
            "call_if_function_pointer_param_branch",
            &["f"],
        ),
        TargetlessDynamicContextCase::unsupported_path(
            "call_match_function_pointer_param_arm",
            &["f"],
        ),
        TargetlessDynamicContextCase::unsupported_path(
            "call_field_function_param",
            &["holder", "callback"],
        ),
        TargetlessDynamicContextCase::unsupported_path(
            "call_indexed_function_pointer",
            &["funcs", "0"],
        ),
        TargetlessDynamicContextCase::unsupported_path(
            "call_indexed_field_function_param",
            &["holder", "callbacks", "0"],
        ),
        TargetlessDynamicContextCase::unsupported_path(
            "call_indexed_tuple_field_function_param",
            &["holder", "0", "0"],
        ),
        TargetlessDynamicContextCase::unsupported("call_async_closure_literal_with_body_call"),
        TargetlessDynamicContextCase::unsupported_path(
            "call_parenthesized_generic_fn_once_value_binding",
            &["generic_f"],
        ),
    ];

    assert_targetless_dynamic_context_cases(&db, &cases)?;

    Ok(())
}

#[test]
fn fixture_context_reads_projected_callable_value_path_failures_and_vec_external()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    assert_targetless_owner_cases(
        &db,
        &[
            TargetlessOwnerCase {
                owner: "call_function_pointer_param",
                rows: &[TargetlessRowCase::path(
                    &["f"],
                    0,
                    CallStatusKind::Unsupported,
                    "call_function_pointer_param",
                )],
            },
            TargetlessOwnerCase {
                owner: "call_generic_fn_once_value_binding",
                rows: &[TargetlessRowCase::path(
                    &["generic_f"],
                    0,
                    CallStatusKind::Unsupported,
                    "call_generic_fn_once_value_binding",
                )],
            },
            TargetlessOwnerCase {
                owner: "call_prelude_vec_new",
                rows: &[TargetlessRowCase::path(
                    &["Vec", "new"],
                    0,
                    CallStatusKind::External,
                    "Vec::new external row",
                )],
            },
        ],
    )?;

    Ok(())
}

#[test]
fn fixture_context_reads_projected_conflicting_callable_value_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;

    for (owner_name, expected_path) in [
        ("call_multi_conflicting_function_pointer_param", &["f"][..]),
        (
            "call_multi_conflicting_generic_fn_once_param",
            &["generic_f"][..],
        ),
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        assert_path_function_candidates(&context[0], owner, expected_path, &expected, owner_name);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_conflicting_callable_field_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let owner_name = "call_multi_conflicting_named_field_function_param";
    let owner = function_id_by_name(&db, owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
    assert_dynamic_path_function_candidates(
        &context[0],
        owner,
        &["holder", "callback"],
        &expected,
        owner_name,
    );

    Ok(())
}
