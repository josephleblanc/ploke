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

pub(in crate::unit) enum ResolvedProofCall<'a> {
    Path {
        path: &'a [&'a str],
        target: Uuid,
        relation: CallRelationKind,
        target_kind: CallTargetKind,
    },
    Method {
        method: &'a str,
        receiver: CallReceiver,
        target: Uuid,
    },
}

impl<'a> ResolvedProofCall<'a> {
    pub(in crate::unit) fn path(
        path: &'a [&'a str],
        target: Uuid,
        relation: CallRelationKind,
        target_kind: CallTargetKind,
    ) -> Self {
        Self::Path {
            path,
            target,
            relation,
            target_kind,
        }
    }

    pub(in crate::unit) fn method(method: &'a str, receiver: CallReceiver, target: Uuid) -> Self {
        Self::Method {
            method,
            receiver,
            target,
        }
    }
}

pub(in crate::unit) struct ResolvedProofCase<'a> {
    pub(in crate::unit) label: &'a str,
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) rows: usize,
    pub(in crate::unit) calls: Vec<ResolvedProofCall<'a>>,
}

pub(in crate::unit) fn resolved_proof_edges(
    db: &Database,
    domain: &str,
    cases: &[ResolvedProofCase<'_>],
) -> Result<Vec<OwnerProofEdge>, DbError> {
    let mut edges = Vec::new();

    for case in cases {
        let context = db.call_context_for_owner(case.owner)?;
        assert_eq!(
            context.len(),
            case.rows,
            "{} context rows: {context:#?}",
            case.label
        );

        for call in &case.calls {
            let (row, target, relation, kind, target_kind) = match call {
                ResolvedProofCall::Path {
                    path,
                    target,
                    relation,
                    target_kind,
                } => (
                    row_by_path(&context, path),
                    *target,
                    *relation,
                    CallSiteKind::Path,
                    *target_kind,
                ),
                ResolvedProofCall::Method {
                    method,
                    receiver,
                    target,
                } => (
                    row_by_method_receiver(&context, method, receiver),
                    *target,
                    CallRelationKind::Method,
                    CallSiteKind::Method,
                    CallTargetKind::Method,
                ),
            };

            assert_resolved_target(row, target, relation, kind, target_kind);
            edges.push(OwnerProofEdge {
                owner: case.owner,
                site: row.site.id,
                span: row.site.span,
                target,
            });
        }

        let expected = context
            .iter()
            .map(|row| 2 + row.targets.len())
            .sum::<usize>();
        let count = db.project_call_proof_facts_for_owner(case.owner, domain)?;
        assert_eq!(count, expected, "{} proof fact count", case.label);
    }

    Ok(edges)
}

pub(in crate::unit) fn assert_fixture_resolved_proofs(
    db: &Database,
    label: &str,
    cases: &[ResolvedProofCase<'_>],
) -> Result<(), DbError> {
    let expected = resolved_proof_edges(db, "bd:fixture-call-graph", cases)?;
    assert_owner_proof_edges(
        db,
        label,
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )
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
