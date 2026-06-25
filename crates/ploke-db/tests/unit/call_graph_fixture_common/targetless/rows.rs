use super::*;

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
        Self::dynamic_args(path, 0, status, label)
    }

    pub(in crate::unit) fn dynamic_args(
        path: Option<&'a [&'a str]>,
        args: u32,
        status: CallStatusKind,
        label: &'a str,
    ) -> Self {
        Self {
            kind: CallSiteKind::Dynamic,
            path,
            args: Some(args),
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

pub(in crate::unit) struct TargetlessOwnerCase<'a> {
    pub(in crate::unit) owner: &'a str,
    pub(in crate::unit) rows: &'a [TargetlessRowCase<'a>],
}

pub(in crate::unit) fn assert_targetless_owner_cases(
    db: &Database,
    cases: &[TargetlessOwnerCase<'_>],
) -> Result<(), DbError> {
    for case in cases {
        let owner = function_id_by_name(db, case.owner)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            case.rows.len(),
            "{} context rows: {context:#?}",
            case.owner
        );

        for row in case.rows {
            assert_targetless_row(&context, owner, *row);
        }
    }

    Ok(())
}
