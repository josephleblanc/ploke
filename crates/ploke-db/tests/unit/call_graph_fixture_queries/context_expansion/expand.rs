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
    let case = try_result_context(&db)?;

    let candidates = db.expand_call_context(
        CallContextSeed::Owner(case.owner),
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
        case.targets.try_fn,
        case.sites.try_call,
        "try_local_assoc outgoing target candidate missing",
    );
    assert_outgoing_candidate(
        &candidates,
        case.targets.method,
        case.sites.method,
        "try-result method outgoing target candidate missing",
    );
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.call_site_id != case.sites.ok),
        "unsupported Ok path call must not be promoted: {candidates:#?}"
    );

    Ok(())
}

#[test]
fn fixture_expand_call_context_target_seed_preserves_method_family_callers() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let assoc_owner = function_id_by_name(&db, "call_method_as_associated_function")?;
    let typed_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let self_field_receiver = CallReceiver::SelfField {
        path: path(&["value"]),
    };
    let self_field_owner = method_id_by_impl_self_type_name(
        &db,
        "SelfFieldAssocOwner",
        "call_self_field_instance_method",
    )?;
    let method_callers = [
        (
            function_id_by_name(&db, "call_typed_local_instance_method")?,
            &typed_receiver,
            "method-call",
        ),
        (
            function_id_by_name(&db, "call_typed_double_reference_local_instance_method")?,
            &typed_receiver,
            "nested-reference method",
        ),
        (self_field_owner, &self_field_receiver, "self-field method"),
    ];
    let mut method_candidates = Vec::new();
    for (owner, receiver, label) in method_callers {
        let context = db.call_context_for_owner(owner)?;
        let site = row_by_method_receiver(&context, "instance_value", receiver)
            .site
            .id;
        method_candidates.push((owner, site, label));
    }

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
        candidates.len() >= method_candidates.len() + 1,
        "method target should expose multiple incoming expansion candidates: {candidates:#?}"
    );
    assert_incoming_candidates_for_target(
        &candidates,
        target,
        "incoming method expansion should preserve the seed target and relation",
    );
    for (owner, site, label) in method_candidates {
        assert_incoming_candidate(
            &candidates,
            owner,
            site,
            target,
            &format!("{label} incoming candidate missing"),
        );
    }
    assert_incoming_candidate(
        &candidates,
        assoc_owner,
        assoc_site,
        target,
        "associated-function incoming candidate missing",
    );

    Ok(())
}
