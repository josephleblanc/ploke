use super::*;

#[test]
fn fixture_projection_stores_real_multi_row_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let case = try_result_context(&db)?;

    let count = db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 8);

    let mut edges = db.proof_checker_edges()?;
    edges.sort_by(|left, right| left.call_site_id.cmp(&right.call_site_id));
    assert_eq!(edges.len(), 2, "proof checker edges: {edges:#?}");
    assert!(
        edges
            .iter()
            .all(|edge| edge.caller_def_id == case.owner.to_string())
    );
    assert!(edges.iter().all(|edge| edge.resolution_state == "resolved"));
    assert!(edges.iter().all(|edge| edge.blocker_reason.is_none()));
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == case.sites.try_call.to_string()
                && edge.callee_def_id.as_deref() == Some(case.targets.try_fn.to_string().as_str())
        }),
        "try path proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == case.sites.method.to_string()
                && edge.callee_def_id.as_deref() == Some(case.targets.method.to_string().as_str())
        }),
        "method proof edges: {edges:#?}"
    );

    let blocked = db.proof_graphrag_context("type_resolution_missing")?;
    assert!(
        blocked.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(case.sites.ok.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "blocked proof rows: {blocked:#?}"
    );

    for site in [case.sites.ok, case.sites.try_call, case.sites.method] {
        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected fixture call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
    }

    Ok(())
}

#[test]
fn fixture_projection_links_mixed_owner_proof_rows_to_call_context() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let case = try_result_context(&db)?;

    let expected_fact_count = case
        .context
        .iter()
        .map(|row| 2 + row.targets.len())
        .sum::<usize>();
    let count = db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?;
    assert_eq!(
        count, expected_fact_count,
        "proof projection should emit one call_site and one call_resolution per site plus resolved edges"
    );

    let proof_rows = db.proof_graphrag_context("")?;
    let resolved_context = case
        .context
        .iter()
        .filter(|row| row.status.status == CallStatusKind::Resolved)
        .count();
    let checker_edges = db.proof_checker_edges()?;
    assert_eq!(
        checker_edges.len(),
        resolved_context,
        "proof checker edges should match resolved call-context rows: {checker_edges:#?}"
    );

    for row in &case.context {
        let site = row.site.id;
        let site_rows = proof_rows_for_site(&proof_rows, site);
        assert_eq!(
            proof_kind_count(&site_rows, "call_site"),
            1,
            "projected proof rows should include one call_site fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_resolution"),
            1,
            "projected proof rows should include one call_resolution fact for {site}: {site_rows:#?}"
        );

        let edge_rows = site_rows
            .iter()
            .filter(|fact| fact.kind == "call_edge")
            .collect::<Vec<_>>();
        match row.status.status {
            CallStatusKind::Resolved => {
                assert_eq!(
                    edge_rows.len(),
                    1,
                    "resolved call site {site} should project one call_edge fact: {site_rows:#?}"
                );
                let owner_id = case.owner.to_string();
                let target_id = row.targets[0].target_id.to_string();
                assert_eq!(
                    edge_rows[0].caller_def_id.as_deref(),
                    Some(owner_id.as_str())
                );
                assert_eq!(
                    edge_rows[0].callee_def_id.as_deref(),
                    Some(target_id.as_str())
                );
                assert_eq!(edge_rows[0].blocker_reason, None);
            }
            CallStatusKind::Unresolved
            | CallStatusKind::Ambiguous
            | CallStatusKind::External
            | CallStatusKind::Unsupported => {
                assert!(
                    edge_rows.is_empty(),
                    "non-resolved call site {site} must not project call_edge facts: {site_rows:#?}"
                );
                let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
                assert!(
                    resolution.blocker_reason.is_some(),
                    "non-resolved call site {site} should project a blocker reason: {site_rows:#?}"
                );
            }
        }
    }

    Ok(())
}
