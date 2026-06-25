use ploke_db::ProofCheckerEdgeRow;

use super::*;

#[test]
fn fixture_projection_stores_real_constructor_call_proof_facts() -> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;

        let count = db.project_call_proof_facts_for_owner(resolved.owner, case.domain)?;
        assert_eq!(
            count, resolved.proof_count,
            "{} owner-scoped proof count",
            case.label
        );

        let edges = db.proof_checker_edges()?;
        assert_proof_edge(&edges, case, &resolved);
        assert_provenance(&db, case, &resolved)?;
    }

    Ok(())
}

fn assert_proof_edge(
    edges: &[ProofCheckerEdgeRow],
    case: &ConstructorCase,
    resolved: &ResolvedConstructor,
) {
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == resolved.site_str
                && edge.caller_def_id == resolved.owner_str
                && edge.callee_def_id.as_deref() == Some(resolved.target_str.as_str())
                && edge.resolution_state == "resolved"
                && edge.blocker_reason.is_none()
        }),
        "{} proof edge missing: {edges:#?}",
        case.label
    );
}

fn assert_provenance(
    db: &Database,
    case: &ConstructorCase,
    resolved: &ResolvedConstructor,
) -> Result<(), DbError> {
    let provenance = db
        .proof_source_provenance(&resolved.site_str)?
        .unwrap_or_else(|| panic!("projected {} source provenance", case.label));
    assert!(
        provenance.source_file.ends_with(case.source_suffix),
        "source provenance: {provenance:#?}"
    );
    assert_eq!(provenance.start_byte, resolved.span.0);
    assert_eq!(provenance.end_byte, resolved.span.1);
    Ok(())
}
