use super::super::*;

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
