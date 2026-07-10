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
            &[TargetlessBlockerCase {
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
        "call_prelude_vec_new",
        &[TargetlessBlockerCase {
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
fn fixture_projection_marks_conflicting_callable_value_and_field_candidates() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let expected_names = candidate_strings(&expected);

    for (owner_name, expected_path) in [
        ("call_multi_conflicting_function_pointer_param", &["f"][..]),
        (
            "call_forwarded_conflicting_function_pointer_leaf",
            &["f"][..],
        ),
        (
            "call_multi_conflicting_generic_fn_once_param",
            &["generic_f"][..],
        ),
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        assert_path_function_candidates(row, owner, expected_path, &expected, owner_name);

        let site = row.site.id.to_string();
        let facts = db.call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_candidate_proof(&facts, &site, &expected_names, owner_name);
        assert!(
            db.proof_checker_edges()?.is_empty(),
            "{owner_name} ambiguous callable candidates must not fabricate proof edges"
        );
    }

    let owner_name = "call_multi_conflicting_named_field_function_param";
    let owner = function_id_by_name(&db, owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
    let row = &context[0];
    assert_dynamic_path_function_candidates(
        row,
        owner,
        &["holder", "callback"],
        &expected,
        owner_name,
    );

    let site = row.site.id.to_string();
    let facts = db.call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_candidate_proof(&facts, &site, &expected_names, owner_name);
    assert!(
        db.proof_checker_edges()?.is_empty(),
        "{owner_name} ambiguous callable candidates must not fabricate proof edges"
    );

    Ok(())
}

#[test]
fn fixture_projection_marks_real_parenthesized_callable_dynamic_rows_without_edges()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    for (owner_name, expected_path, label) in [
        (
            "call_parenthesized_function_pointer_param",
            &["f"][..],
            "parenthesized function-pointer parameter dynamic proof setup",
        ),
        (
            "call_function_pointer_param_cast",
            &["f"][..],
            "function-pointer parameter cast dynamic proof setup",
        ),
        (
            "call_parenthesized_generic_fn_once_value_binding",
            &["generic_f"][..],
            "parenthesized generic FnOnce dynamic proof setup",
        ),
        (
            "call_field_function_param",
            &["holder", "callback"][..],
            "callable field parameter dynamic proof setup",
        ),
        (
            "call_indexed_function_pointer",
            &["funcs", "0"][..],
            "indexed function pointer dynamic proof setup",
        ),
        (
            "call_indexed_field_function_param",
            &["holder", "callbacks", "0"][..],
            "indexed field callable parameter dynamic proof setup",
        ),
        (
            "call_indexed_tuple_field_function_param",
            &["holder", "0", "0"][..],
            "indexed tuple-field callable parameter dynamic proof setup",
        ),
    ] {
        assert_projected_blockers(
            &db,
            &mut expected,
            owner_name,
            &[TargetlessBlockerCase {
                row: TargetlessRowCase::dynamic(
                    Some(expected_path),
                    CallStatusKind::Unsupported,
                    label,
                ),
                blocker_reason: "dynamic_dispatch_unbounded",
            }],
        )?;
    }

    assert_targetless_blocker_proofs(
        &db,
        "parenthesized callable dynamic rows",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}
