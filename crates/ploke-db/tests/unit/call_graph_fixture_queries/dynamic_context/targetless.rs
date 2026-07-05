use super::*;

#[test]
fn fixture_context_reads_projected_targetless_dynamic_failures() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        TargetlessDynamicContextCase::unsupported("call_if_closure_branch"),
        TargetlessDynamicContextCase::unsupported("call_match_closure_arm"),
        TargetlessDynamicContextCase::unsupported_path(
            "call_parenthesized_function_pointer_param",
            &["f"],
        ),
        TargetlessDynamicContextCase::unsupported("call_if_function_pointer_param_branch"),
        TargetlessDynamicContextCase::unsupported("call_match_function_pointer_param_arm"),
        TargetlessDynamicContextCase::unsupported("call_dereferenced_closure_binding"),
        TargetlessDynamicContextCase::unsupported_path(
            "call_field_function_param",
            &["holder", "callback"],
        ),
        TargetlessDynamicContextCase::unsupported("call_indexed_function_pointer"),
        TargetlessDynamicContextCase::unsupported("call_indexed_field_function_param"),
        TargetlessDynamicContextCase::unsupported("call_indexed_tuple_field_function_param"),
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
fn fixture_context_reads_projected_returned_closure_dynamic_blocker() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_closure")?;
    let maker = function_id_by_name(&db, "make_closure")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned closure context rows: {context:#?}"
    );

    let maker_row = row_by_path(&context, &["make_closure"]);
    assert_resolved_target(
        maker_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::dynamic(
            Some(&["make_closure"]),
            CallStatusKind::Unsupported,
            "returned closure dynamic call",
        ),
    );

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
