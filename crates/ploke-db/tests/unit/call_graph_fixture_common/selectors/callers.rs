use super::*;

pub(in crate::unit) fn caller_by_owner_kind_path<'a>(
    callers: &'a [CallCallerRow],
    owner: Uuid,
    kind: CallSiteKind,
    expected: &[&str],
) -> &'a CallCallerRow {
    let expected = path(expected);
    let matches = callers
        .iter()
        .filter(|row| {
            row.site.owner_id == owner
                && row.site.kind == kind
                && row.site.path.as_ref() == Some(&expected)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one incoming {kind:?} caller row {expected:?}; caller rows: {callers:#?}"
    );
    matches[0]
}

pub(in crate::unit) fn caller_by_owner_method_receiver<'a>(
    callers: &'a [CallCallerRow],
    owner: Uuid,
    method: &str,
    receiver: &CallReceiver,
) -> &'a CallCallerRow {
    let matches = callers
        .iter()
        .filter(|row| {
            row.site.owner_id == owner
                && row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some(method)
                && row.site.receiver.as_ref() == Some(receiver)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one incoming method caller {method} with receiver {receiver:?}; caller rows: {callers:#?}"
    );
    matches[0]
}

pub(in crate::unit) fn assert_callers_for_target(
    callers: &[CallCallerRow],
    target: Uuid,
    min_count: usize,
    label: &str,
) {
    assert!(
        callers.len() >= min_count,
        "{label} should have at least {min_count} incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "{label} caller query returned a mismatched target edge: {callers:#?}"
    );
}

pub(in crate::unit) fn assert_min_resolved_callers(
    callers: &[CallCallerRow],
    min_count: usize,
    label: &str,
) {
    let resolved = callers
        .iter()
        .filter(|caller| {
            caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)
        })
        .count();
    assert!(
        resolved >= min_count,
        "{label} should have at least {min_count} resolved incoming callers: {callers:#?}"
    );
}

pub(in crate::unit) fn assert_resolved_callers_for_target(
    callers: &[CallCallerRow],
    target: Uuid,
    min_count: usize,
    label: &str,
) {
    assert_callers_for_target(callers, target, min_count, label);
    assert!(
        callers.iter().all(|caller| {
            caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)
        }),
        "{label} incoming callers should preserve resolved statuses: {callers:#?}"
    );
}
