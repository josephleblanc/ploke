use super::*;

#[test]
fn fixture_callers_for_target_reads_real_incoming_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let path_owner = function_id_by_name(&db, "call_crate_local_target")?;
    let dynamic_owner = function_id_by_name(&db, "call_parenthesized_local_target")?;

    let callers = db.callers_for_target(target)?;
    assert_callers_for_target(&callers, target, 2, "local_target");
    assert_min_resolved_callers(&callers, 2, "local_target");

    let path = caller_by_owner_kind_path(
        &callers,
        path_owner,
        CallSiteKind::Path,
        &["crate", "local_target"],
    );
    assert_eq!(path.status.status, CallStatusKind::Resolved);
    assert_eq!(path.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path.target.relation, CallRelationKind::Function);
    assert_eq!(path.target.source_kind, CallSiteKind::Path);
    assert_eq!(path.target.target_kind, CallTargetKind::Function);

    let dynamic = caller_by_owner_kind_path(
        &callers,
        dynamic_owner,
        CallSiteKind::Dynamic,
        &["local_target"],
    );
    assert_eq!(dynamic.status.status, CallStatusKind::Resolved);
    assert_eq!(
        dynamic.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(dynamic.target.relation, CallRelationKind::DynamicFunction);
    assert_eq!(dynamic.target.source_kind, CallSiteKind::Dynamic);
    assert_eq!(dynamic.target.target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_callers_for_target_excludes_closure_or_async_body_outer_owners() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let forbidden_owners = [
        function_id_by_name(&db, "closure_body_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "async_block_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "call_move_closure_literal_with_body_call")?,
        function_id_by_name(&db, "call_async_closure_literal_with_body_call")?,
    ];

    let callers = db.callers_for_target(target)?;
    assert!(
        callers
            .iter()
            .all(|caller| !forbidden_owners.contains(&caller.site.owner_id)),
        "target-centered local_target callers leaked closure/async body outer owners: {callers:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming
            .iter()
            .all(|candidate| !forbidden_owners.contains(&candidate.node_id)),
        "target-seeded local_target expansion leaked closure/async body outer owners: {incoming:#?}"
    );

    Ok(())
}

#[test]
fn fixture_callers_for_target_reads_method_and_associated_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let callers = db.callers_for_target(target)?;
    assert_resolved_callers_for_target(&callers, target, 2, "LocalAssoc::instance_value");

    let owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let caller = caller_by_owner_method_receiver(
        &callers,
        owner,
        "instance_value",
        &CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["LocalAssoc"]),
        },
    );
    assert_eq!(caller.target.relation, CallRelationKind::Method);
    assert_eq!(caller.target.source_kind, CallSiteKind::Method);
    assert_eq!(caller.target.target_kind, CallTargetKind::Method);

    let owner = function_id_by_name(&db, "call_typed_double_reference_local_instance_method")?;
    let caller = caller_by_owner_method_receiver(
        &callers,
        owner,
        "instance_value",
        &CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["LocalAssoc"]),
        },
    );
    assert_eq!(caller.target.relation, CallRelationKind::Method);
    assert_eq!(caller.target.source_kind, CallSiteKind::Method);
    assert_eq!(caller.target.target_kind, CallTargetKind::Method);

    let owner = function_id_by_name(&db, "call_method_as_associated_function")?;
    let caller = caller_by_owner_kind_path(
        &callers,
        owner,
        CallSiteKind::Path,
        &["LocalAssoc", "instance_value"],
    );
    assert_eq!(caller.site.arg_count, Some(1));
    assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(caller.target.target_kind, CallTargetKind::Method);

    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let callers = db.callers_for_target(target)?;
    assert_callers_for_target(&callers, target, 3, "LocalAssoc::make");

    let owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let caller =
        caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["LocalAssoc", "make"]);
    assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(caller.target.target_kind, CallTargetKind::Method);

    let owner = method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?;
    let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["Self", "make"]);
    assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(caller.target.target_kind, CallTargetKind::Method);

    Ok(())
}
