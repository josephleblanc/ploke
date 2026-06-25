use super::*;

#[test]
fn fixture_callers_for_target_reads_real_incoming_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let path_owner = function_id_by_name(&db, "call_crate_local_target")?;
    let dynamic_owner = function_id_by_name(&db, "call_parenthesized_local_target")?;

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "local_target should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "incoming caller query returned a mismatched target edge: {callers:#?}"
    );
    let resolved_callers = callers
        .iter()
        .filter(|caller| caller.status.status == CallStatusKind::Resolved)
        .collect::<Vec<_>>();
    assert!(
        resolved_callers.len() >= 2,
        "local_target should have multiple resolved incoming callers: {callers:#?}"
    );

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
fn fixture_expand_call_context_reads_real_outgoing_and_incoming_candidates() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let path_owner = function_id_by_name(&db, "call_crate_local_target")?;
    let dynamic_owner = function_id_by_name(&db, "call_parenthesized_local_target")?;
    let path_context = db.call_context_for_owner(path_owner)?;
    let path_site = row_by_path(&path_context, &["crate", "local_target"])
        .site
        .id;
    let callers = db.callers_for_target(target)?;
    let incoming_path_site = caller_by_owner_kind_path(
        &callers,
        path_owner,
        CallSiteKind::Path,
        &["crate", "local_target"],
    )
    .site
    .id;
    let dynamic_site = caller_by_owner_kind_path(
        &callers,
        dynamic_owner,
        CallSiteKind::Dynamic,
        &["local_target"],
    )
    .site
    .id;
    assert_eq!(
        incoming_path_site, path_site,
        "owner and target helpers should agree on path call-site identity"
    );

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(path_owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(
        outgoing.len(),
        1,
        "path owner should expose one outgoing call-context candidate: {outgoing:#?}"
    );
    assert_eq!(outgoing[0].node_id, target);
    assert_eq!(outgoing[0].target_id, target);
    assert_eq!(outgoing[0].relation, CallContextRelation::OutgoingTarget);
    assert_eq!(outgoing[0].call_site_id, path_site);
    assert_eq!(outgoing[0].distance, 1);

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 128,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.len() >= 2,
        "local_target should expose multiple incoming caller candidates: {incoming:#?}"
    );
    assert!(
        incoming.iter().all(|candidate| {
            candidate.target_id == target
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 1
        }),
        "incoming expansion should only include caller candidates for the target: {incoming:#?}"
    );
    assert_call_candidate(
        &incoming,
        path_owner,
        CallContextRelation::IncomingCaller,
        path_site,
        target,
        "path caller candidate missing",
    );
    assert_call_candidate(
        &incoming,
        dynamic_owner,
        CallContextRelation::IncomingCaller,
        dynamic_site,
        target,
        "dynamic caller candidate missing",
    );

    Ok(())
}

#[test]
fn fixture_expand_call_context_owner_seed_filters_mixed_status_rows() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "try-result context rows: {context:#?}");

    let ok_site = row_by_path(&context, &["Ok"]).site.id;
    let try_site = row_by_path(&context, &["try_local_assoc"]).site.id;
    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method_site = row_by_method_receiver(&context, "instance_value", &receiver)
        .site
        .id;

    let candidates = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(
        candidates.len(),
        2,
        "owner expansion should promote only resolved rows: {candidates:#?}"
    );
    assert_call_candidate(
        &candidates,
        try_target,
        CallContextRelation::OutgoingTarget,
        try_site,
        try_target,
        "try_local_assoc outgoing target candidate missing",
    );
    assert_call_candidate(
        &candidates,
        method_target,
        CallContextRelation::OutgoingTarget,
        method_site,
        method_target,
        "try-result method outgoing target candidate missing",
    );
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.call_site_id != ok_site),
        "unsupported Ok path call must not be promoted: {candidates:#?}"
    );

    Ok(())
}

#[test]
fn fixture_expand_call_context_target_seed_preserves_method_family_callers() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let method_owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let assoc_owner = function_id_by_name(&db, "call_method_as_associated_function")?;

    let method_context = db.call_context_for_owner(method_owner)?;
    let method_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let method_site = row_by_method_receiver(&method_context, "instance_value", &method_receiver)
        .site
        .id;

    let assoc_context = db.call_context_for_owner(assoc_owner)?;
    let assoc_site = row_by_path(&assoc_context, &["LocalAssoc", "instance_value"])
        .site
        .id;

    let candidates = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 128,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        candidates.len() >= 2,
        "method target should expose multiple incoming expansion candidates: {candidates:#?}"
    );
    assert!(
        candidates.iter().all(|candidate| {
            candidate.target_id == target
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 1
        }),
        "incoming method expansion should preserve the seed target and relation: {candidates:#?}"
    );
    assert_call_candidate(
        &candidates,
        method_owner,
        CallContextRelation::IncomingCaller,
        method_site,
        target,
        "method-call incoming candidate missing",
    );
    assert_call_candidate(
        &candidates,
        assoc_owner,
        CallContextRelation::IncomingCaller,
        assoc_site,
        target,
        "associated-function incoming candidate missing",
    );

    Ok(())
}

#[test]
fn fixture_expand_call_context_target_seed_preserves_constructor_callers() -> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;
        assert_constructor_callers(&db, case, &resolved)?;

        let candidates = db.expand_call_context(
            CallContextSeed::Target(resolved.target),
            CallContextOptions {
                include_outgoing_targets: false,
                max_candidates: 128,
                ..CallContextOptions::default()
            },
        )?;
        assert_call_candidate(
            &candidates,
            resolved.owner,
            CallContextRelation::IncomingCaller,
            resolved.site,
            resolved.target,
            &format!("{} incoming candidate missing", case.label),
        );
    }

    Ok(())
}

#[test]
fn fixture_callers_for_target_reads_method_and_associated_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "LocalAssoc::instance_value should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "method target caller query returned a mismatched target edge: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "incoming method callers should preserve resolved statuses: {callers:#?}"
    );

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
    assert!(
        callers.len() >= 3,
        "LocalAssoc::make should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "associated-function target caller query returned a mismatched target edge: {callers:#?}"
    );

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
