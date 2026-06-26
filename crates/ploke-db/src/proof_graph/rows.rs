use cozo::DataValue;
use serde_json::Value;

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
    pub(super) resolved_def_id: Option<String>,
    pub(super) candidate_def_ids: Vec<String>,
    pub(super) external_summary_id: Option<String>,
    pub(super) authority_term: Option<String>,
    pub(super) source_file: Option<String>,
    pub(super) start_byte: Option<u32>,
    pub(super) end_byte: Option<u32>,
    pub(super) line_start: Option<u32>,
    pub(super) line_end: Option<u32>,
    pub(super) effect_class: Option<String>,
    pub(super) blocker_reason: Option<String>,
    pub(super) status: Option<String>,
    pub(super) detail: Option<String>,
    json_terms: Vec<String>,
}

impl ProofFactRow {
    pub(super) fn from_data_values(row: &[DataValue]) -> Result<Self, DbError> {
        let json = optional_json(row, 19);
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
            resolved_def_id: json.and_then(|value| json_string(value, "resolved_def_id")),
            candidate_def_ids: json
                .map(|value| json_string_array(value, "candidate_def_ids"))
                .unwrap_or_default(),
            external_summary_id: json.and_then(|value| json_string(value, "external_summary_id")),
            authority_term: json.and_then(|value| json_string(value, "authority_term")),
            source_file: optional_string(row, 10),
            start_byte: optional_u32(row, 11, "start_byte")?,
            end_byte: optional_u32(row, 12, "end_byte")?,
            line_start: optional_u32(row, 13, "line_start")?,
            line_end: optional_u32(row, 14, "line_end")?,
            effect_class: optional_string(row, 15),
            blocker_reason: optional_string(row, 16),
            status: optional_string(row, 17),
            detail: optional_string(row, 18),
            json_terms: json.map(json_search_terms).unwrap_or_default(),
        })
    }

    pub(super) fn matches_query(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let scalar_match = [
            Some(self.fact_id.as_str()),
            Some(self.kind.as_str()),
            self.evidence_use.as_deref(),
            self.build_domain_id.as_deref(),
            self.call_site_id.as_deref(),
            self.call_edge_id.as_deref(),
            self.caller_def_id.as_deref(),
            self.callee_def_id.as_deref(),
            self.resolution_state.as_deref(),
            self.resolved_def_id.as_deref(),
            self.external_summary_id.as_deref(),
            self.authority_term.as_deref(),
            self.source_file.as_deref(),
            self.effect_class.as_deref(),
            self.blocker_reason.as_deref(),
            self.status.as_deref(),
            self.detail.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| value.to_ascii_lowercase().contains(query));
        let candidate_match = self
            .candidate_def_ids
            .iter()
            .any(|value| value.to_ascii_lowercase().contains(query));
        let json_match = self
            .json_terms
            .iter()
            .any(|value| value.to_ascii_lowercase().contains(query));
        scalar_match || candidate_match || json_match
    }
}

impl From<ProofFactRow> for ProofGraphContextRow {
    fn from(row: ProofFactRow) -> Self {
        Self {
            fact_id: row.fact_id,
            kind: row.kind,
            build_domain_id: row.build_domain_id,
            call_site_id: row.call_site_id,
            call_edge_id: row.call_edge_id,
            caller_def_id: row.caller_def_id,
            callee_def_id: row.callee_def_id,
            resolution_state: row.resolution_state,
            resolved_def_id: row.resolved_def_id,
            candidate_def_ids: row.candidate_def_ids,
            external_summary_id: row.external_summary_id,
            authority_term: row.authority_term,
            evidence_use: row.evidence_use,
            source_file: row.source_file,
            start_byte: row.start_byte,
            end_byte: row.end_byte,
            line_start: row.line_start,
            line_end: row.line_end,
            effect_class: row.effect_class,
            blocker_reason: row.blocker_reason,
            status: row.status,
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

fn optional_json(row: &[DataValue], index: usize) -> Option<&Value> {
    match row.get(index) {
        Some(DataValue::Json(value)) => Some(&value.0),
        _ => None,
    }
}

fn json_string(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(ToOwned::to_owned)
}

fn json_string_array(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|candidate| candidate.as_str().map(ToOwned::to_owned))
        .collect()
}

fn json_search_terms(value: &Value) -> Vec<String> {
    let mut terms = Vec::new();
    collect_json_strings(value, &mut terms);
    terms
}

fn collect_json_strings(value: &Value, terms: &mut Vec<String>) {
    match value {
        Value::String(text) => terms.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_json_strings(item, terms);
            }
        }
        Value::Object(fields) => {
            for value in fields.values() {
                collect_json_strings(value, terms);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
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
