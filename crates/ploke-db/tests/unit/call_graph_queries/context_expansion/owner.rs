use super::*;

#[test]
fn context_for_owner_returns_sites_statuses_and_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(1);
    let path_site = Uuid::from_u128(2);
    let target = Uuid::from_u128(3);
    let dyn_site = Uuid::from_u128(4);
    let dyn_target = Uuid::from_u128(5);

    insert_call_site(
        &db,
        SiteSeed {
            id: path_site,
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
    insert_edge(&db, owner, path_site, "Path")?;
    insert_relation(&db, path_site, target, "Function", "Path", "Function")?;
    insert_status(&db, path_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: dyn_site,
            owner,
            kind: "Dynamic",
            span: (30, 41),
            path: None,
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(1),
            generic_arg_count: None,
        },
    )?;
    insert_edge(&db, owner, dyn_site, "Dynamic")?;
    insert_relation(
        &db,
        dyn_site,
        dyn_target,
        "DynamicFunction",
        "Dynamic",
        "Function",
    )?;
    insert_status(&db, dyn_site, "Dynamic", "Resolved", Some("LocalExact"))?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "context rows: {context:#?}");

    let path_row = &context[0];
    assert_eq!(path_row.site.id, path_site);
    assert_eq!(path_row.site.kind, CallSiteKind::Path);
    assert_eq!(
        path_row.site.path.as_deref(),
        Some(["crate".to_string(), "helper".to_string()].as_slice())
    );
    assert_eq!(path_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        path_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(path_row.targets.len(), 1);
    assert_eq!(path_row.targets[0].target_id, target);
    assert_eq!(path_row.targets[0].relation, CallRelationKind::Function);

    let dyn_row = &context[1];
    assert_eq!(dyn_row.site.id, dyn_site);
    assert_eq!(dyn_row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(dyn_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        dyn_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(dyn_row.targets.len(), 1);
    assert_eq!(dyn_row.targets[0].target_id, dyn_target);
    assert_eq!(
        dyn_row.targets[0].relation,
        CallRelationKind::DynamicFunction
    );
    assert_eq!(dyn_row.targets[0].target_kind, CallTargetKind::Function);

    Ok(())
}
