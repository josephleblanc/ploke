use super::*;

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetProofSite {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) site: Uuid,
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct ProofSiteCase<'a> {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) kind: CallSiteKind,
    pub(in crate::unit) path: &'a [&'a str],
    pub(in crate::unit) relation: CallRelationKind,
    pub(in crate::unit) endpoint: CallTargetKind,
}

impl<'a> ProofSiteCase<'a> {
    pub(in crate::unit) fn path(
        owner: Uuid,
        path: &'a [&'a str],
        relation: CallRelationKind,
        endpoint: CallTargetKind,
    ) -> Self {
        Self {
            owner,
            kind: CallSiteKind::Path,
            path,
            relation,
            endpoint,
        }
    }

    pub(in crate::unit) fn dynamic(
        owner: Uuid,
        path: &'a [&'a str],
        relation: CallRelationKind,
        endpoint: CallTargetKind,
    ) -> Self {
        Self {
            owner,
            kind: CallSiteKind::Dynamic,
            path,
            relation,
            endpoint,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct ProofMethodCase<'a> {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) method: &'a str,
    pub(in crate::unit) receiver: &'a CallReceiver,
    pub(in crate::unit) relation: CallRelationKind,
    pub(in crate::unit) endpoint: CallTargetKind,
}

impl<'a> ProofMethodCase<'a> {
    pub(in crate::unit) fn method(
        owner: Uuid,
        method: &'a str,
        receiver: &'a CallReceiver,
    ) -> Self {
        Self {
            owner,
            method,
            receiver,
            relation: CallRelationKind::Method,
            endpoint: CallTargetKind::Method,
        }
    }
}

pub(in crate::unit) fn assert_proof_site_cases(
    db: &Database,
    callers: &[CallCallerRow],
    cases: &[ProofSiteCase<'_>],
) -> Result<Vec<TargetProofSite>, DbError> {
    let mut expected = Vec::new();

    for case in cases {
        let context = db.call_context_for_owner(case.owner)?;
        let row = row_by_kind_path(&context, case.kind, case.path);
        let caller = caller_by_owner_kind_path(callers, case.owner, case.kind, case.path);
        assert_eq!(caller.site.id, row.site.id);
        assert_eq!(caller.target.relation, case.relation);
        assert_eq!(caller.target.source_kind, case.kind);
        assert_eq!(caller.target.target_kind, case.endpoint);
        expected.push(TargetProofSite {
            owner: case.owner,
            site: row.site.id,
        });
    }

    Ok(expected)
}

pub(in crate::unit) fn assert_proof_method_cases(
    db: &Database,
    callers: &[CallCallerRow],
    cases: &[ProofMethodCase<'_>],
) -> Result<Vec<TargetProofSite>, DbError> {
    let mut expected = Vec::new();

    for case in cases {
        let context = db.call_context_for_owner(case.owner)?;
        let row = row_by_method_receiver(&context, case.method, case.receiver);
        let caller =
            caller_by_owner_method_receiver(callers, case.owner, case.method, case.receiver);
        assert_eq!(caller.site.id, row.site.id);
        assert_eq!(caller.target.relation, case.relation);
        assert_eq!(caller.target.source_kind, CallSiteKind::Method);
        assert_eq!(caller.target.target_kind, case.endpoint);
        expected.push(TargetProofSite {
            owner: case.owner,
            site: row.site.id,
        });
    }

    Ok(expected)
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
