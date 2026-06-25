use super::*;

pub(in crate::unit) fn row_by_path<'a>(
    context: &'a [CallContextRow],
    expected: &[&str],
) -> &'a CallContextRow {
    let expected = path(expected);
    let matches = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path && row.site.path.as_ref() == Some(&expected)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one path row {expected:?}; context rows: {context:#?}"
    );
    matches[0]
}

pub(in crate::unit) fn row_by_method_receiver<'a>(
    context: &'a [CallContextRow],
    method: &str,
    receiver: &CallReceiver,
) -> &'a CallContextRow {
    let matches = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some(method)
                && row.site.receiver.as_ref() == Some(receiver)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one method row {method} with receiver {receiver:?}; context rows: {context:#?}"
    );
    matches[0]
}

pub(in crate::unit) fn row_by_kind_path<'a>(
    context: &'a [CallContextRow],
    kind: CallSiteKind,
    expected: &[&str],
) -> &'a CallContextRow {
    let expected = path(expected);
    let matches = context
        .iter()
        .filter(|row| row.site.kind == kind && row.site.path.as_ref() == Some(&expected))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {kind:?} row {expected:?}; context rows: {context:#?}"
    );
    matches[0]
}

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

pub(in crate::unit) fn assert_resolved_target(
    row: &CallContextRow,
    target: Uuid,
    relation: CallRelationKind,
    source: CallSiteKind,
    target_kind: CallTargetKind,
) {
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, relation);
    assert_eq!(row.targets[0].source_kind, source);
    assert_eq!(row.targets[0].target_kind, target_kind);
}

pub(in crate::unit) fn assert_call_candidate(
    candidates: &[CallContextCandidate],
    node_id: Uuid,
    relation: CallContextRelation,
    call_site_id: Uuid,
    target_id: Uuid,
    message: &str,
) {
    assert!(
        candidates.iter().any(|candidate| {
            candidate.node_id == node_id
                && candidate.relation == relation
                && candidate.call_site_id == call_site_id
                && candidate.target_id == target_id
                && candidate.distance == 1
        }),
        "{message}; candidates: {candidates:#?}"
    );
}

pub(in crate::unit) fn assert_incoming_candidate(
    candidates: &[CallContextCandidate],
    owner_id: Uuid,
    call_site_id: Uuid,
    target_id: Uuid,
    message: &str,
) {
    assert_call_candidate(
        candidates,
        owner_id,
        CallContextRelation::IncomingCaller,
        call_site_id,
        target_id,
        message,
    );
}

pub(in crate::unit) fn assert_outgoing_candidate(
    candidates: &[CallContextCandidate],
    target_id: Uuid,
    call_site_id: Uuid,
    message: &str,
) {
    assert_call_candidate(
        candidates,
        target_id,
        CallContextRelation::OutgoingTarget,
        call_site_id,
        target_id,
        message,
    );
}

pub(in crate::unit) fn assert_incoming_candidates_for_target(
    candidates: &[CallContextCandidate],
    target_id: Uuid,
    message: &str,
) {
    assert!(
        candidates.iter().all(|candidate| {
            candidate.target_id == target_id
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 1
        }),
        "{message}: {candidates:#?}"
    );
}

pub(in crate::unit) fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}
