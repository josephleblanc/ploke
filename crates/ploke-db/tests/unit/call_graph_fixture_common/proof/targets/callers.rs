use super::*;

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

pub(in crate::unit) fn expected_target_proof_count_with_binding_evidence(
    db: &Database,
    callers: &[CallCallerRow],
) -> Result<usize, DbError> {
    let mut count = expected_target_proof_count(callers);
    for caller in callers {
        let site = caller.site.id.to_string();
        let owner = caller.site.owner_id.to_string();
        let state = expected_resolution_state(caller.status.status);
        let rows = db.proof_binding_evidence_for_call_site(&site)?;
        for row in &rows {
            assert!(
                matches!(
                    row.binding_evidence_kind.as_str(),
                    "returned_callable" | "self_field_callable"
                ),
                "target proof count helper should only account for supported binding evidence kinds: {row:#?}"
            );
            assert_eq!(row.call_site_id, site);
            assert_eq!(row.caller_def_id, owner);
            assert_eq!(row.resolution_state, state);
        }
        count += rows.len();
    }
    Ok(count)
}

fn expected_resolution_state(status: CallStatusKind) -> &'static str {
    match status {
        CallStatusKind::Resolved => "resolved",
        CallStatusKind::Unresolved => "unresolved",
        CallStatusKind::Ambiguous => "ambiguous",
        CallStatusKind::External | CallStatusKind::Unsupported => "blocked",
    }
}
