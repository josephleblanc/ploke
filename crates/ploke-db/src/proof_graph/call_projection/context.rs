use crate::{
    DbError,
    call_graph::{
        CallCallerRow, CallContextRow, CallRelationKind, CallSiteKind, CallStatusKind,
        CallTargetKind,
    },
};

pub(super) fn validate_call_context(row: &CallContextRow) -> Result<(), DbError> {
    match row.status.status {
        CallStatusKind::Resolved if row.targets.len() != 1 => Err(DbError::Cozo(format!(
            "resolved call site {} expected exactly one target, found {}",
            row.site.id,
            row.targets.len()
        ))),
        CallStatusKind::Unresolved | CallStatusKind::External | CallStatusKind::Unsupported
            if !row.targets.is_empty() =>
        {
            Err(DbError::Cozo(format!(
                "non-resolved call site {} has local call_relation targets: {:?}",
                row.site.id, row.targets
            )))
        }
        CallStatusKind::Ambiguous
            if !row.targets.is_empty() && !is_ambiguous_dynamic_candidate_row(row) =>
        {
            Err(DbError::Cozo(format!(
                "non-resolved call site {} has local call_relation targets: {:?}",
                row.site.id, row.targets
            )))
        }
        _ => Ok(()),
    }
}

pub(super) fn caller_context_row(row: CallCallerRow) -> CallContextRow {
    CallContextRow {
        site: row.site,
        status: row.status,
        targets: vec![row.target],
    }
}

fn is_ambiguous_dynamic_candidate_row(row: &CallContextRow) -> bool {
    row.site.kind == CallSiteKind::Dynamic
        && row.targets.iter().all(|target| {
            target.relation == CallRelationKind::DynamicFunction
                && target.source_kind == CallSiteKind::Dynamic
                && target.target_kind == CallTargetKind::Function
        })
}
