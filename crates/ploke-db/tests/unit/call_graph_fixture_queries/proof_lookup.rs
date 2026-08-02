use super::*;

#[test]
fn fixture_proof_context_queries_link_real_resolved_call_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;
    let domain = "bd:fixture-call-graph";

    let count = db.project_call_proof_facts_for_owner(owner, domain)?;
    assert_eq!(count, 3);
    let target_id = target.to_string();
    let cases = [
        ("graphrag target id", db.proof_graphrag_context(&target_id)?),
        ("symbol target id", db.proof_symbol_lookup(&target_id)?),
        ("build domain", db.proof_domain_context(domain)?),
    ];
    for (label, rows) in cases {
        assert_resolved_site_proof(label, &rows, owner, site, target);
    }

    Ok(())
}

#[test]
fn fixture_proof_symbol_lookup_links_target_centered_local_target_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let path_owner = function_id_by_name(&db, "call_crate_local_target")?;
    let dynamic_owner = function_id_by_name(&db, "call_parenthesized_local_target")?;
    let callers = db.callers_for_target(target)?;
    let path_site = caller_by_owner_kind_path(
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

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    let expected_count = expected_target_proof_count_with_binding_evidence(&db, &callers)?;
    assert_eq!(
        count, expected_count,
        "target-centered symbol lookup setup proof count"
    );

    let rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_eq!(
        rows.len(),
        expected_count,
        "target-centered symbol lookup should return linked resolved facts and candidate-only ambiguous facts: {rows:#?}"
    );

    for (owner, site) in [(path_owner, path_site), (dynamic_owner, dynamic_site)] {
        assert_resolved_site_proof("target-centered symbol lookup", &rows, owner, site, target);
    }

    Ok(())
}

#[test]
fn fixture_proof_symbol_lookup_matches_ambiguous_dynamic_candidate_payloads() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "other_target")?;
    let sibling = function_id_by_name(&db, "local_target")?;
    let owner = function_id_by_name(&db, "call_if_ambiguous_function_item")?;
    let path_owner = function_id_by_name(&db, AMBIGUOUS_PATH_OWNER)?;
    let direct_owner = function_id_by_name(&db, OTHER_TARGET_DIRECT_RESOLVED_OWNERS[0])?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        OTHER_TARGET_CALLER_COUNT,
        "other_target should be reachable through every direct and candidate fixture caller: {callers:#?}"
    );
    let direct_site = caller_by_owner_kind_path(
        &callers,
        direct_owner,
        CallSiteKind::Path,
        &["other_target"],
    )
    .site
    .id;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "ambiguous owner context: {context:#?}");
    let site = context[0].site.id;
    let mut expected_sites = vec![("dynamic branch candidate", site, false)];
    expected_sites.push((
        "path initialized-binding candidate",
        caller_by_owner_kind_path(&callers, path_owner, CallSiteKind::Path, &["f"])
            .site
            .id,
        false,
    ));

    for (owner_name, expected_path) in AMBIGUOUS_PATH_FUNCTION_CANDIDATES {
        expected_sites.push((
            owner_name,
            caller_by_owner_kind_path(
                &callers,
                function_id_by_name(&db, owner_name)?,
                CallSiteKind::Path,
                expected_path,
            )
            .site
            .id,
            false,
        ));
    }

    for (owner_name, expected_path) in AMBIGUOUS_DYNAMIC_FUNCTION_CANDIDATES {
        expected_sites.push((
            owner_name,
            caller_by_owner_kind_path(
                &callers,
                function_id_by_name(&db, owner_name)?,
                CallSiteKind::Dynamic,
                expected_path,
            )
            .site
            .id,
            false,
        ));
    }

    for owner_name in RETURNED_CONFLICTING_FUNCTION_POINTER_OWNERS {
        expected_sites.push((
            owner_name,
            caller_by_owner_kind_path(
                &callers,
                function_id_by_name(&db, owner_name)?,
                CallSiteKind::Dynamic,
                &["return_conflicting_forwarded_function_pointer"],
            )
            .site
            .id,
            true,
        ));
    }

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    let expected_count = expected_target_proof_count_with_binding_evidence(&db, &callers)?;
    assert_eq!(
        count, expected_count,
        "ambiguous target-centered proof count"
    );

    let rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_resolved_site_proof(
        "direct other_target caller",
        &rows,
        direct_owner,
        direct_site,
        target,
    );
    for (label, site, has_binding_evidence) in expected_sites {
        let site_rows = proof_rows_for_site(&rows, site);
        let expected_rows = if has_binding_evidence { 3 } else { 2 };
        assert_eq!(
            site_rows.len(),
            expected_rows,
            "{label} lookup should include linked call_site, call_resolution, and optional binding_evidence facts without a resolved call_edge: {site_rows:#?}"
        );
        assert_eq!(proof_kind_count(&site_rows, "call_site"), 1);
        assert_eq!(proof_kind_count(&site_rows, "call_resolution"), 1);
        assert_eq!(proof_kind_count(&site_rows, "call_edge"), 0);
        assert_eq!(
            proof_kind_count(&site_rows, "binding_evidence"),
            usize::from(has_binding_evidence),
            "{label} returned-callable binding evidence count"
        );

        let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
        assert_eq!(resolution.resolution_state.as_deref(), Some("ambiguous"));
        let mut actual = resolution.candidate_def_ids.clone();
        actual.sort();
        let mut expected = vec![target.to_string(), sibling.to_string()];
        expected.sort();
        assert_eq!(
            actual, expected,
            "{label} should expose the full ambiguous candidate set"
        );
    }

    Ok(())
}
