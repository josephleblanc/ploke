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
