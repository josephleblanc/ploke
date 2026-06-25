use super::super::*;

#[test]
fn call_graph_queries_exclude_invalid_endpoint_family_rows() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2a1);
    let site = Uuid::from_u128(0x2a2);
    let valid_target = Uuid::from_u128(0x2a3);
    let wrong_source = Uuid::from_u128(0x2a4);
    let wrong_target = Uuid::from_u128(0x2a5);
    let wrong_relation = Uuid::from_u128(0x2a6);

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
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, valid_target, "Function", "Path", "Function")?;
    insert_relation(&db, site, wrong_source, "Method", "Method", "Method")?;
    insert_relation(&db, site, wrong_target, "Function", "Path", "Method")?;
    insert_relation(
        &db,
        site,
        wrong_relation,
        "TupleStructConstructor",
        "Path",
        "Method",
    )?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let targets = db.call_targets_for_site(site)?;
    assert_eq!(targets.len(), 1, "target rows: {targets:#?}");
    assert_eq!(targets[0].target_id, valid_target);
    assert_eq!(targets[0].relation, CallRelationKind::Function);
    assert_eq!(targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(targets[0].target_kind, CallTargetKind::Function);

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "owner context: {context:#?}");
    assert_eq!(
        context[0].targets.len(),
        1,
        "owner context should exclude invalid relation families: {context:#?}"
    );
    assert_eq!(context[0].targets[0].target_id, valid_target);

    let callers = db.callers_for_target(valid_target)?;
    assert_eq!(callers.len(), 1, "valid callers: {callers:#?}");
    assert_eq!(callers[0].site.owner_id, owner);
    assert_eq!(callers[0].target.target_id, valid_target);

    for invalid in [wrong_source, wrong_target, wrong_relation] {
        let callers = db.callers_for_target(invalid)?;
        assert!(
            callers.is_empty(),
            "invalid target {invalid} should not surface target-centered callers: {callers:#?}"
        );

        let incoming = db.expand_call_context(
            CallContextSeed::Target(invalid),
            CallContextOptions {
                include_outgoing_targets: false,
                ..CallContextOptions::default()
            },
        )?;
        assert!(
            incoming.is_empty(),
            "invalid target {invalid} should not be promoted for call-context expansion: {incoming:#?}"
        );
    }

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(outgoing.len(), 1, "outgoing candidates: {outgoing:#?}");
    assert_eq!(outgoing[0].node_id, valid_target);
    assert_eq!(outgoing[0].target_id, valid_target);

    Ok(())
}

#[test]
fn call_graph_queries_exclude_missing_endpoint_target_rows() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2b1);
    let site = Uuid::from_u128(0x2b2);
    let valid_target = Uuid::from_u128(0x2b3);
    let missing_target = Uuid::from_u128(0x2b4);

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
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, valid_target, "Function", "Path", "Function")?;
    insert_relation_raw(&db, site, missing_target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let targets = db.call_targets_for_site(site)?;
    assert_eq!(
        targets.len(),
        1,
        "call targets should exclude relation rows whose declared endpoint is missing: {targets:#?}"
    );
    assert_eq!(targets[0].target_id, valid_target);

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "owner context: {context:#?}");
    assert_eq!(
        context[0].targets.len(),
        1,
        "owner context should not count missing endpoint rows as resolved targets: {context:#?}"
    );
    assert_eq!(context[0].targets[0].target_id, valid_target);

    let callers = db.callers_for_target(valid_target)?;
    assert_eq!(callers.len(), 1, "valid callers: {callers:#?}");
    assert_eq!(callers[0].target.target_id, valid_target);

    let dangling_callers = db.callers_for_target(missing_target)?;
    assert!(
        dangling_callers.is_empty(),
        "target-centered helper must not surface missing endpoint callers: {dangling_callers:#?}"
    );

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(outgoing.len(), 1, "outgoing candidates: {outgoing:#?}");
    assert_eq!(outgoing[0].node_id, valid_target);

    let incoming = db.expand_call_context(
        CallContextSeed::Target(missing_target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.is_empty(),
        "call-context expansion must not promote missing endpoint callers: {incoming:#?}"
    );

    Ok(())
}
