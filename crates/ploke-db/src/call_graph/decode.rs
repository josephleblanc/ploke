use cozo::{DataValue, Num};

use crate::{
    DbError,
    database::{to_string, to_string_list, to_uuid},
};

use super::{
    CallContextRow, CallReceiver, CallRelationKind, CallResolutionKind, CallResolutionRow,
    CallSiteKind, CallSiteRow, CallStatusKind, CallTargetKind, CallTargetRow,
};

pub(super) fn decode_site(row: &[DataValue]) -> Result<CallSiteRow, DbError> {
    let site = CallSiteRow {
        id: to_uuid(&row[0])?,
        owner_id: to_uuid(&row[1])?,
        kind: CallSiteKind::from_str(&to_string(&row[2])?)?,
        span: span_pair(&row[3])?,
        cfgs: to_string_list(&row[4])?,
        path: optional_string_list(&row[5])?,
        method: optional_string(&row[6])?,
        macro_name: optional_string(&row[7])?,
        receiver: CallReceiver::from_parts(&row[8], &row[9])?,
        arg_count: optional_index(&row[10])?,
        generic_arg_count: optional_index(&row[11])?,
    };
    validate_call_site_shape(&site)?;
    Ok(site)
}

fn validate_call_site_shape(site: &CallSiteRow) -> Result<(), DbError> {
    let valid = match site.kind {
        CallSiteKind::Path => {
            non_empty_path(site.path.as_deref())
                && site.method.is_none()
                && site.macro_name.is_none()
                && site.receiver.is_none()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_some()
        }
        CallSiteKind::Method => {
            site.path.is_none()
                && non_empty_string(site.method.as_deref())
                && site.macro_name.is_none()
                && site.receiver.is_some()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_some()
        }
        CallSiteKind::Dynamic => {
            optional_non_empty_path(site.path.as_deref())
                && site.method.is_none()
                && site.macro_name.is_none()
                && site.receiver.is_none()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_none()
        }
        CallSiteKind::Macro => {
            site.path.is_none()
                && site.method.is_none()
                && non_empty_string(site.macro_name.as_deref())
                && site.receiver.is_none()
                && site.arg_count.is_none()
                && site.generic_arg_count.is_none()
        }
    };

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed {:?} call_site {}",
            site.kind, site.id
        )))
    }
}

fn non_empty_path(path: Option<&[String]>) -> bool {
    path.is_some_and(|path| !path.is_empty() && path.iter().all(|segment| !segment.is_empty()))
}

fn optional_non_empty_path(path: Option<&[String]>) -> bool {
    path.is_none_or(|path| !path.is_empty() && path.iter().all(|segment| !segment.is_empty()))
}

fn non_empty_string(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

pub(super) fn decode_target(row: &[DataValue]) -> Result<CallTargetRow, DbError> {
    Ok(CallTargetRow {
        site_id: to_uuid(&row[0])?,
        target_id: to_uuid(&row[1])?,
        relation: CallRelationKind::from_str(&to_string(&row[2])?)?,
        source_kind: CallSiteKind::from_str(&to_string(&row[3])?)?,
        target_kind: CallTargetKind::from_str(&to_string(&row[4])?)?,
    })
}

pub(super) fn decode_resolution(row: &[DataValue]) -> Result<CallResolutionRow, DbError> {
    let status = CallResolutionRow {
        site_id: to_uuid(&row[0])?,
        site_kind: CallSiteKind::from_str(&to_string(&row[1])?)?,
        status: CallStatusKind::from_str(&to_string(&row[2])?)?,
        resolution: optional_resolution(&row[3])?,
    };
    validate_resolution_shape(&status)?;
    Ok(status)
}

fn validate_resolution_shape(status: &CallResolutionRow) -> Result<(), DbError> {
    let valid = match status.status {
        CallStatusKind::Resolved => status.resolution == Some(CallResolutionKind::LocalExact),
        CallStatusKind::Unresolved
        | CallStatusKind::Ambiguous
        | CallStatusKind::External
        | CallStatusKind::Unsupported => status.resolution.is_none(),
    };

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "call_resolution_status resolution_kind {:?} is invalid for {:?} call site {}",
            status.resolution, status.status, status.site_id
        )))
    }
}

pub(super) fn validate_owner_context_targets(row: &CallContextRow) -> Result<(), DbError> {
    if row.status.status == CallStatusKind::Resolved && row.targets.len() != 1 {
        return Err(DbError::Cozo(format!(
            "resolved call site {} expected exactly one target, found {}",
            row.site.id,
            row.targets.len()
        )));
    }
    Ok(())
}

fn optional_string(value: &DataValue) -> Result<Option<String>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(to_string(other)?)),
    }
}

fn optional_string_list(value: &DataValue) -> Result<Option<Vec<String>>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(to_string_list(other)?)),
    }
}

fn optional_index(value: &DataValue) -> Result<Option<u32>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(required_index(other)?)),
    }
}

fn optional_resolution(value: &DataValue) -> Result<Option<CallResolutionKind>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(CallResolutionKind::from_str(&to_string(other)?)?)),
    }
}

fn required_index(value: &DataValue) -> Result<u32, DbError> {
    match value {
        DataValue::Num(Num::Int(value)) => u32::try_from(*value)
            .map_err(|err| DbError::Cozo(format!("call graph integer out of u32 range: {err}"))),
        other => Err(DbError::Cozo(format!(
            "expected call graph integer, found {other:?}"
        ))),
    }
}

fn span_pair(value: &DataValue) -> Result<(u32, u32), DbError> {
    match value {
        DataValue::List(items) if items.len() == 2 => {
            Ok((required_index(&items[0])?, required_index(&items[1])?))
        }
        other => Err(DbError::Cozo(format!(
            "expected call graph span pair, found {other:?}"
        ))),
    }
}
