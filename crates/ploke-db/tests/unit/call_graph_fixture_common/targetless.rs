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

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetlessMethodCase<'a> {
    pub(in crate::unit) method: &'a str,
    pub(in crate::unit) receiver: &'a CallReceiver,
    pub(in crate::unit) args: Option<u32>,
    pub(in crate::unit) generics: Option<u32>,
    pub(in crate::unit) status: CallStatusKind,
    pub(in crate::unit) resolution: Option<CallResolutionKind>,
    pub(in crate::unit) label: &'a str,
}

impl<'a> TargetlessMethodCase<'a> {
    pub(in crate::unit) fn method(
        method: &'a str,
        receiver: &'a CallReceiver,
        status: CallStatusKind,
        label: &'a str,
    ) -> Self {
        Self {
            method,
            receiver,
            args: Some(0),
            generics: Some(0),
            status,
            resolution: None,
            label,
        }
    }
}

pub(in crate::unit) fn assert_targetless_method_row<'a>(
    context: &'a [CallContextRow],
    owner: Uuid,
    case: TargetlessMethodCase<'_>,
) -> &'a CallContextRow {
    let row = row_by_method_receiver(context, case.method, case.receiver);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, case.args);
    assert_eq!(row.site.generic_arg_count, case.generics);
    assert_eq!(row.status.status, case.status);
    assert_eq!(row.status.resolution, case.resolution);
    assert!(
        row.targets.is_empty(),
        "{} targetless method row must not fabricate targets: {row:#?}",
        case.label
    );
    row
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetlessMacroCase<'a> {
    pub(in crate::unit) name: &'a str,
    pub(in crate::unit) status: CallStatusKind,
    pub(in crate::unit) resolution: Option<CallResolutionKind>,
    pub(in crate::unit) label: &'a str,
}

impl<'a> TargetlessMacroCase<'a> {
    pub(in crate::unit) fn macro_call(name: &'a str, label: &'a str) -> Self {
        Self {
            name,
            status: CallStatusKind::Unsupported,
            resolution: None,
            label,
        }
    }
}

pub(in crate::unit) fn assert_targetless_macro_row<'a>(
    context: &'a [CallContextRow],
    owner: Uuid,
    case: TargetlessMacroCase<'_>,
) -> &'a CallContextRow {
    let matches = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Macro
                && row.site.macro_name.as_deref() == Some(case.name)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one targetless macro row {} for {}; context rows: {context:#?}",
        case.name,
        case.label
    );

    let row = matches[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Macro);
    assert_eq!(row.site.macro_name.as_deref(), Some(case.name));
    assert_eq!(row.site.path, None);
    assert_eq!(row.site.receiver, None);
    assert_eq!(row.site.arg_count, None);
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, case.status);
    assert_eq!(row.status.resolution, case.resolution);
    assert!(
        row.targets.is_empty(),
        "{} targetless macro row must not fabricate targets: {row:#?}",
        case.label
    );
    row
}
