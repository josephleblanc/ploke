use super::super::*;

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
