use super::*;

#[test]
fn fixture_projection_stores_real_multi_row_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "context rows: {context:#?}");
    let ok_site = row_by_path(&context, &["Ok"]).site.id;
    let try_site = row_by_path(&context, &["try_local_assoc"]).site.id;
    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method_site = row_by_method_receiver(&context, "instance_value", &receiver)
        .site
        .id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 8);

    let mut edges = db.proof_checker_edges()?;
    edges.sort_by(|left, right| left.call_site_id.cmp(&right.call_site_id));
    assert_eq!(edges.len(), 2, "proof checker edges: {edges:#?}");
    assert!(
        edges
            .iter()
            .all(|edge| edge.caller_def_id == owner.to_string())
    );
    assert!(edges.iter().all(|edge| edge.resolution_state == "resolved"));
    assert!(edges.iter().all(|edge| edge.blocker_reason.is_none()));
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == try_site.to_string()
                && edge.callee_def_id.as_deref() == Some(try_target.to_string().as_str())
        }),
        "try path proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == method_site.to_string()
                && edge.callee_def_id.as_deref() == Some(method_target.to_string().as_str())
        }),
        "method proof edges: {edges:#?}"
    );

    let blocked = db.proof_graphrag_context("type_resolution_missing")?;
    assert!(
        blocked.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(ok_site.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "blocked proof rows: {blocked:#?}"
    );

    for site in [ok_site, try_site, method_site] {
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
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "context rows: {context:#?}");

    let expected_fact_count = context
        .iter()
        .map(|row| 2 + row.targets.len())
        .sum::<usize>();
    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(
        count, expected_fact_count,
        "proof projection should emit one call_site and one call_resolution per site plus resolved edges"
    );

    let proof_rows = db.proof_graphrag_context("")?;
    let resolved_context = context
        .iter()
        .filter(|row| row.status.status == CallStatusKind::Resolved)
        .count();
    let checker_edges = db.proof_checker_edges()?;
    assert_eq!(
        checker_edges.len(),
        resolved_context,
        "proof checker edges should match resolved call-context rows: {checker_edges:#?}"
    );

    for row in &context {
        let site = row.site.id.to_string();
        let site_rows = proof_rows
            .iter()
            .filter(|fact| fact.call_site_id.as_deref() == Some(site.as_str()))
            .collect::<Vec<_>>();
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
                let owner_id = owner.to_string();
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
