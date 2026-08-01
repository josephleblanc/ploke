use std::{fs, path::PathBuf};

use ploke_llm::ProviderAttemptTimeline;
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
    pub(super) provider_attempts: Vec<ProviderAttemptRow>,
}

#[derive(Debug, Clone)]
pub(super) struct ProviderAttemptRow {
    pub(super) provider_attempt_id: String,
    pub(super) campaign_id: Option<String>,
    pub(super) timeline: ProviderAttemptTimeline,
    pub(super) source_log_ref: String,
    pub(super) source_event_index: i64,
    pub(super) recorded_at: Option<String>,
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
    let mut provider_attempts = Vec::new();
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
        let trace = trace_event_row_from_value(
            &value,
            campaign_id.as_deref(),
            &log.log_ref_id,
            line_index,
            line,
        )?;
        if let Some(provider_attempt) = provider_attempt_row(&value, &trace)? {
            provider_attempts.push(provider_attempt);
        }
        traces.push(trace);
    }

    Ok(ParsedObservationJsonl {
        log,
        traces,
        provider_attempts,
    })
}

fn provider_attempt_row(
    value: &serde_json::Value,
    trace: &EvalTraceEventRow,
) -> Result<Option<ProviderAttemptRow>, EvalStoreError> {
    if trace.target != "chat_http" || trace.event_name.as_deref() != Some("provider_attempt") {
        return Ok(None);
    }
    let timeline =
        serde_json::from_value::<ProviderAttemptTimeline>(value.clone()).map_err(|source| {
            EvalStoreError::Validation {
                field: "observation_jsonl.provider_attempt",
                detail: format!(
                    "provider_attempt event {} is not a valid ProviderAttemptTimeline: {source}",
                    trace.source_event_index.unwrap_or_default()
                ),
            }
        })?;
    let source_log_ref =
        trace
            .source_log_ref
            .clone()
            .ok_or_else(|| EvalStoreError::Validation {
                field: "provider_attempt.source_log_ref",
                detail: "provider attempt projection requires its source log reference".to_string(),
            })?;
    let source_event_index =
        trace
            .source_event_index
            .ok_or_else(|| EvalStoreError::Validation {
                field: "provider_attempt.source_event_index",
                detail: "provider attempt projection requires its source event index".to_string(),
            })?;
    Ok(Some(ProviderAttemptRow {
        provider_attempt_id: trace.trace_event_id.clone(),
        campaign_id: trace.campaign_id.clone(),
        timeline,
        source_log_ref,
        source_event_index,
        recorded_at: trace.recorded_at.clone(),
    }))
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
        outcome: optional_string_field(object, "outcome")
            .or_else(|| optional_string_field(object, "transport_outcome")),
        duration_ms: optional_i64_field(object, "duration_ms")
            .or_else(|| optional_i64_field(object, "elapsed_ms")),
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
