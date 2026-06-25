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

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetlessRowCase<'a> {
    pub(in crate::unit) kind: CallSiteKind,
    pub(in crate::unit) path: Option<&'a [&'a str]>,
    pub(in crate::unit) args: Option<u32>,
    pub(in crate::unit) generics: Option<u32>,
    pub(in crate::unit) status: CallStatusKind,
    pub(in crate::unit) resolution: Option<CallResolutionKind>,
    pub(in crate::unit) label: &'a str,
}

impl<'a> TargetlessRowCase<'a> {
    pub(in crate::unit) fn path(
        path: &'a [&'a str],
        args: u32,
        status: CallStatusKind,
        label: &'a str,
    ) -> Self {
        Self {
            kind: CallSiteKind::Path,
            path: Some(path),
            args: Some(args),
            generics: Some(0),
            status,
            resolution: None,
            label,
        }
    }

    pub(in crate::unit) fn dynamic(
        path: Option<&'a [&'a str]>,
        status: CallStatusKind,
        label: &'a str,
    ) -> Self {
        Self {
            kind: CallSiteKind::Dynamic,
            path,
            args: Some(0),
            generics: None,
            status,
            resolution: None,
            label,
        }
    }
}

pub(in crate::unit) fn assert_targetless_row<'a>(
    context: &'a [CallContextRow],
    owner: Uuid,
    case: TargetlessRowCase<'_>,
) -> &'a CallContextRow {
    let expected = case.path.map(path);
    let matches = context
        .iter()
        .filter(|row| row.site.kind == case.kind && row.site.path.as_ref() == expected.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one targetless {:?} row {:?} for {}; context rows: {context:#?}",
        case.kind,
        expected,
        case.label
    );

    let row = matches[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, case.kind);
    assert_eq!(row.site.path.as_ref(), expected.as_ref());
    assert_eq!(row.site.arg_count, case.args);
    assert_eq!(row.site.generic_arg_count, case.generics);
    assert_eq!(row.status.status, case.status);
    assert_eq!(row.status.resolution, case.resolution);
    assert!(
        row.targets.is_empty(),
        "{} targetless row must not fabricate targets: {row:#?}",
        case.label
    );
    row
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

pub(in crate::unit) fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}
