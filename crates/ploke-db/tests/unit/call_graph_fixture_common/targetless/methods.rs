use super::*;

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
