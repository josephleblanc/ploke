use super::*;

#[test]
fn fixture_projection_links_target_centered_proof_rows_to_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "local_target should expose multiple incoming callers: {callers:#?}"
    );
    let resolved_callers = callers
        .iter()
        .filter(|caller| caller.status.status == CallStatusKind::Resolved)
        .collect::<Vec<_>>();
    assert!(
        resolved_callers.len() >= 2,
        "target-centered proof linkage setup should include resolved callers for the seed target: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "target-centered proof linkage setup returned mismatched target rows: {callers:#?}"
    );

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    let expected_count = expected_target_proof_count(&callers);
    assert_eq!(
        count, expected_count,
        "target-centered proof projection should emit call_site and call_resolution for each caller, plus call_edge for resolved callers"
    );

    let proof_rows = db.proof_graphrag_context("")?;
    assert_eq!(
        proof_rows.len(),
        count,
        "target-centered projection should not store unrelated proof rows: {proof_rows:#?}"
    );
    let checker_edges = db.proof_checker_edges()?;
    assert_eq!(
        checker_edges.len(),
        resolved_callers.len(),
        "proof checker edges should match target-centered caller rows: {checker_edges:#?}"
    );

    for caller in &callers {
        let site = caller.site.id.to_string();
        let site_rows = proof_rows
            .iter()
            .filter(|fact| fact.call_site_id.as_deref() == Some(site.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            proof_kind_count(&site_rows, "call_site"),
            1,
            "target-centered proof rows should include one call_site fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_resolution"),
            1,
            "target-centered proof rows should include one call_resolution fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_edge"),
            usize::from(caller.status.status == CallStatusKind::Resolved),
            "target-centered proof rows should include call_edge facts only for resolved callers for {site}: {site_rows:#?}"
        );

        let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
        if caller.status.status == CallStatusKind::Resolved {
            let edge = proof_fact_for_kind(&site_rows, "call_edge");
            let owner_id = caller.site.owner_id.to_string();
            let target_id = target.to_string();
            assert_eq!(edge.caller_def_id.as_deref(), Some(owner_id.as_str()));
            assert_eq!(edge.callee_def_id.as_deref(), Some(target_id.as_str()));
            assert_eq!(edge.blocker_reason, None);
            assert_eq!(resolution.blocker_reason, None);
        } else {
            assert_eq!(
                resolution.blocker_reason.as_deref(),
                Some("type_resolution_missing")
            );
        }

        let provenance = db
            .proof_source_provenance(&site)?
            .expect("projected target-centered caller source provenance");
        assert_eq!(provenance.call_site_id, site);
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, caller.site.span.0);
        assert_eq!(provenance.end_byte, caller.site.span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_excludes_closure_async_outer_owners_from_target_proof() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let forbidden_owners = [
        function_id_by_name(&db, "closure_body_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "async_block_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "call_move_closure_literal_with_body_call")?,
        function_id_by_name(&db, "call_async_closure_literal_with_body_call")?,
    ]
    .map(|owner| owner.to_string());

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    assert!(
        count > 0,
        "target-centered proof should project real callers"
    );

    let edges = db.proof_checker_edges()?;
    assert!(
        !edges.is_empty(),
        "target-centered local_target proof should include resolved proof edges"
    );
    assert!(
        edges
            .iter()
            .all(|edge| !forbidden_owners.contains(&edge.caller_def_id)),
        "target-centered local_target proof leaked closure/async outer owners into proof edges: {edges:#?}"
    );

    let rows = db.proof_graphrag_context("")?;
    assert!(
        rows.iter().all(|row| match row.caller_def_id.as_ref() {
            Some(caller) => !forbidden_owners.contains(caller),
            None => true,
        }),
        "target-centered local_target proof leaked closure/async outer owners into proof facts: {rows:#?}"
    );

    Ok(())
}
