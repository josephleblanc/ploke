use super::*;

#[test]
fn fixture_low_level_helpers_read_projected_sites_targets_and_statuses() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let sites = db.call_sites_for_owner(owner)?;
    assert_eq!(sites.len(), 3, "call sites: {sites:#?}");
    assert!(
        sites.windows(2).all(|pair| pair[0].span <= pair[1].span),
        "call_sites_for_owner must preserve source-span order: {sites:#?}"
    );

    let ok_path = path(&["Ok"]);
    let ok_site = sites
        .iter()
        .find(|site| site.kind == CallSiteKind::Path && site.path.as_ref() == Some(&ok_path))
        .expect("Ok path call site");
    let ok_status = db
        .call_resolution_for_site(ok_site.id)?
        .expect("Ok status row");
    assert_eq!(ok_status.site_id, ok_site.id);
    assert_eq!(ok_status.site_kind, CallSiteKind::Path);
    assert_eq!(ok_status.status, CallStatusKind::Unsupported);
    assert_eq!(ok_status.resolution, None);
    assert!(
        db.call_targets_for_site(ok_site.id)?.is_empty(),
        "unsupported Ok path should not have call targets"
    );

    let try_path = path(&["try_local_assoc"]);
    let try_site = sites
        .iter()
        .find(|site| site.kind == CallSiteKind::Path && site.path.as_ref() == Some(&try_path))
        .expect("try_local_assoc path call site");
    let try_status = db
        .call_resolution_for_site(try_site.id)?
        .expect("try_local_assoc status row");
    assert_eq!(try_status.site_id, try_site.id);
    assert_eq!(try_status.site_kind, CallSiteKind::Path);
    assert_eq!(try_status.status, CallStatusKind::Resolved);
    assert_eq!(try_status.resolution, Some(CallResolutionKind::LocalExact));
    let try_targets = db.call_targets_for_site(try_site.id)?;
    assert_eq!(try_targets.len(), 1, "try targets: {try_targets:#?}");
    assert_eq!(try_targets[0].site_id, try_site.id);
    assert_eq!(try_targets[0].target_id, try_target);
    assert_eq!(try_targets[0].relation, CallRelationKind::Function);
    assert_eq!(try_targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(try_targets[0].target_kind, CallTargetKind::Function);

    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method_site = sites
        .iter()
        .find(|site| {
            site.kind == CallSiteKind::Method
                && site.method.as_deref() == Some("instance_value")
                && site.receiver.as_ref() == Some(&receiver)
        })
        .expect("try receiver method call site");
    let method_status = db
        .call_resolution_for_site(method_site.id)?
        .expect("method status row");
    assert_eq!(method_status.site_id, method_site.id);
    assert_eq!(method_status.site_kind, CallSiteKind::Method);
    assert_eq!(method_status.status, CallStatusKind::Resolved);
    assert_eq!(
        method_status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    let method_targets = db.call_targets_for_site(method_site.id)?;
    assert_eq!(
        method_targets.len(),
        1,
        "method targets: {method_targets:#?}"
    );
    assert_eq!(method_targets[0].site_id, method_site.id);
    assert_eq!(method_targets[0].target_id, method_target);
    assert_eq!(method_targets[0].relation, CallRelationKind::Method);
    assert_eq!(method_targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(method_targets[0].target_kind, CallTargetKind::Method);

    Ok(())
}

#[test]
fn fixture_low_level_helpers_read_method_owner_self_field_site() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = method_id_by_impl_self_type_name(
        &db,
        "SelfFieldAssocOwner",
        "call_self_field_instance_method",
    )?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let sites = db.call_sites_for_owner(owner)?;
    assert_eq!(sites.len(), 1, "self-field method owner sites: {sites:#?}");
    let receiver = CallReceiver::SelfField {
        path: path(&["value"]),
    };
    let site = sites
        .iter()
        .find(|site| {
            site.kind == CallSiteKind::Method
                && site.method.as_deref() == Some("instance_value")
                && site.receiver.as_ref() == Some(&receiver)
        })
        .expect("self-field instance method call site");

    let status = db
        .call_resolution_for_site(site.id)?
        .expect("self-field method status row");
    assert_eq!(status.site_id, site.id);
    assert_eq!(status.site_kind, CallSiteKind::Method);
    assert_eq!(status.status, CallStatusKind::Resolved);
    assert_eq!(status.resolution, Some(CallResolutionKind::LocalExact));

    let targets = db.call_targets_for_site(site.id)?;
    assert_eq!(targets.len(), 1, "self-field method targets: {targets:#?}");
    assert_eq!(targets[0].site_id, site.id);
    assert_eq!(targets[0].target_id, target);
    assert_eq!(targets[0].relation, CallRelationKind::Method);
    assert_eq!(targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(targets[0].target_kind, CallTargetKind::Method);

    Ok(())
}
