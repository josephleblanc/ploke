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

    assert_targetless_owner_cases(
        &db,
        &[TargetlessOwnerCase {
            owner: "call_parenthesized_boxed_dyn_fn_value_binding",
            rows: &[
                TargetlessRowCase::path(
                    &["Box", "new"],
                    1,
                    CallStatusKind::External,
                    "parenthesized Box::new setup call",
                ),
                TargetlessRowCase::dynamic(
                    Some(&["boxed_fn"]),
                    CallStatusKind::Unsupported,
                    "parenthesized boxed dyn Fn dynamic call",
                ),
            ],
        }],
    )?;

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
                owner: "call_boxed_dyn_fn_value_binding",
                rows: &[
                    TargetlessRowCase::path(
                        &["Box", "new"],
                        1,
                        CallStatusKind::External,
                        "Box::new setup call",
                    ),
                    TargetlessRowCase::path(
                        &["boxed_fn"],
                        0,
                        CallStatusKind::Unsupported,
                        "boxed dyn Fn path call",
                    ),
                ],
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
