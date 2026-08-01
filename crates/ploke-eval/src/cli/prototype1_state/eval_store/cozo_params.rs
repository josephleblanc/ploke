use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_llm::{HttpBodyFailure, HttpSendFailure};

use super::{
    error::EvalStoreError,
    evidence::{
        EvalAttemptRow, EvalChannelMessageRow, EvalChannelReceiptRow, EvalImportEventRow,
        EvalInvocationRow, EvalLogRefRow, EvalRecordRefRow, EvalTraceEventRow,
        EvalTransitionEventRow,
    },
    observation::ProviderAttemptRow,
};

pub(super) fn transition_event_params(row: &EvalTransitionEventRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("event_id".to_string(), row.event_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("parent_id".to_string(), row.parent_id.clone().into());
    params.insert("runtime_id".to_string(), row.runtime_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("generation".to_string(), row.generation.into());
    params.insert("transition".to_string(), row.transition.clone().into());
    params.insert("phase".to_string(), row.phase.clone().into());
    params.insert("outcome".to_string(), row.outcome.clone().into());
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert(
        "producer_role".to_string(),
        row.producer_role.clone().into(),
    );
    params.insert(
        "visibility_scope".to_string(),
        row.visibility_scope.clone().into(),
    );
    params.insert("source_class".to_string(), row.source_class.clone().into());
    params.insert(
        "evidence_class".to_string(),
        row.evidence_class.clone().into(),
    );
    params.insert(
        "validation_status".to_string(),
        row.validation_status.clone().into(),
    );
    params.insert(
        "source_stream_id".to_string(),
        row.source_stream_id.clone().into(),
    );
    params.insert(
        "source_event_index".to_string(),
        row.source_event_index.into(),
    );
    params.insert("source_line".to_string(), row.source_line.into());
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert(
        "semantic_hash".to_string(),
        row.semantic_hash.clone().into(),
    );
    params.insert("recorded_at".to_string(), row.recorded_at.into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

pub(super) fn record_ref_params(row: &EvalRecordRefRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "record_ref_id".to_string(),
        row.record_ref_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("family".to_string(), row.family.clone().into());
    params.insert(
        "schema_version".to_string(),
        row.schema_version.clone().into(),
    );
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert(
        "producer_role".to_string(),
        row.producer_role.clone().into(),
    );
    params.insert("producer_id".to_string(), row.producer_id.clone().into());
    params.insert("source_class".to_string(), row.source_class.clone().into());
    params.insert(
        "evidence_class".to_string(),
        row.evidence_class.clone().into(),
    );
    params.insert(
        "visibility_scope".to_string(),
        row.visibility_scope.clone().into(),
    );
    params.insert(
        "validation_status".to_string(),
        row.validation_status.clone().into(),
    );
    params.insert(
        "source_stream_id".to_string(),
        row.source_stream_id.clone().into(),
    );
    params.insert(
        "source_event_index".to_string(),
        row.source_event_index.into(),
    );
    params.insert("source_line".to_string(), row.source_line.into());
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("payload_json".to_string(), row.payload_json.clone().into());
    params.insert("recorded_at".to_string(), row.recorded_at.into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

pub(super) fn log_ref_params(row: &EvalLogRefRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("log_ref_id".to_string(), row.log_ref_id.clone().into());
    params.insert(
        "campaign_id".to_string(),
        option_string_param(&row.campaign_id),
    );
    params.insert(
        "runtime_id".to_string(),
        option_string_param(&row.runtime_id),
    );
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert("log_kind".to_string(), row.log_kind.clone().into());
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert("byte_start".to_string(), option_i64_param(row.byte_start));
    params.insert("byte_len".to_string(), option_i64_param(row.byte_len));
    params.insert(
        "content_sha256".to_string(),
        option_string_param(&row.content_sha256),
    );
    params.insert(
        "sensitivity".to_string(),
        option_string_param(&row.sensitivity),
    );
    params.insert(
        "recorded_at".to_string(),
        option_string_param(&row.recorded_at),
    );
    params
}

pub(super) fn invocation_params(row: &EvalInvocationRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "invocation_id".to_string(),
        row.invocation_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("runtime_id".to_string(), row.runtime_id.clone().into());
    params.insert("role".to_string(), row.role.clone().into());
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert(
        "producer_role".to_string(),
        row.producer_role.clone().into(),
    );
    params.insert(
        "visibility_scope".to_string(),
        row.visibility_scope.clone().into(),
    );
    params.insert("source_class".to_string(), row.source_class.clone().into());
    params.insert(
        "evidence_class".to_string(),
        row.evidence_class.clone().into(),
    );
    params.insert(
        "validation_status".to_string(),
        row.validation_status.clone().into(),
    );
    params.insert(
        "invocation_path".to_string(),
        row.invocation_path.clone().into(),
    );
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("recorded_at".to_string(), row.recorded_at.clone().into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

pub(super) fn attempt_params(row: &EvalAttemptRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("attempt_id".to_string(), row.attempt_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("runtime_id".to_string(), row.runtime_id.clone().into());
    params.insert("role".to_string(), row.role.clone().into());
    params.insert("parent_id".to_string(), option_string_param(&row.parent_id));
    params.insert("node_id".to_string(), option_string_param(&row.node_id));
    params.insert(
        "invocation_id".to_string(),
        option_string_param(&row.invocation_id),
    );
    params.insert(
        "channel_id".to_string(),
        option_string_param(&row.channel_id),
    );
    params.insert(
        "artifact_id".to_string(),
        option_string_param(&row.artifact_id),
    );
    params.insert(
        "binary_ref".to_string(),
        option_string_param(&row.binary_ref),
    );
    params.insert(
        "started_at".to_string(),
        option_string_param(&row.started_at),
    );
    params.insert("status".to_string(), option_string_param(&row.status));
    params
}

pub(super) fn channel_message_params(row: &EvalChannelMessageRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "channel_message_id".to_string(),
        row.channel_message_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("runtime_id".to_string(), row.runtime_id.clone().into());
    params.insert("direction".to_string(), row.direction.clone().into());
    params.insert("message_kind".to_string(), row.message_kind.clone().into());
    params.insert("message_id".to_string(), row.message_id.clone().into());
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert(
        "producer_role".to_string(),
        row.producer_role.clone().into(),
    );
    params.insert(
        "visibility_scope".to_string(),
        row.visibility_scope.clone().into(),
    );
    params.insert("source_class".to_string(), row.source_class.clone().into());
    params.insert(
        "evidence_class".to_string(),
        row.evidence_class.clone().into(),
    );
    params.insert(
        "validation_status".to_string(),
        row.validation_status.clone().into(),
    );
    params.insert(
        "endpoint_path".to_string(),
        row.endpoint_path.clone().into(),
    );
    params.insert("cursor_offset".to_string(), row.cursor_offset.into());
    params.insert("bytes_written".to_string(), row.bytes_written.into());
    params.insert("body_hash".to_string(), row.body_hash.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert("recorded_at".to_string(), row.recorded_at.clone().into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

pub(super) fn channel_receipt_params(row: &EvalChannelReceiptRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("receipt_id".to_string(), row.receipt_id.clone().into());
    params.insert("channel_id".to_string(), row.channel_id.clone().into());
    params.insert("message_id".to_string(), row.message_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("runtime_id".to_string(), row.runtime_id.clone().into());
    params.insert(
        "observed_by".to_string(),
        option_string_param(&row.observed_by),
    );
    params.insert("direction".to_string(), row.direction.clone().into());
    params.insert(
        "validation_status".to_string(),
        row.validation_status.clone().into(),
    );
    params.insert(
        "imported_ref".to_string(),
        option_string_param(&row.imported_ref),
    );
    params.insert("observed_at".to_string(), row.observed_at.clone().into());
    params
}

pub(super) fn import_event_params(row: &EvalImportEventRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("import_id".to_string(), row.import_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("importer_id".to_string(), row.importer_id.clone().into());
    params.insert(
        "source_runtime_id".to_string(),
        option_string_param(&row.source_runtime_id),
    );
    params.insert("source_scope".to_string(), row.source_scope.clone().into());
    params.insert("target_scope".to_string(), row.target_scope.clone().into());
    params.insert("evidence_ref".to_string(), row.evidence_ref.clone().into());
    params.insert(
        "receipt_id".to_string(),
        option_string_param(&row.receipt_id),
    );
    params.insert(
        "validation_status".to_string(),
        row.validation_status.clone().into(),
    );
    params.insert("imported_at".to_string(), row.imported_at.clone().into());
    params
}

pub(super) fn trace_event_params(row: &EvalTraceEventRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "trace_event_id".to_string(),
        row.trace_event_id.clone().into(),
    );
    params.insert(
        "campaign_id".to_string(),
        option_string_param(&row.campaign_id),
    );
    params.insert("parent_id".to_string(), option_string_param(&row.parent_id));
    params.insert(
        "runtime_id".to_string(),
        option_string_param(&row.runtime_id),
    );
    params.insert("node_id".to_string(), option_string_param(&row.node_id));
    params.insert("generation".to_string(), option_i64_param(row.generation));
    params.insert("branch_id".to_string(), option_string_param(&row.branch_id));
    params.insert("role".to_string(), option_string_param(&row.role));
    params.insert("pipeline".to_string(), option_string_param(&row.pipeline));
    params.insert("stage".to_string(), option_string_param(&row.stage));
    params.insert("authority".to_string(), option_string_param(&row.authority));
    params.insert(
        "transition".to_string(),
        option_string_param(&row.transition),
    );
    params.insert(
        "event_name".to_string(),
        option_string_param(&row.event_name),
    );
    params.insert("span_name".to_string(), option_string_param(&row.span_name));
    params.insert("target".to_string(), row.target.clone().into());
    params.insert("level".to_string(), row.level.clone().into());
    params.insert("outcome".to_string(), option_string_param(&row.outcome));
    params.insert("duration_ms".to_string(), option_i64_param(row.duration_ms));
    params.insert(
        "record_access".to_string(),
        option_string_param(&row.record_access),
    );
    params.insert(
        "record_kind".to_string(),
        option_string_param(&row.record_kind),
    );
    params.insert(
        "record_path".to_string(),
        option_string_param(&row.record_path),
    );
    params.insert(
        "record_index".to_string(),
        option_i64_param(row.record_index),
    );
    params.insert(
        "record_count".to_string(),
        option_i64_param(row.record_count),
    );
    params.insert("program".to_string(), option_string_param(&row.program));
    params.insert("exit_code".to_string(), option_i64_param(row.exit_code));
    params.insert("error".to_string(), option_string_param(&row.error));
    params.insert(
        "source_log_ref".to_string(),
        option_string_param(&row.source_log_ref),
    );
    params.insert(
        "source_event_index".to_string(),
        option_i64_param(row.source_event_index),
    );
    params.insert(
        "recorded_at".to_string(),
        option_string_param(&row.recorded_at),
    );
    params
}

pub(super) fn provider_attempt_params(
    row: &ProviderAttemptRow,
) -> Result<BTreeMap<String, DataValue>, EvalStoreError> {
    let attempt = &row.timeline;
    let mut params = BTreeMap::new();
    params.insert(
        "provider_attempt_id".to_string(),
        row.provider_attempt_id.clone().into(),
    );
    params.insert(
        "campaign_id".to_string(),
        option_string_param(&row.campaign_id),
    );
    params.insert(
        "request_id".to_string(),
        attempt.request_id.to_string().into(),
    );
    params.insert("attempt".to_string(), i64::from(attempt.attempt).into());
    params.insert(
        "max_attempts".to_string(),
        i64::from(attempt.max_attempts).into(),
    );
    params.insert(
        "started_at_ms".to_string(),
        u64_param(attempt.started_at_ms, "provider_attempt.started_at_ms")?,
    );
    params.insert(
        "request_sent_ms".to_string(),
        option_u64_param(attempt.request_sent_ms, "provider_attempt.request_sent_ms")?,
    );
    params.insert(
        "headers_received_ms".to_string(),
        option_u64_param(
            attempt.headers_received_ms,
            "provider_attempt.headers_received_ms",
        )?,
    );
    params.insert(
        "output_started_ms".to_string(),
        option_u64_param(
            attempt.output_started_ms,
            "provider_attempt.output_started_ms",
        )?,
    );
    params.insert(
        "output_progress_ms".to_string(),
        option_u64_param(
            attempt.output_progress_ms,
            "provider_attempt.output_progress_ms",
        )?,
    );
    params.insert(
        "output_completed_ms".to_string(),
        option_u64_param(
            attempt.output_completed_ms,
            "provider_attempt.output_completed_ms",
        )?,
    );
    params.insert(
        "failed_ms".to_string(),
        option_u64_param(attempt.failed_ms, "provider_attempt.failed_ms")?,
    );
    params.insert(
        "status".to_string(),
        attempt
            .status
            .map(|status| DataValue::from(i64::from(status)))
            .unwrap_or(DataValue::Null),
    );
    params.insert(
        "response_bytes".to_string(),
        option_usize_param(attempt.response_bytes, "provider_attempt.response_bytes")?,
    );
    params.insert(
        "transport_outcome".to_string(),
        attempt.outcome.as_str().into(),
    );
    params.insert(
        "failure_phase".to_string(),
        attempt
            .failure_phase
            .map(|phase| DataValue::from(phase.as_str()))
            .unwrap_or(DataValue::Null),
    );
    params.insert(
        "send_failure".to_string(),
        attempt
            .send_failure
            .as_ref()
            .map(HttpSendFailure::as_str)
            .map(DataValue::from)
            .unwrap_or(DataValue::Null),
    );
    params.insert(
        "body_failure".to_string(),
        attempt
            .body_failure
            .as_ref()
            .map(HttpBodyFailure::as_str)
            .map(DataValue::from)
            .unwrap_or(DataValue::Null),
    );
    params.insert(
        "response_outcome".to_string(),
        attempt.response_outcome.as_str().into(),
    );
    params.insert(
        "retry_decision".to_string(),
        attempt.retry_decision.as_str().into(),
    );
    params.insert(
        "retry_after_ms".to_string(),
        option_u64_param(attempt.retry_after_ms, "provider_attempt.retry_after_ms")?,
    );
    params.insert(
        "backoff_ms".to_string(),
        option_u64_param(attempt.backoff_ms, "provider_attempt.backoff_ms")?,
    );
    params.insert("error".to_string(), option_string_param(&attempt.error));
    params.insert(
        "source_log_ref".to_string(),
        row.source_log_ref.clone().into(),
    );
    params.insert(
        "source_event_index".to_string(),
        row.source_event_index.into(),
    );
    params.insert(
        "recorded_at".to_string(),
        option_string_param(&row.recorded_at),
    );
    Ok(params)
}

fn option_string_param(value: &Option<String>) -> DataValue {
    value
        .clone()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null)
}

fn option_i64_param(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

fn u64_param(value: u64, field: &'static str) -> Result<DataValue, EvalStoreError> {
    i64::try_from(value)
        .map(DataValue::from)
        .map_err(|_| EvalStoreError::Validation {
            field,
            detail: "provider attempt value exceeds Cozo Int range".to_string(),
        })
}

fn option_u64_param(value: Option<u64>, field: &'static str) -> Result<DataValue, EvalStoreError> {
    value
        .map(|value| u64_param(value, field))
        .transpose()
        .map(|value| value.unwrap_or(DataValue::Null))
}

fn option_usize_param(
    value: Option<usize>,
    field: &'static str,
) -> Result<DataValue, EvalStoreError> {
    value
        .map(|value| {
            i64::try_from(value)
                .map(DataValue::from)
                .map_err(|_| EvalStoreError::Validation {
                    field,
                    detail: "provider attempt value exceeds Cozo Int range".to_string(),
                })
        })
        .transpose()
        .map(|value| value.unwrap_or(DataValue::Null))
}
