use std::collections::BTreeMap;

use cozo::DataValue;

use super::evidence::{
    EvalInvocationRow, EvalLogRefRow, EvalRecordRefRow, EvalTraceEventRow, EvalTransitionEventRow,
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

fn option_string_param(value: &Option<String>) -> DataValue {
    value
        .clone()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null)
}

fn option_i64_param(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}
