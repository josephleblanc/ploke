use super::*;

#[test]
fn expand_call_context_does_not_promote_non_resolved_target_edges() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let resolved_owner = Uuid::from_u128(0x1a1);
    let unresolved_owner = Uuid::from_u128(0x1a2);
    let ambiguous_owner = Uuid::from_u128(0x1a3);
    let target = Uuid::from_u128(0x1a4);
    let resolved_site = Uuid::from_u128(0x1a5);
    let unresolved_site = Uuid::from_u128(0x1a6);
    let ambiguous_site = Uuid::from_u128(0x1a7);

    for (owner, site, status, resolution) in [
        (
            resolved_owner,
            resolved_site,
            "Resolved",
            Some("LocalExact"),
        ),
        (unresolved_owner, unresolved_site, "Unresolved", None),
        (ambiguous_owner, ambiguous_site, "Ambiguous", None),
    ] {
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
        insert_status(&db, site, "Path", status, resolution)?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        3,
        "low-level target-centered helper should expose all persisted target rows: {callers:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(
        incoming.len(),
        1,
        "call-context expansion should only promote resolved target rows: {incoming:#?}"
    );
    assert_eq!(incoming[0].node_id, resolved_owner);
    assert_eq!(incoming[0].call_site_id, resolved_site);
    assert_eq!(incoming[0].relation, CallContextRelation::IncomingCaller);

    for owner in [unresolved_owner, ambiguous_owner] {
        let outgoing = db.expand_call_context(
            CallContextSeed::Owner(owner),
            CallContextOptions {
                include_incoming_callers: false,
                ..CallContextOptions::default()
            },
        )?;
        assert!(
            outgoing.is_empty(),
            "non-resolved owner {owner} should not promote target edges: {outgoing:#?}"
        );
    }

    Ok(())
}
