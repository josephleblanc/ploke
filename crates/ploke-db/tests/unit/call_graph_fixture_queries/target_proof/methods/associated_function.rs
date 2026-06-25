use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_associated_function_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let local_owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let self_owner = method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?;
    let qualified_owner = function_id_by_name(&db, "call_qualified_local_assoc_make")?;
    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 3, "target-centered associated-function")?;
    let expected = assert_proof_site_cases(
        &db,
        &callers,
        &[
            ProofSiteCase::path(
                local_owner,
                &["LocalAssoc", "make"],
                CallRelationKind::AssociatedFunction,
                CallTargetKind::Method,
            ),
            ProofSiteCase::path(
                self_owner,
                &["Self", "make"],
                CallRelationKind::AssociatedFunction,
                CallTargetKind::Method,
            ),
            ProofSiteCase::path(
                qualified_owner,
                &["LocalAssoc", "make"],
                CallRelationKind::AssociatedFunction,
                CallTargetKind::Method,
            ),
        ],
    )?;

    assert_target_proof_projection(
        &db,
        "target-centered associated-function",
        "bd:fixture-call-graph",
        target,
        &callers,
        &expected,
        "src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}
