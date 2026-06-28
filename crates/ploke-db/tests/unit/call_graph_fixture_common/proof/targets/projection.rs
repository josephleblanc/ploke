use super::*;

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
    assert_target_proof_projection_source_counts(
        db,
        label,
        domain,
        target,
        callers,
        expected,
        &[(source_suffix, expected.len())],
        blocker_reason,
    )
}

pub(in crate::unit) fn assert_target_proof_projection_source_counts(
    db: &Database,
    label: &str,
    domain: &str,
    target: Uuid,
    callers: &[CallCallerRow],
    expected: &[TargetProofSite],
    source_counts: &[(&str, usize)],
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

    let mut actual_source_counts = std::collections::BTreeMap::<&str, usize>::new();
    for site in expected {
        let provenance = db
            .proof_source_provenance(&site.site.to_string())?
            .unwrap_or_else(|| panic!("projected {label} source provenance"));
        let suffix = source_counts
            .iter()
            .map(|(suffix, _)| *suffix)
            .find(|suffix| provenance.source_file.ends_with(suffix))
            .unwrap_or_else(|| {
                panic!("{label} source provenance did not match expected suffixes: {provenance:#?}")
            });
        *actual_source_counts.entry(suffix).or_default() += 1;
        assert!(provenance.start_byte < provenance.end_byte);
    }

    let expected_source_counts = source_counts
        .iter()
        .copied()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        actual_source_counts, expected_source_counts,
        "{label} source provenance counts"
    );

    Ok(())
}
