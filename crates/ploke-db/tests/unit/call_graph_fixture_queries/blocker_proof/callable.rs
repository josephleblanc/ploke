use super::*;

#[test]
fn fixture_projection_marks_real_callable_path_and_vec_external_rows_without_edges()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    for (owner_name, expected_path) in [
        ("call_function_pointer_param", &["f"][..]),
        ("call_generic_fn_once_value_binding", &["generic_f"][..]),
    ] {
        assert_projected_blockers(
            &db,
            &mut expected,
            owner_name,
            &[ExpectedBlocker {
                row: TargetlessRowCase::path(
                    expected_path,
                    0,
                    CallStatusKind::Unsupported,
                    owner_name,
                ),
                blocker_reason: "type_resolution_missing",
            }],
        )?;
    }

    assert_projected_blockers(
        &db,
        &mut expected,
        "call_boxed_dyn_fn_value_binding",
        &[
            ExpectedBlocker {
                row: TargetlessRowCase::path(
                    &["Box", "new"],
                    1,
                    CallStatusKind::External,
                    "Box::new proof setup",
                ),
                blocker_reason: "external_dependency_summary_missing",
            },
            ExpectedBlocker {
                row: TargetlessRowCase::path(
                    &["boxed_fn"],
                    0,
                    CallStatusKind::Unsupported,
                    "boxed dyn Fn proof setup",
                ),
                blocker_reason: "type_resolution_missing",
            },
        ],
    )?;

    assert_projected_blockers(
        &db,
        &mut expected,
        "call_prelude_vec_new",
        &[ExpectedBlocker {
            row: TargetlessRowCase::path(
                &["Vec", "new"],
                0,
                CallStatusKind::External,
                "Vec::new proof setup",
            ),
            blocker_reason: "external_dependency_summary_missing",
        }],
    )?;

    assert_targetless_blocker_proofs(
        &db,
        "callable path and external rows",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_marks_real_parenthesized_callable_dynamic_rows_without_edges()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    assert_projected_blockers(
        &db,
        &mut expected,
        "call_parenthesized_generic_fn_once_value_binding",
        &[ExpectedBlocker {
            row: TargetlessRowCase::dynamic(
                Some(&["generic_f"]),
                CallStatusKind::Unsupported,
                "parenthesized generic FnOnce dynamic proof setup",
            ),
            blocker_reason: "dynamic_dispatch_unbounded",
        }],
    )?;

    assert_projected_blockers(
        &db,
        &mut expected,
        "call_parenthesized_boxed_dyn_fn_value_binding",
        &[
            ExpectedBlocker {
                row: TargetlessRowCase::path(
                    &["Box", "new"],
                    1,
                    CallStatusKind::External,
                    "parenthesized boxed dyn Fn Box::new proof setup",
                ),
                blocker_reason: "external_dependency_summary_missing",
            },
            ExpectedBlocker {
                row: TargetlessRowCase::dynamic(
                    Some(&["boxed_fn"]),
                    CallStatusKind::Unsupported,
                    "parenthesized boxed dyn Fn dynamic proof setup",
                ),
                blocker_reason: "dynamic_dispatch_unbounded",
            },
        ],
    )?;

    assert_targetless_blocker_proofs(
        &db,
        "parenthesized callable dynamic rows",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[derive(Clone, Copy)]
struct ExpectedBlocker<'a> {
    row: TargetlessRowCase<'a>,
    blocker_reason: &'static str,
}

fn assert_projected_blockers(
    db: &Database,
    expected: &mut Vec<BlockerProofSite>,
    owner_name: &str,
    blockers: &[ExpectedBlocker<'_>],
) -> Result<(), DbError> {
    let owner = function_id_by_name(db, owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        blockers.len(),
        "{owner_name} proof context rows: {context:#?}"
    );

    for blocker in blockers {
        let row = assert_targetless_row(&context, owner, blocker.row);
        expected.push(BlockerProofSite {
            site: row.site.id,
            span: row.site.span,
            blocker_reason: blocker.blocker_reason,
        });
    }

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, blockers.len() * 2);
    Ok(())
}
