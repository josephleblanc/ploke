use super::super::*;

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
