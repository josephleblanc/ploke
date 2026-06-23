use std::collections::BTreeMap;

use cozo::DataValue;

use super::{
    artifact::ensure_artifact_schema,
    continuation::ensure_continuation_schema,
    cozo_store::EvalDb,
    error::EvalStoreError,
    evaluation::ensure_evaluation_schema,
    evidence::{
        ATTEMPT_REL, CHANNEL_MESSAGE_REL, CHANNEL_RECEIPT_REL, EVENT_REL, IMPORT_EVENT_REL,
        INVOCATION_REL, LOG_REF_REL, RECORD_REL, TRACE_EVENT_REL,
    },
    selection::ensure_selection_schema,
};

pub(super) fn ensure_eval_store_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    if !eval_relation_exists(db, EVENT_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_transition_event {
    event_id: String =>
    campaign_id: String,
    parent_id: String,
    runtime_id: String,
    node_id: String,
    generation: Int,
    transition: String,
    phase: String,
    outcome: String,
    store_scope: String,
    producer_role: String,
    visibility_scope: String,
    source_class: String,
    evidence_class: String,
    validation_status: String,
    source_stream_id: String,
    source_event_index: Int,
    source_line: Int,
    source_ref: String,
    content_sha256: String,
    semantic_hash: String,
    recorded_at: Int,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_transition_event",
            source,
        })?;
    }

    if !eval_relation_exists(db, RECORD_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_record_ref {
    record_ref_id: String =>
    campaign_id: String,
    family: String,
    schema_version: String,
    store_scope: String,
    producer_role: String,
    producer_id: String,
    source_class: String,
    evidence_class: String,
    visibility_scope: String,
    validation_status: String,
    source_stream_id: String,
    source_event_index: Int,
    source_line: Int,
    source_ref: String,
    content_sha256: String,
    payload_json: String,
    recorded_at: Int,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_record_ref",
            source,
        })?;
    }

    if !eval_relation_exists(db, LOG_REF_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_log_ref {
    log_ref_id: String =>
    campaign_id: String?,
    runtime_id: String?,
    store_scope: String,
    log_kind: String,
    source_ref: String,
    byte_start: Int?,
    byte_len: Int?,
    content_sha256: String?,
    sensitivity: String?,
    recorded_at: String?
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_log_ref",
            source,
        })?;
    }

    if !eval_relation_exists(db, ATTEMPT_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_attempt {
    attempt_id: String =>
    campaign_id: String,
    runtime_id: String,
    role: String,
    parent_id: String?,
    node_id: String?,
    invocation_id: String?,
    channel_id: String?,
    artifact_id: String?,
    binary_ref: String?,
    started_at: String?,
    status: String?
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_attempt",
            source,
        })?;
    }

    if !eval_relation_exists(db, INVOCATION_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_invocation {
    invocation_id: String =>
    campaign_id: String,
    node_id: String,
    runtime_id: String,
    role: String,
    store_scope: String,
    producer_role: String,
    visibility_scope: String,
    source_class: String,
    evidence_class: String,
    validation_status: String,
    invocation_path: String,
    source_ref: String,
    content_sha256: String,
    recorded_at: String,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_invocation",
            source,
        })?;
    }

    if !eval_relation_exists(db, CHANNEL_MESSAGE_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_channel_message {
    channel_message_id: String =>
    campaign_id: String,
    node_id: String,
    runtime_id: String,
    direction: String,
    message_kind: String,
    message_id: String,
    store_scope: String,
    producer_role: String,
    visibility_scope: String,
    source_class: String,
    evidence_class: String,
    validation_status: String,
    endpoint_path: String,
    cursor_offset: Int,
    bytes_written: Int,
    body_hash: String,
    content_sha256: String,
    source_ref: String,
    recorded_at: String,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_channel_message",
            source,
        })?;
    }

    if !eval_relation_exists(db, CHANNEL_RECEIPT_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_channel_receipt {
    receipt_id: String =>
    channel_id: String,
    message_id: String,
    campaign_id: String,
    node_id: String,
    runtime_id: String,
    observed_by: String?,
    direction: String,
    validation_status: String,
    imported_ref: String?,
    observed_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_channel_receipt",
            source,
        })?;
    }

    if !eval_relation_exists(db, IMPORT_EVENT_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_import_event {
    import_id: String =>
    campaign_id: String,
    importer_id: String,
    source_runtime_id: String?,
    source_scope: String,
    target_scope: String,
    evidence_ref: String,
    receipt_id: String?,
    validation_status: String,
    imported_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_import_event",
            source,
        })?;
    }

    if !eval_relation_exists(db, TRACE_EVENT_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_trace_event {
    trace_event_id: String =>
    campaign_id: String?,
    parent_id: String?,
    runtime_id: String?,
    node_id: String?,
    generation: Int?,
    branch_id: String?,
    role: String?,
    pipeline: String?,
    stage: String?,
    authority: String?,
    transition: String?,
    event_name: String?,
    span_name: String?,
    target: String,
    level: String,
    outcome: String?,
    duration_ms: Int?,
    record_access: String?,
    record_kind: String?,
    record_path: String?,
    record_index: Int?,
    record_count: Int?,
    program: String?,
    exit_code: Int?,
    error: String?,
    source_log_ref: String?,
    source_event_index: Int?,
    recorded_at: String?
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_trace_event",
            source,
        })?;
    }

    ensure_evaluation_schema(db)?;
    ensure_continuation_schema(db)?;
    ensure_selection_schema(db)?;
    ensure_artifact_schema(db)?;

    Ok(())
}

pub(super) fn eval_relation_exists<D: EvalDb + ?Sized>(
    db: &D,
    relation: &str,
) -> Result<bool, EvalStoreError> {
    let result = db
        .eval_query_params("::relations", BTreeMap::new())
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.relations",
            source,
        })?;
    Ok(result.rows.iter().any(|row| {
        row.first().and_then(|value| match value {
            DataValue::Str(value) => Some(value.as_str()),
            _ => None,
        }) == Some(relation)
    }))
}
