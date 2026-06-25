use super::*;

#[test]
fn callers_for_target_returns_sites_statuses_and_matching_edges() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x101);
    let target = Uuid::from_u128(0x102);
    let other_target = Uuid::from_u128(0x103);
    let path_site = Uuid::from_u128(0x104);
    let method_site = Uuid::from_u128(0x105);
    let excluded_site = Uuid::from_u128(0x106);

    insert_resolved_graph(
        &db,
        ResolvedGraphSeed {
            owner,
            module: Uuid::from_u128(0x107),
            site: path_site,
            target,
            file: None,
            span: (10, 20),
            path: &["crate", "target"],
        },
    )?;

    insert_call_site(
        &db,
        SiteSeed {
            id: method_site,
            owner,
            kind: "Method",
            span: (30, 45),
            path: None,
            method: Some("target_method"),
            macro_name: None,
            receiver: Some(("TypedLocalBinding", vec!["value", "TargetType"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, method_site, "Method")?;
    insert_relation(&db, method_site, target, "Method", "Method", "Method")?;
    insert_status(&db, method_site, "Method", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: excluded_site,
            owner,
            kind: "Path",
            span: (50, 60),
            path: Some(vec!["crate", "other"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, excluded_site, "Path")?;
    insert_relation(
        &db,
        excluded_site,
        other_target,
        "Function",
        "Path",
        "Function",
    )?;
    insert_status(&db, excluded_site, "Path", "Resolved", Some("LocalExact"))?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 2, "callers: {callers:#?}");
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "target-centered query returned a mismatched edge: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "callers should preserve status rows: {callers:#?}"
    );

    let path = callers
        .iter()
        .find(|caller| caller.site.id == path_site)
        .expect("path caller row");
    assert_eq!(path.site.kind, CallSiteKind::Path);
    assert_eq!(
        path.site.path.as_deref(),
        Some(["crate".to_string(), "target".to_string()].as_slice())
    );
    assert_eq!(path.target.relation, CallRelationKind::Function);
    assert_eq!(path.target.source_kind, CallSiteKind::Path);
    assert_eq!(path.target.target_kind, CallTargetKind::Function);

    let method = callers
        .iter()
        .find(|caller| caller.site.id == method_site)
        .expect("method caller row");
    assert_eq!(method.site.kind, CallSiteKind::Method);
    assert_eq!(method.site.method.as_deref(), Some("target_method"));
    assert_eq!(
        method.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: vec!["TargetType".to_string()]
        })
    );
    assert_eq!(method.target.relation, CallRelationKind::Method);
    assert_eq!(method.target.source_kind, CallSiteKind::Method);
    assert_eq!(method.target.target_kind, CallTargetKind::Method);

    Ok(())
}
