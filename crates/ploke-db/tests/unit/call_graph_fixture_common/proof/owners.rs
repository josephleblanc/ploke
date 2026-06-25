use super::*;

#[derive(Clone, Copy)]
pub(in crate::unit) struct OwnerProofEdge {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) site: Uuid,
    pub(in crate::unit) span: (u32, u32),
    pub(in crate::unit) target: Uuid,
}

#[derive(Clone, Copy)]
pub(in crate::unit) enum ProofEdgeCount {
    Exact,
    AtLeast,
}

pub(in crate::unit) fn assert_owner_proof_edges(
    db: &Database,
    label: &str,
    expected: &[OwnerProofEdge],
    source_suffix: &str,
    blocker_reason: &str,
    count: ProofEdgeCount,
) -> Result<(), DbError> {
    let edges = db.proof_checker_edges()?;
    match count {
        ProofEdgeCount::Exact => assert_eq!(
            edges.len(),
            expected.len(),
            "{label} proof checker edges: {edges:#?}"
        ),
        ProofEdgeCount::AtLeast => assert!(
            edges.len() >= expected.len(),
            "{label} proof checker edges should include at least the expected edges: {edges:#?}"
        ),
    }

    for expected in expected {
        let owner = expected.owner.to_string();
        let site = expected.site.to_string();
        let target = expected.target.to_string();
        assert!(
            edges.iter().any(|edge| {
                edge.call_site_id == site
                    && edge.caller_def_id == owner
                    && edge.callee_def_id.as_deref() == Some(target.as_str())
                    && edge.resolution_state == "resolved"
                    && edge.blocker_reason.is_none()
            }),
            "{label} proof edge missing for {site}: {edges:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site)?
            .unwrap_or_else(|| panic!("projected fixture {label} call-site source provenance"));
        assert!(
            provenance.source_file.ends_with(source_suffix),
            "source provenance for {site}: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, expected.span.0);
        assert_eq!(provenance.end_byte, expected.span.1);
    }

    assert!(
        db.proof_graphrag_context(blocker_reason)?.is_empty(),
        "resolved {label} proofs should not produce {blocker_reason} blockers"
    );

    Ok(())
}
