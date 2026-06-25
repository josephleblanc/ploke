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

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetProofSite {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) site: Uuid,
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct BlockerProofSite {
    pub(in crate::unit) site: Uuid,
    pub(in crate::unit) span: (u32, u32),
    pub(in crate::unit) blocker_reason: &'static str,
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

pub(in crate::unit) fn assert_targetless_blocker_proofs(
    db: &Database,
    label: &str,
    expected: &[BlockerProofSite],
    source_suffix: &str,
) -> Result<(), DbError> {
    assert!(
        db.proof_checker_edges()?.is_empty(),
        "{label} targetless blockers must not fabricate proof edges"
    );

    assert_blocker_proofs(db, label, expected, source_suffix)
}

pub(in crate::unit) fn assert_blocker_proofs(
    db: &Database,
    label: &str,
    expected: &[BlockerProofSite],
    source_suffix: &str,
) -> Result<(), DbError> {
    for expected in expected {
        let site = expected.site.to_string();
        let rows = db.proof_graphrag_context(expected.blocker_reason)?;
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.as_str())
                    && row.blocker_reason.as_deref() == Some(expected.blocker_reason)
            }),
            "{label} proof rows missing {site}: {rows:#?}"
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

    Ok(())
}

pub(in crate::unit) fn assert_resolved_target_callers(
    callers: &[CallCallerRow],
    target: Uuid,
    min_count: usize,
    label: &str,
) -> Result<(), DbError> {
    assert!(
        callers.len() >= min_count,
        "{label} should have at least {min_count} incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "{label} setup returned mismatched target rows: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "{label} setup should only include resolved local callers: {callers:#?}"
    );
    Ok(())
}

pub(in crate::unit) fn expected_target_proof_count(callers: &[CallCallerRow]) -> usize {
    callers
        .iter()
        .map(|caller| {
            if caller.status.status == CallStatusKind::Resolved {
                3
            } else {
                2
            }
        })
        .sum()
}

pub(in crate::unit) fn assert_target_proof_projection(
    db: &Database,
    label: &str,
    domain: &str,
    target: Uuid,
    callers: &[CallCallerRow],
    expected: &[TargetProofSite],
    source_suffix: &str,
    blocker_reason: &str,
) -> Result<(), DbError> {
    let count = db.project_call_proof_facts_for_target(target, domain)?;
    let resolved_count = callers
        .iter()
        .filter(|caller| caller.status.status == CallStatusKind::Resolved)
        .count();
    let expected_count = expected_target_proof_count(callers);
    assert_eq!(count, expected_count, "{label} target-centered proof count");

    let target_str = target.to_string();
    let edges = db.proof_checker_edges()?;
    assert_eq!(
        edges.len(),
        resolved_count,
        "{label} target-centered proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().all(
            |edge| edge.callee_def_id.as_deref() == Some(target_str.as_str())
                && edge.resolution_state == "resolved"
                && edge.blocker_reason.is_none()
        ),
        "{label} target-centered proof edges should all resolve to the seed target: {edges:#?}"
    );

    for site in expected {
        let owner = site.owner.to_string();
        let call_site = site.site.to_string();
        assert!(
            edges
                .iter()
                .any(|edge| { edge.call_site_id == call_site && edge.caller_def_id == owner }),
            "{label} incoming proof edge missing for {call_site}: {edges:#?}"
        );
    }

    assert!(
        db.proof_graphrag_context(blocker_reason)?.is_empty(),
        "{label} projection should not include unrelated {blocker_reason} blockers"
    );

    for site in expected {
        let provenance = db
            .proof_source_provenance(&site.site.to_string())?
            .unwrap_or_else(|| panic!("projected {label} source provenance"));
        assert!(
            provenance.source_file.ends_with(source_suffix),
            "source provenance: {provenance:#?}"
        );
        assert!(provenance.start_byte < provenance.end_byte);
    }

    Ok(())
}
