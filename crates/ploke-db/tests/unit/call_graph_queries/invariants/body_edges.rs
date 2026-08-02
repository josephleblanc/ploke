use super::super::*;

#[test]
fn call_graph_queries_exclude_body_contains_call_site_kind_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2c1);
    let site = Uuid::from_u128(0x2c2);
    let target = Uuid::from_u128(0x2c3);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Method")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let targets = db.call_targets_for_site(site)?;
    assert_eq!(
        targets.len(),
        1,
        "site-local target lookup should still expose valid direct target rows: {targets:#?}"
    );

    let sites = db.call_sites_for_owner(owner)?;
    assert!(
        sites.is_empty(),
        "owner query must exclude BodyContainsCall rows whose target kind disagrees with call_site.call_kind: {sites:#?}"
    );

    let context = db.call_context_for_owner(owner)?;
    assert!(
        context.is_empty(),
        "owner context must not assemble mismatched BodyContainsCall rows: {context:#?}"
    );

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.is_empty(),
        "target-centered callers must exclude mismatched BodyContainsCall rows: {callers:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.is_empty(),
        "call-context expansion must not promote mismatched BodyContainsCall rows: {incoming:#?}"
    );

    Ok(())
}

#[test]
fn call_graph_queries_exclude_body_contains_owner_kind_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2d1);
    let site = Uuid::from_u128(0x2d2);
    let target = Uuid::from_u128(0x2d3);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge_with_source_kind(&db, owner, site, "Method", "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let targets = db.call_targets_for_site(site)?;
    assert_eq!(
        targets.len(),
        1,
        "site-local target lookup should still expose valid direct target rows: {targets:#?}"
    );

    let sites = db.call_sites_for_owner(owner)?;
    assert!(
        sites.is_empty(),
        "owner query must exclude BodyContainsCall rows whose source kind disagrees with the stored owner kind: {sites:#?}"
    );

    let context = db.call_context_for_owner(owner)?;
    assert!(
        context.is_empty(),
        "owner context must not assemble owner-kind mismatched BodyContainsCall rows: {context:#?}"
    );

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.is_empty(),
        "target-centered callers must exclude owner-kind mismatched BodyContainsCall rows: {callers:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.is_empty(),
        "call-context expansion must not promote owner-kind mismatched BodyContainsCall rows: {incoming:#?}"
    );

    Ok(())
}
