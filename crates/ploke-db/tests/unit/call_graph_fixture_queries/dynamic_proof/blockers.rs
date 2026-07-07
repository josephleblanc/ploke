use super::*;

#[test]
fn fixture_projection_marks_real_branch_and_match_dynamic_failures_without_edges()
-> Result<(), DbError> {
    let cases = [
        (
            "call_if_closure_branch",
            None,
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_closure_arm",
            None,
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_if_function_pointer_param_branch",
            Some(&["f"][..]),
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_function_pointer_param_arm",
            Some(&["f"][..]),
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
    ];

    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    for (owner_name, path, expected_status, blocker_reason) in cases {
        assert_projected_blockers(
            &db,
            &mut expected,
            owner_name,
            &[TargetlessBlockerCase {
                row: TargetlessRowCase::dynamic(path, expected_status, owner_name),
                blocker_reason,
            }],
        )?;
    }

    assert_targetless_blocker_proofs(
        &db,
        "branch/match dynamic failure",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}
