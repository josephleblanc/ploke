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
