use super::*;

#[test]
fn fixture_context_reads_projected_targetless_dynamic_failures() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        TargetlessDynamicContextCase {
            owner: "call_match_guarded_function_item",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_if_closure_branch",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_match_closure_arm",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_parenthesized_function_pointer_param",
            path: Some(&["f"]),
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_if_function_pointer_param_branch",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_match_function_pointer_param_arm",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_if_nested_branch_expression",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_match_nested_arm_expression",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_closure_binding_cast",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_dereferenced_closure_binding",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_field_function_param",
            path: Some(&["holder", "callback"]),
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_indexed_function_pointer",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_indexed_field_function_param",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_indexed_tuple_field_function_param",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_move_closure_literal_with_body_call",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_async_closure_literal_with_body_call",
            path: None,
            status: CallStatusKind::Unsupported,
        },
        TargetlessDynamicContextCase {
            owner: "call_parenthesized_generic_fn_once_value_binding",
            path: Some(&["generic_f"]),
            status: CallStatusKind::Unsupported,
        },
    ];

    assert_targetless_dynamic_context_cases(&db, &cases)?;

    let owner = function_id_by_name(&db, "call_parenthesized_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "boxed dyn Fn context rows: {context:#?}");

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["Box", "new"],
            1,
            CallStatusKind::External,
            "parenthesized Box::new setup call",
        ),
    );

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::dynamic(
            Some(&["boxed_fn"]),
            CallStatusKind::Unsupported,
            "parenthesized boxed dyn Fn dynamic call",
        ),
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_callable_value_path_failures_and_vec_external()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let path_failures = [
        ("call_function_pointer_param", &["f"][..]),
        ("call_generic_fn_once_value_binding", &["generic_f"][..]),
    ];

    for (owner_name, expected_path) in path_failures {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        assert_targetless_row(
            &context,
            owner,
            TargetlessRowCase::path(expected_path, 0, CallStatusKind::Unsupported, owner_name),
        );
    }

    let owner = function_id_by_name(&db, "call_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "boxed dyn Fn path context rows: {context:#?}"
    );

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["Box", "new"],
            1,
            CallStatusKind::External,
            "Box::new setup call",
        ),
    );

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["boxed_fn"],
            0,
            CallStatusKind::Unsupported,
            "boxed dyn Fn path call",
        ),
    );

    let owner = function_id_by_name(&db, "call_prelude_vec_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "Vec::new context rows: {context:#?}");
    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["Vec", "new"],
            0,
            CallStatusKind::External,
            "Vec::new external row",
        ),
    );

    Ok(())
}
