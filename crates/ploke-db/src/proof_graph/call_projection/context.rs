use crate::{
    DbError,
    call_graph::{CallContextRow, CallRelationKind, CallSiteKind, CallStatusKind, CallTargetKind},
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
            if !row.targets.is_empty() && !is_ambiguous_candidate_row(row) =>
        {
            Err(DbError::Cozo(format!(
                "non-resolved call site {} has local call_relation targets: {:?}",
                row.site.id, row.targets
            )))
        }
        _ => Ok(()),
    }
}

fn is_ambiguous_candidate_row(row: &CallContextRow) -> bool {
    is_ambiguous_dynamic_candidate_row(row) || is_ambiguous_path_candidate_row(row)
}

fn is_ambiguous_dynamic_candidate_row(row: &CallContextRow) -> bool {
    row.site.kind == CallSiteKind::Dynamic
        && row
            .targets
            .iter()
            .all(is_ambiguous_dynamic_candidate_target)
}

fn is_ambiguous_dynamic_candidate_target(target: &crate::call_graph::CallTargetRow) -> bool {
    target.source_kind == CallSiteKind::Dynamic
        && matches!(
            (target.relation, target.target_kind),
            (CallRelationKind::DynamicFunction, CallTargetKind::Function)
                | (CallRelationKind::DynamicClosure, CallTargetKind::Closure)
        )
}

fn is_ambiguous_path_candidate_row(row: &CallContextRow) -> bool {
    row.site.kind == CallSiteKind::Path && row.targets.iter().all(is_ambiguous_path_candidate)
}

fn is_ambiguous_path_candidate(target: &crate::call_graph::CallTargetRow) -> bool {
    target.source_kind == CallSiteKind::Path
        && matches!(
            (target.relation, target.target_kind),
            (CallRelationKind::Function, CallTargetKind::Function)
                | (CallRelationKind::Closure, CallTargetKind::Closure)
        )
}
