use super::*;

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

#[test]
fn call_graph_queries_reject_status_source_kind_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2b1);
    let site = Uuid::from_u128(0x2b2);
    let target = Uuid::from_u128(0x2b3);

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
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Method", "Resolved", Some("LocalExact"))?;

    let status_error = db
        .call_resolution_for_site(site)
        .expect_err("status source-kind mismatch must be rejected");
    assert!(
        status_error
            .to_string()
            .contains("call_resolution_status source_kind"),
        "unexpected status error: {status_error}"
    );

    let context_error = db
        .call_context_for_owner(owner)
        .expect_err("owner context must reject mismatched status rows");
    assert!(
        context_error
            .to_string()
            .contains("call_resolution_status source_kind"),
        "unexpected context error: {context_error}"
    );

    Ok(())
}

#[test]
fn call_graph_queries_reject_status_resolution_kind_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2f1);
    let resolved_site = Uuid::from_u128(0x2f2);
    let resolved_target = Uuid::from_u128(0x2f3);
    let external_site = Uuid::from_u128(0x2f4);
    let external_target = Uuid::from_u128(0x2f5);

    insert_call_site(
        &db,
        SiteSeed {
            id: resolved_site,
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
    insert_edge(&db, owner, resolved_site, "Path")?;
    insert_relation(
        &db,
        resolved_site,
        resolved_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_status(&db, resolved_site, "Path", "Resolved", None)?;

    insert_call_site(
        &db,
        SiteSeed {
            id: external_site,
            owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["std", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, external_site, "Path")?;
    insert_relation(
        &db,
        external_site,
        external_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_status(&db, external_site, "Path", "External", Some("LocalExact"))?;

    for site in [resolved_site, external_site] {
        let status_error = db
            .call_resolution_for_site(site)
            .expect_err("status/resolution mismatch must be rejected");
        assert!(
            status_error
                .to_string()
                .contains("call_resolution_status resolution_kind"),
            "unexpected status error for {site}: {status_error}"
        );
    }

    let context_error = db
        .call_context_for_owner(owner)
        .expect_err("owner context must reject status/resolution mismatch");
    assert!(
        context_error
            .to_string()
            .contains("call_resolution_status resolution_kind"),
        "unexpected owner context error: {context_error}"
    );

    let caller_error = db
        .callers_for_target(resolved_target)
        .expect_err("target-centered callers must reject status/resolution mismatch");
    assert!(
        caller_error
            .to_string()
            .contains("call_resolution_status resolution_kind"),
        "unexpected target-centered caller error: {caller_error}"
    );

    let incoming_error = db
        .expand_call_context(
            CallContextSeed::Target(resolved_target),
            CallContextOptions {
                include_outgoing_targets: false,
                ..CallContextOptions::default()
            },
        )
        .expect_err("call-context expansion must reject status/resolution mismatch");
    assert!(
        incoming_error
            .to_string()
            .contains("call_resolution_status resolution_kind"),
        "unexpected expansion error: {incoming_error}"
    );

    Ok(())
}

#[test]
fn call_graph_queries_reject_resolved_target_cardinality_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let missing_owner = Uuid::from_u128(0x301);
    let missing_site = Uuid::from_u128(0x302);
    let multi_owner = Uuid::from_u128(0x303);
    let multi_site = Uuid::from_u128(0x304);
    let first_target = Uuid::from_u128(0x305);
    let second_target = Uuid::from_u128(0x306);

    insert_call_site(
        &db,
        SiteSeed {
            id: missing_site,
            owner: missing_owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "missing_target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, missing_owner, missing_site, "Path")?;
    insert_status(&db, missing_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: multi_site,
            owner: multi_owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["crate", "multi_target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, multi_owner, multi_site, "Path")?;
    insert_relation(
        &db,
        multi_site,
        first_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_relation(
        &db,
        multi_site,
        second_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_status(&db, multi_site, "Path", "Resolved", Some("LocalExact"))?;

    for owner in [missing_owner, multi_owner] {
        let context_error = db
            .call_context_for_owner(owner)
            .expect_err("resolved call target cardinality mismatch must be rejected");
        assert!(
            context_error.to_string().contains("resolved call site"),
            "unexpected owner context error for {owner}: {context_error}"
        );

        let expansion_error = db
            .expand_call_context(
                CallContextSeed::Owner(owner),
                CallContextOptions {
                    include_incoming_callers: false,
                    ..CallContextOptions::default()
                },
            )
            .expect_err("owner-seeded expansion must reject resolved target cardinality mismatch");
        assert!(
            expansion_error.to_string().contains("resolved call site"),
            "unexpected expansion error for {owner}: {expansion_error}"
        );
    }

    Ok(())
}

#[test]
fn callers_for_target_rejects_resolved_target_cardinality_mismatch() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x321);
    let site = Uuid::from_u128(0x322);
    let first_target = Uuid::from_u128(0x323);
    let second_target = Uuid::from_u128(0x324);

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
    insert_relation(&db, site, first_target, "Function", "Path", "Function")?;
    insert_relation(&db, site, second_target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    for target in [first_target, second_target] {
        let caller_error = db
            .callers_for_target(target)
            .expect_err("target-centered callers must reject resolved multi-target sites");
        assert!(
            caller_error.to_string().contains("resolved call site"),
            "unexpected target-centered caller error for {target}: {caller_error}"
        );

        let incoming_error = db
            .expand_call_context(
                CallContextSeed::Target(target),
                CallContextOptions {
                    include_outgoing_targets: false,
                    ..CallContextOptions::default()
                },
            )
            .expect_err("target-centered expansion must reject resolved multi-target sites");
        assert!(
            incoming_error.to_string().contains("resolved call site"),
            "unexpected expansion error for {target}: {incoming_error}"
        );
    }

    Ok(())
}

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

#[test]
fn call_graph_queries_reject_malformed_call_site_shape() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x2e1);
    let site = Uuid::from_u128(0x2e2);
    let target = Uuid::from_u128(0x2e3);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "target"]),
            method: Some("leaked_method"),
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
        .call_sites_for_owner(owner)
        .expect_err("malformed call-site shape must be rejected");
    assert!(
        owner_error.to_string().contains("malformed Path call_site"),
        "unexpected owner query error: {owner_error}"
    );

    let context_error = db
        .call_context_for_owner(owner)
        .expect_err("owner context must reject malformed call-site shape");
    assert!(
        context_error
            .to_string()
            .contains("malformed Path call_site"),
        "unexpected owner context error: {context_error}"
    );

    let caller_error = db
        .callers_for_target(target)
        .expect_err("target-centered callers must reject malformed call-site shape");
    assert!(
        caller_error
            .to_string()
            .contains("malformed Path call_site"),
        "unexpected target-centered caller error: {caller_error}"
    );

    let incoming_error = db
        .expand_call_context(
            CallContextSeed::Target(target),
            CallContextOptions {
                include_outgoing_targets: false,
                ..CallContextOptions::default()
            },
        )
        .expect_err("call-context expansion must reject malformed call-site shape");
    assert!(
        incoming_error
            .to_string()
            .contains("malformed Path call_site"),
        "unexpected expansion error: {incoming_error}"
    );

    Ok(())
}

#[test]
fn context_for_owner_rejects_missing_status() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(21);
    let site = Uuid::from_u128(22);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Macro",
            span: (7, 18),
            path: None,
            method: None,
            macro_name: Some("println"),
            receiver: None,
            arg_count: None,
            generic_arg_count: None,
        },
    )?;
    insert_edge(&db, owner, site, "Macro")?;

    let error = db
        .call_context_for_owner(owner)
        .expect_err("missing call status should fail");
    assert!(
        error.to_string().contains("missing call_resolution_status"),
        "unexpected error: {error}"
    );

    Ok(())
}

#[test]
fn callers_for_target_rejects_missing_status() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x311);
    let site = Uuid::from_u128(0x312);
    let target = Uuid::from_u128(0x313);

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
    insert_relation(&db, site, target, "Function", "Path", "Function")?;

    let caller_error = db
        .callers_for_target(target)
        .expect_err("target-centered callers must reject missing call status");
    assert!(
        caller_error
            .to_string()
            .contains("missing call_resolution_status"),
        "unexpected target-centered caller error: {caller_error}"
    );

    let incoming_error = db
        .expand_call_context(
            CallContextSeed::Target(target),
            CallContextOptions {
                include_outgoing_targets: false,
                ..CallContextOptions::default()
            },
        )
        .expect_err("target-centered expansion must reject missing call status");
    assert!(
        incoming_error
            .to_string()
            .contains("missing call_resolution_status"),
        "unexpected expansion error: {incoming_error}"
    );

    Ok(())
}
