use super::*;

const TARGET_PATH: &[&str] = &["crate", "target"];

#[test]
fn proof_projection_requires_source_provenance_before_storage() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x1c1);
    let site = Uuid::from_u128(0x1c2);
    let target = Uuid::from_u128(0x1c3);
    let module = Uuid::from_u128(0x1c4);

    insert_resolved_graph(
        &db,
        ResolvedGraphSeed::path_call(owner, module, site, target, None),
    )?;

    let owner_error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("owner-scoped projection requires source provenance");
    assert!(
        owner_error.to_string().contains("missing source file"),
        "unexpected owner projection error: {owner_error}"
    );

    let target_error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection requires caller source provenance");
    assert!(
        target_error.to_string().contains("missing source file"),
        "unexpected target projection error: {target_error}"
    );

    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "missing source provenance must not leave partial proof facts"
    );

    Ok(())
}

#[test]
fn target_centered_proof_projection_prevalidates_caller_sources_before_storage()
-> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let good_owner = Uuid::from_u128(0x1d1);
    let good_module = Uuid::from_u128(0x1d2);
    let good_site = Uuid::from_u128(0x1d3);
    let bad_owner = Uuid::from_u128(0x1d4);
    let bad_site = Uuid::from_u128(0x1d5);
    let target = Uuid::from_u128(0x1d6);

    insert_resolved_graph(
        &db,
        ResolvedGraphSeed {
            owner: good_owner,
            module: good_module,
            site: good_site,
            target,
            file: Some("src/good.rs"),
            span: (10, 24),
            path: TARGET_PATH,
        },
    )?;
    insert_resolved_graph(
        &db,
        ResolvedGraphSeed {
            owner: bad_owner,
            module: Uuid::from_u128(0x1d7),
            site: bad_site,
            target,
            file: None,
            span: (30, 44),
            path: TARGET_PATH,
        },
    )?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 2, "target callers: {callers:#?}");

    let facts_error = db
        .call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered fact generation requires every caller source");
    assert!(
        facts_error.to_string().contains("missing source file"),
        "unexpected target-centered fact generation error: {facts_error}"
    );

    let projection_error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection requires every caller source");
    assert!(
        projection_error.to_string().contains("missing source file"),
        "unexpected target-centered projection error: {projection_error}"
    );

    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered source prevalidation must not store partial proof facts"
    );

    Ok(())
}
