use std::collections::HashSet;

use super::*;

#[test]
fn fixture_node_proof_projection_projects_owner_node_rows() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_node(owner)?;
    assert!(
        !context.outgoing.is_empty(),
        "owner node should expose outgoing call rows: {context:#?}"
    );
    assert!(
        context.incoming.is_empty(),
        "fixture owner node should not have incoming callers in this case: {context:#?}"
    );

    let count = db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")?;
    assert_eq!(
        count,
        expected_node_proof_count(&context),
        "owner-node proof count"
    );

    let site = row_by_path(&context.outgoing, &["crate", "local_target"])
        .site
        .id;
    let rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_resolved_site_proof("owner-node proof projection", &rows, owner, site, target);

    Ok(())
}

#[test]
fn fixture_node_proof_projection_projects_function_target_rows() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_node(target)?;
    assert!(
        context.outgoing.is_empty(),
        "local_target has no outgoing call rows in this fixture case: {context:#?}"
    );
    assert!(
        context.incoming.len() >= 2,
        "local_target should expose multiple incoming caller rows: {context:#?}"
    );

    let count = db.project_call_proof_facts_for_node(target, "bd:fixture-call-graph")?;
    assert_eq!(
        count,
        expected_node_proof_count(&context),
        "function target-node proof count"
    );

    let rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_eq!(
        rows.len(),
        count,
        "target-node symbol lookup should return every projected caller fact: {rows:#?}"
    );

    Ok(())
}

#[test]
fn fixture_node_proof_projection_projects_constructor_target_rows() -> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;
        let context = db.call_context_for_node(resolved.target)?;
        assert!(
            context.outgoing.is_empty(),
            "{} constructor target should not require owner-scoped source lookup: {context:#?}",
            case.label
        );
        assert!(
            !context.incoming.is_empty(),
            "{} constructor target should expose incoming caller rows: {context:#?}",
            case.label
        );

        let count = db.project_call_proof_facts_for_node(resolved.target, case.domain)?;
        assert_eq!(
            count,
            expected_node_proof_count(&context),
            "{} constructor target-node proof count",
            case.label
        );

        let edges = db.proof_checker_edges()?;
        assert!(
            edges.iter().any(|edge| {
                edge.call_site_id == resolved.site_str
                    && edge.caller_def_id == resolved.owner_str
                    && edge.callee_def_id.as_deref() == Some(resolved.target_str.as_str())
                    && edge.resolution_state == "resolved"
                    && edge.blocker_reason.is_none()
            }),
            "{} node projection proof edge missing: {edges:#?}",
            case.label
        );
    }

    Ok(())
}

fn expected_node_proof_count(context: &CallNodeContext) -> usize {
    let mut seen = HashSet::new();
    context
        .outgoing
        .iter()
        .chain(context.incoming.iter())
        .filter(|row| seen.insert(row.site.id))
        .map(|row| {
            if row.status.status == CallStatusKind::Resolved {
                2 + row.targets.len()
            } else {
                2
            }
        })
        .sum()
}
