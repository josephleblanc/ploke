use super::super::*;

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
