use super::*;

#[test]
fn fixture_proof_query_links_real_resolved_call_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    let rows = db.proof_graphrag_context(&target.to_string())?;
    assert_eq!(
        rows.len(),
        3,
        "real callee id query should return the edge plus linked call_site and call_resolution rows: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.caller_def_id.as_deref() == Some(owner.to_string().as_str())
        }),
        "linked real call_site row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.callee_def_id.as_deref() == Some(target.to_string().as_str())
        }),
        "matching real call_edge row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.blocker_reason.is_none()
        }),
        "linked real call_resolution row missing: {rows:#?}"
    );

    Ok(())
}

#[test]
fn fixture_proof_symbol_lookup_links_real_resolved_call_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    let rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_eq!(
        rows.len(),
        3,
        "real symbol lookup should return the edge plus linked call_site and call_resolution rows: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.caller_def_id.as_deref() == Some(owner.to_string().as_str())
        }),
        "linked real call_site row missing from symbol lookup: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.callee_def_id.as_deref() == Some(target.to_string().as_str())
        }),
        "matching real call_edge row missing from symbol lookup: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.blocker_reason.is_none()
        }),
        "linked real call_resolution row missing from symbol lookup: {rows:#?}"
    );

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
    assert_eq!(
        count,
        expected_target_proof_count(&callers),
        "target-centered symbol lookup setup proof count"
    );

    let rows = db.proof_symbol_lookup(&target.to_string())?;
    let resolved_count = callers
        .iter()
        .filter(|caller| caller.status.status == CallStatusKind::Resolved)
        .count();
    assert_eq!(
        rows.len(),
        resolved_count * 3,
        "target-centered symbol lookup should return linked call_site, call_edge, and call_resolution facts for resolved callers: {rows:#?}"
    );

    for (owner, site) in [(path_owner, path_site), (dynamic_owner, dynamic_site)] {
        let owner_id = owner.to_string();
        let target_id = target.to_string();
        let site_id = site.to_string();
        let site_rows = rows
            .iter()
            .filter(|fact| fact.call_site_id.as_deref() == Some(site_id.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            site_rows.len(),
            3,
            "symbol lookup should include linked call_site, call_edge, and call_resolution facts for {site}: {site_rows:#?}"
        );
        assert_eq!(proof_kind_count(&site_rows, "call_site"), 1);
        assert_eq!(proof_kind_count(&site_rows, "call_edge"), 1);
        assert_eq!(proof_kind_count(&site_rows, "call_resolution"), 1);

        let site_fact = proof_fact_for_kind(&site_rows, "call_site");
        assert_eq!(site_fact.caller_def_id.as_deref(), Some(owner_id.as_str()));

        let edge = proof_fact_for_kind(&site_rows, "call_edge");
        assert_eq!(edge.caller_def_id.as_deref(), Some(owner_id.as_str()));
        assert_eq!(edge.callee_def_id.as_deref(), Some(target_id.as_str()));
        assert_eq!(edge.blocker_reason, None);

        let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
        assert_eq!(resolution.blocker_reason, None);
    }

    Ok(())
}
