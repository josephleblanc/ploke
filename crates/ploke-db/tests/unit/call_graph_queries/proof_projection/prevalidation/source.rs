use super::*;

#[test]
fn proof_projection_requires_source_provenance_before_storage() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x1c1);
    let site = Uuid::from_u128(0x1c2);
    let target = Uuid::from_u128(0x1c3);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

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

    insert_owner_source(&db, good_owner, good_module, "src/good.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: good_site,
            owner: good_owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, good_owner, good_site, "Path")?;
    insert_relation(&db, good_site, target, "Function", "Path", "Function")?;
    insert_status(&db, good_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: bad_site,
            owner: bad_owner,
            kind: "Path",
            span: (30, 44),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, bad_owner, bad_site, "Path")?;
    insert_relation(&db, bad_site, target, "Function", "Path", "Function")?;
    insert_status(&db, bad_site, "Path", "Resolved", Some("LocalExact"))?;

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
