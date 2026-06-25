use super::*;
use ploke_db::ProofGraphContextRow;

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
        assert_linked_resolved_facts(label, &rows, owner, site, target);
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

fn assert_linked_resolved_facts(
    label: &str,
    rows: &[ProofGraphContextRow],
    owner: Uuid,
    site: Uuid,
    target: Uuid,
) {
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
        "{label} should return linked call_site, call_edge, and call_resolution facts for {site}: {site_rows:#?}"
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
