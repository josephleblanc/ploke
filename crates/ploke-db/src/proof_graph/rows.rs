use cozo::DataValue;

use super::ProofGraphContextRow;
use crate::DbError;

#[derive(Debug, Clone)]
pub(super) struct ProofFactRow {
    pub(super) fact_id: String,
    pub(super) kind: String,
    pub(super) evidence_use: Option<String>,
    pub(super) build_domain_id: Option<String>,
    pub(super) call_site_id: Option<String>,
    pub(super) call_edge_id: Option<String>,
    pub(super) caller_def_id: Option<String>,
    pub(super) callee_def_id: Option<String>,
    pub(super) resolution_state: Option<String>,
    pub(super) source_file: Option<String>,
    pub(super) start_byte: Option<u32>,
    pub(super) end_byte: Option<u32>,
    pub(super) line_start: Option<u32>,
    pub(super) line_end: Option<u32>,
    pub(super) effect_class: Option<String>,
    pub(super) blocker_reason: Option<String>,
    pub(super) status: Option<String>,
    pub(super) detail: Option<String>,
}

impl ProofFactRow {
    pub(super) fn from_data_values(row: &[DataValue]) -> Result<Self, DbError> {
        Ok(Self {
            fact_id: required_string(row, 0, "fact_id")?,
            kind: required_string(row, 1, "kind")?,
            evidence_use: optional_string(row, 3),
            build_domain_id: optional_string(row, 4),
            call_site_id: optional_string(row, 5),
            call_edge_id: optional_string(row, 6),
            caller_def_id: optional_string(row, 7),
            callee_def_id: optional_string(row, 8),
            resolution_state: optional_string(row, 9),
            source_file: optional_string(row, 10),
            start_byte: optional_u32(row, 11, "start_byte")?,
            end_byte: optional_u32(row, 12, "end_byte")?,
            line_start: optional_u32(row, 13, "line_start")?,
            line_end: optional_u32(row, 14, "line_end")?,
            effect_class: optional_string(row, 15),
            blocker_reason: optional_string(row, 16),
            status: optional_string(row, 17),
            detail: optional_string(row, 18),
        })
    }

    pub(super) fn matches_query(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        [
            Some(self.fact_id.as_str()),
            Some(self.kind.as_str()),
            self.evidence_use.as_deref(),
            self.build_domain_id.as_deref(),
            self.call_site_id.as_deref(),
            self.call_edge_id.as_deref(),
            self.caller_def_id.as_deref(),
            self.callee_def_id.as_deref(),
            self.resolution_state.as_deref(),
            self.source_file.as_deref(),
            self.effect_class.as_deref(),
            self.blocker_reason.as_deref(),
            self.status.as_deref(),
            self.detail.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| value.to_ascii_lowercase().contains(query))
    }
}

impl From<ProofFactRow> for ProofGraphContextRow {
    fn from(row: ProofFactRow) -> Self {
        Self {
            fact_id: row.fact_id,
            kind: row.kind,
            call_site_id: row.call_site_id,
            caller_def_id: row.caller_def_id,
            callee_def_id: row.callee_def_id,
            evidence_use: row.evidence_use,
            blocker_reason: row.blocker_reason,
            source_file: row.source_file,
            line_start: row.line_start,
            detail: row.detail,
        }
    }
}

pub(super) fn string_value(value: Option<String>) -> DataValue {
    value
        .map(|value| DataValue::Str(value.into()))
        .unwrap_or(DataValue::Null)
}

pub(super) fn int_value(value: Option<u32>) -> DataValue {
    value
        .map(|value| DataValue::from(i64::from(value)))
        .unwrap_or(DataValue::Null)
}

fn optional_string(row: &[DataValue], index: usize) -> Option<String> {
    row.get(index)
        .and_then(DataValue::get_str)
        .map(ToOwned::to_owned)
}

fn required_string(row: &[DataValue], index: usize, field: &str) -> Result<String, DbError> {
    optional_string(row, index)
        .ok_or_else(|| DbError::Cozo(format!("proof graph row missing string field {field}")))
}

fn optional_u32(row: &[DataValue], index: usize, field: &str) -> Result<Option<u32>, DbError> {
    row.get(index)
        .and_then(DataValue::get_int)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| DbError::Cozo(format!("proof graph field {field} is out of u32 range")))
}
