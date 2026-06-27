use super::*;

#[test]
fn fixture_context_reads_projected_targetless_dynamic_failures() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        TargetlessDynamicContextCase::unsupported("call_match_guarded_function_item"),
        TargetlessDynamicContextCase::unsupported("call_if_closure_branch"),
        TargetlessDynamicContextCase::unsupported("call_match_closure_arm"),
        TargetlessDynamicContextCase::unsupported_path(
            "call_parenthesized_function_pointer_param",
            &["f"],
        ),
        TargetlessDynamicContextCase::unsupported("call_if_function_pointer_param_branch"),
        TargetlessDynamicContextCase::unsupported("call_match_function_pointer_param_arm"),
        TargetlessDynamicContextCase::unsupported("call_closure_binding_cast"),
        TargetlessDynamicContextCase::unsupported("call_dereferenced_closure_binding"),
        TargetlessDynamicContextCase::unsupported_path(
            "call_field_function_param",
            &["holder", "callback"],
        ),
        TargetlessDynamicContextCase::unsupported("call_indexed_function_pointer"),
        TargetlessDynamicContextCase::unsupported("call_indexed_field_function_param"),
        TargetlessDynamicContextCase::unsupported("call_indexed_tuple_field_function_param"),
        TargetlessDynamicContextCase::unsupported("call_move_closure_literal_with_body_call"),
        TargetlessDynamicContextCase::unsupported("call_async_closure_literal_with_body_call"),
        TargetlessDynamicContextCase::unsupported_path(
            "call_parenthesized_generic_fn_once_value_binding",
            &["generic_f"],
        ),
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
