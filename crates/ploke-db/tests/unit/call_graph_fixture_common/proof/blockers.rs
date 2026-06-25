use super::*;

#[derive(Clone, Copy)]
pub(in crate::unit) struct BlockerProofSite {
    pub(in crate::unit) site: Uuid,
    pub(in crate::unit) span: (u32, u32),
    pub(in crate::unit) blocker_reason: &'static str,
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetlessBlockerCase<'a> {
    pub(in crate::unit) row: TargetlessRowCase<'a>,
    pub(in crate::unit) blocker_reason: &'static str,
}

pub(in crate::unit) fn assert_projected_blockers(
    db: &Database,
    expected: &mut Vec<BlockerProofSite>,
    owner_name: &str,
    blockers: &[TargetlessBlockerCase<'_>],
) -> Result<(), DbError> {
    let owner = function_id_by_name(db, owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        blockers.len(),
        "{owner_name} proof context rows: {context:#?}"
    );

    for blocker in blockers {
        let row = assert_targetless_row(&context, owner, blocker.row);
        expected.push(BlockerProofSite {
            site: row.site.id,
            span: row.site.span,
            blocker_reason: blocker.blocker_reason,
        });
    }

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, blockers.len() * 2);
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
