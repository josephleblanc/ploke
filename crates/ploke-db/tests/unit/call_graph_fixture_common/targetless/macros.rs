use super::*;

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
