use super::*;

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
    assert_outgoing_candidate(
        &outgoing,
        target,
        path_site,
        "path outgoing target candidate missing",
    );

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
    assert_incoming_candidates_for_target(
        &incoming,
        target,
        "incoming expansion should only include caller candidates for the target",
    );
    assert_incoming_candidate(
        &incoming,
        path_owner,
        path_site,
        target,
        "path caller candidate missing",
    );
    assert_incoming_candidate(
        &incoming,
        dynamic_owner,
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
    assert_outgoing_candidate(
        &candidates,
        try_target,
        try_site,
        "try_local_assoc outgoing target candidate missing",
    );
    assert_outgoing_candidate(
        &candidates,
        method_target,
        method_site,
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
    assert_incoming_candidates_for_target(
        &candidates,
        target,
        "incoming method expansion should preserve the seed target and relation",
    );
    assert_incoming_candidate(
        &candidates,
        method_owner,
        method_site,
        target,
        "method-call incoming candidate missing",
    );
    assert_incoming_candidate(
        &candidates,
        assoc_owner,
        assoc_site,
        target,
        "associated-function incoming candidate missing",
    );

    Ok(())
}
