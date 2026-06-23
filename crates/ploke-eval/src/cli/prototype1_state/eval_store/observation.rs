use std::{fs, path::PathBuf};

use ploke_records::ids::CampaignId;

use super::{
    error::EvalStoreError,
    evidence::{
        EvalLogRefRow, EvalTraceEventRow, LogRefEvidence, STORE_SCOPE, hash_parts, log_ref_row,
        sha256_bytes, usize_to_i64,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObservationJsonlImport {
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone)]
pub(super) struct ParsedObservationJsonl {
    pub(super) log: EvalLogRefRow,
    pub(super) traces: Vec<EvalTraceEventRow>,
}

pub(super) fn parse_observation_jsonl(
    import: ObservationJsonlImport,
) -> Result<ParsedObservationJsonl, EvalStoreError> {
    let bytes = fs::read(&import.path).map_err(|source| EvalStoreError::Io {
        phase: "observation_jsonl.read",
        path: import.path.clone(),
        source,
    })?;
    let text = String::from_utf8(bytes.clone()).map_err(|source| EvalStoreError::Validation {
        field: "observation_jsonl.utf8",
        detail: source.to_string(),
    })?;
    let content_sha256 = sha256_bytes(&bytes);
    let byte_len = i64::try_from(bytes.len()).map_err(|_| EvalStoreError::Validation {
        field: "log_ref.byte_len",
        detail: format!("observation log '{}' is too large", import.path.display()),
    })?;
    let log = log_ref_row(LogRefEvidence {
        campaign_id: import.campaign_id.clone(),
        runtime_id: None,
        store_scope: STORE_SCOPE.to_string(),
        log_kind: "observation_jsonl".to_string(),
        source_ref: import.path.display().to_string(),
        byte_start: Some(0),
        byte_len: Some(byte_len),
        content_sha256: Some(content_sha256),
        sensitivity: Some("internal_diagnostic".to_string()),
        recorded_at: None,
    })?;

    let campaign_id = import.campaign_id.as_ref().map(|id| id.to_string());
    let mut traces = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            return Err(EvalStoreError::Validation {
                field: "observation_jsonl.line",
                detail: format!(
                    "blank JSONL line {} in '{}'",
                    line_index + 1,
                    import.path.display()
                ),
            });
        }
        let value = serde_json::from_str::<serde_json::Value>(line).map_err(|source| {
            EvalStoreError::Validation {
                field: "observation_jsonl.line",
                detail: format!(
                    "invalid JSONL line {} in '{}': {source}",
                    line_index + 1,
                    import.path.display()
                ),
            }
        })?;
        traces.push(trace_event_row_from_value(
            &value,
            campaign_id.as_deref(),
            &log.log_ref_id,
            line_index,
            line,
        )?);
    }

    Ok(ParsedObservationJsonl { log, traces })
}

fn trace_event_row_from_value(
    value: &serde_json::Value,
    default_campaign_id: Option<&str>,
    log_ref_id: &str,
    source_event_index: usize,
    raw_line: &str,
) -> Result<EvalTraceEventRow, EvalStoreError> {
    let object = value
        .as_object()
        .ok_or_else(|| EvalStoreError::Validation {
            field: "observation_jsonl.line",
            detail: "observation JSONL line must be an object".to_string(),
        })?;
    let target = required_string_field(object, "target")?;
    let level = required_string_field(object, "level")?;
    let index = usize_to_i64(source_event_index, "trace.source_event_index")?;
    let content_sha256 = sha256_bytes(raw_line.as_bytes());
    let trace_event_id = hash_parts(&[
        "p1.eval.trace_event.v1",
        log_ref_id,
        &index.to_string(),
        &content_sha256,
    ]);
    Ok(EvalTraceEventRow {
        trace_event_id,
        campaign_id: optional_string_field(object, "campaign_id")
            .or_else(|| default_campaign_id.map(str::to_string)),
        parent_id: optional_string_field(object, "parent_id"),
        runtime_id: optional_string_field(object, "runtime_id"),
        node_id: optional_string_field(object, "node_id"),
        generation: optional_i64_field(object, "generation"),
        branch_id: optional_string_field(object, "branch_id"),
        role: optional_string_field(object, "role"),
        pipeline: optional_string_field(object, "pipeline"),
        stage: optional_string_field(object, "stage")
            .or_else(|| optional_string_field(object, "phase")),
        authority: optional_string_field(object, "authority"),
        transition: optional_string_field(object, "transition"),
        event_name: optional_string_field(object, "event"),
        span_name: span_name_field(object),
        target,
        level,
        outcome: optional_string_field(object, "outcome"),
        duration_ms: optional_i64_field(object, "duration_ms"),
        record_access: optional_string_field(object, "record_access"),
        record_kind: optional_string_field(object, "record_kind"),
        record_path: optional_string_field(object, "record_path"),
        record_index: optional_i64_field(object, "record_index"),
        record_count: optional_i64_field(object, "record_count"),
        program: optional_string_field(object, "program"),
        exit_code: optional_i64_field(object, "exit_code"),
        error: optional_string_field(object, "error"),
        source_log_ref: Some(log_ref_id.to_string()),
        source_event_index: Some(index),
        recorded_at: optional_string_field(object, "timestamp")
            .or_else(|| optional_string_field(object, "time")),
    })
}

fn required_string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<String, EvalStoreError> {
    optional_string_field(object, field).ok_or_else(|| EvalStoreError::Validation {
        field,
        detail: "required observation JSONL string field is missing".to_string(),
    })
}

fn optional_string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    match object.get(field)? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn optional_i64_field(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Option<i64> {
    match object.get(field)? {
        serde_json::Value::Number(value) => value.as_i64(),
        serde_json::Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn span_name_field(object: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    optional_string_field(object, "span_name").or_else(|| {
        object
            .get("span")?
            .get("name")?
            .as_str()
            .map(str::to_string)
    })
}
