use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "try_local_assoc")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 1, "target-centered function")?;
    let expected = assert_proof_site_cases(
        &db,
        &callers,
        &[ProofSiteCase::path(
            owner,
            &["try_local_assoc"],
            CallRelationKind::Function,
            CallTargetKind::Function,
        )],
    )?;

    assert_target_proof_projection(
        &db,
        "target-centered function",
        "bd:fixture-call-graph",
        target,
        &callers,
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_dynamic_call_proof_facts() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let owner = function_id_by_name(&db, "call_aliased_indexed_named_field_function_binding")?;

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "local_target should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "target-centered dynamic proof setup returned mismatched target rows: {callers:#?}"
    );
    let expected = assert_proof_site_cases(
        &db,
        &callers,
        &[ProofSiteCase::dynamic(
            owner,
            &["alias", "callbacks", "0"],
            CallRelationKind::DynamicFunction,
            CallTargetKind::Function,
        )],
    )?;

    assert_target_proof_projection(
        &db,
        "target-centered dynamic",
        "bd:fixture-call-graph",
        target,
        &callers,
        &expected,
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_constructor_call_proof_facts()
-> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;
        let callers = assert_constructor_callers(&db, case, &resolved)?;

        assert_target_proof_projection(
            &db,
            case.label,
            case.domain,
            resolved.target,
            &callers,
            &[TargetProofSite {
                owner: resolved.owner,
                site: resolved.site,
            }],
            case.source_suffix,
            "type_resolution_missing",
        )?;
    }

    Ok(())
}
