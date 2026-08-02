use super::*;

#[derive(Clone, Copy)]
pub(in crate::unit) struct ResolvedDynamicContextCase {
    pub(in crate::unit) owner: &'static str,
    pub(in crate::unit) path: &'static [&'static str],
    pub(in crate::unit) expected_rows: usize,
}

pub(in crate::unit) fn assert_resolved_dynamic_context_cases(
    db: &Database,
    target: Uuid,
    cases: &[ResolvedDynamicContextCase],
) -> Result<(), DbError> {
    for case in cases {
        let owner = function_id_by_name(db, case.owner)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            case.expected_rows,
            "{} context rows: {context:#?}",
            case.owner
        );

        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, case.path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.site.receiver, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetlessDynamicContextCase {
    pub(in crate::unit) owner: &'static str,
    pub(in crate::unit) path: Option<&'static [&'static str]>,
    pub(in crate::unit) status: CallStatusKind,
}

impl TargetlessDynamicContextCase {
    pub(in crate::unit) fn unsupported(owner: &'static str) -> Self {
        Self {
            owner,
            path: None,
            status: CallStatusKind::Unsupported,
        }
    }

    pub(in crate::unit) fn unsupported_path(
        owner: &'static str,
        path: &'static [&'static str],
    ) -> Self {
        Self {
            owner,
            path: Some(path),
            status: CallStatusKind::Unsupported,
        }
    }
}

pub(in crate::unit) fn assert_targetless_dynamic_context_cases(
    db: &Database,
    cases: &[TargetlessDynamicContextCase],
) -> Result<(), DbError> {
    for case in cases {
        let owner = function_id_by_name(db, case.owner)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{} context rows: {context:#?}",
            case.owner
        );

        assert_targetless_row(
            &context,
            owner,
            TargetlessRowCase::dynamic(case.path, case.status, case.owner),
        );
    }

    Ok(())
}
