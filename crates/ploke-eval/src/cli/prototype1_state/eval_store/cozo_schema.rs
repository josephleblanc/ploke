use std::collections::{BTreeMap, BTreeSet};

use cozo::DataValue;

use super::{
    agent_turn::ensure_agent_turn_schema,
    artifact::ensure_artifact_schema,
    build::ensure_build_schema,
    child_plan::{
        CHILD_PLAN_SCHEMA_VERSION, ChildPlanChildSchema, ChildPlanRejectedSchema, ChildPlanSchema,
        ensure_child_plan_schema,
    },
    continuation::ensure_continuation_schema,
    cozo_store::EvalDb,
    error::EvalStoreError,
    evaluation::ensure_evaluation_schema,
    harness::{
        HarnessDiagnosticSchema, HarnessRequestSchema, HarnessWorkspaceChangeSchema,
        HarnessWorkspaceSchema, ensure_harness_schema,
    },
    operation::ensure_operation_schema,
    runner_io::{
        RUNNER_REQUEST_SCHEMA_VERSION, RUNNER_RESULT_SCHEMA_VERSION, RunnerRequestArgSchema,
        RunnerRequestSchema, RunnerRequestTargetSchema, RunnerResultSchema,
        ensure_runner_io_schema,
    },
    scheduler_node::{
        SCHEDULER_NODE_SCHEMA_VERSION, SchedulerNodeSchema, SchedulerNodeStatusSchema,
        SchedulerNodeTargetSchema, ensure_scheduler_node_schema,
    },
    schema::{EvalRelationSchema, define_eval_schema},
    selection::ensure_selection_schema,
    setup::{RunProfilePolicySchema, ensure_setup_schema},
};

define_eval_schema!(TransitionEventSchema {
    "eval_transition_event",
    event_id: "String" =>
    campaign_id: "String",
    parent_id: "String",
    runtime_id: "String",
    node_id: "String",
    generation: "Int",
    transition: "String",
    phase: "String",
    outcome: "String",
    store_scope: "String",
    producer_role: "String",
    visibility_scope: "String",
    source_class: "String",
    evidence_class: "String",
    validation_status: "String",
    source_stream_id: "String",
    source_event_index: "Int",
    source_line: "Int",
    source_ref: "String",
    content_sha256: "String",
    semantic_hash: "String",
    recorded_at: "Int",
    ingested_at: "String",
});

define_eval_schema!(RecordRefSchema {
    "eval_record_ref",
    record_ref_id: "String" =>
    campaign_id: "String",
    family: "String",
    schema_version: "String",
    store_scope: "String",
    producer_role: "String",
    producer_id: "String",
    source_class: "String",
    evidence_class: "String",
    visibility_scope: "String",
    validation_status: "String",
    source_stream_id: "String",
    source_event_index: "Int",
    source_line: "Int",
    source_ref: "String",
    content_sha256: "String",
    payload_json: "String",
    recorded_at: "Int",
    ingested_at: "String",
});

define_eval_schema!(LogRefSchema {
    "eval_log_ref",
    log_ref_id: "String" =>
    campaign_id: "String?",
    runtime_id: "String?",
    store_scope: "String",
    log_kind: "String",
    source_ref: "String",
    byte_start: "Int?",
    byte_len: "Int?",
    content_sha256: "String?",
    sensitivity: "String?",
    recorded_at: "String?",
});

define_eval_schema!(AttemptSchema {
    "eval_attempt",
    attempt_id: "String" =>
    campaign_id: "String",
    runtime_id: "String",
    role: "String",
    parent_id: "String?",
    node_id: "String?",
    invocation_id: "String?",
    channel_id: "String?",
    artifact_id: "String?",
    binary_ref: "String?",
    started_at: "String?",
    status: "String?",
});

define_eval_schema!(InvocationSchema {
    "eval_invocation",
    invocation_id: "String" =>
    campaign_id: "String",
    node_id: "String",
    runtime_id: "String",
    role: "String",
    store_scope: "String",
    producer_role: "String",
    visibility_scope: "String",
    source_class: "String",
    evidence_class: "String",
    validation_status: "String",
    invocation_path: "String",
    source_ref: "String",
    content_sha256: "String",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(ChannelMessageSchema {
    "eval_channel_message",
    channel_message_id: "String" =>
    campaign_id: "String",
    node_id: "String",
    runtime_id: "String",
    direction: "String",
    message_kind: "String",
    message_id: "String",
    store_scope: "String",
    producer_role: "String",
    visibility_scope: "String",
    source_class: "String",
    evidence_class: "String",
    validation_status: "String",
    endpoint_path: "String",
    cursor_offset: "Int",
    bytes_written: "Int",
    body_hash: "String",
    content_sha256: "String",
    source_ref: "String",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(ChannelReceiptSchema {
    "eval_channel_receipt",
    receipt_id: "String" =>
    channel_id: "String",
    message_id: "String",
    campaign_id: "String",
    node_id: "String",
    runtime_id: "String",
    observed_by: "String?",
    direction: "String",
    validation_status: "String",
    imported_ref: "String?",
    observed_at: "String",
});

define_eval_schema!(ImportEventSchema {
    "eval_import_event",
    import_id: "String" =>
    campaign_id: "String",
    importer_id: "String",
    source_runtime_id: "String?",
    source_scope: "String",
    target_scope: "String",
    evidence_ref: "String",
    receipt_id: "String?",
    validation_status: "String",
    imported_at: "String",
});

define_eval_schema!(TraceEventSchema {
    "eval_trace_event",
    trace_event_id: "String" =>
    campaign_id: "String?",
    parent_id: "String?",
    runtime_id: "String?",
    node_id: "String?",
    generation: "Int?",
    branch_id: "String?",
    role: "String?",
    pipeline: "String?",
    stage: "String?",
    authority: "String?",
    transition: "String?",
    event_name: "String?",
    span_name: "String?",
    target: "String",
    level: "String",
    outcome: "String?",
    duration_ms: "Int?",
    record_access: "String?",
    record_kind: "String?",
    record_path: "String?",
    record_index: "Int?",
    record_count: "Int?",
    program: "String?",
    exit_code: "Int?",
    error: "String?",
    source_log_ref: "String?",
    source_event_index: "Int?",
    recorded_at: "String?",
});

pub(super) fn ensure_eval_store_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    let existing = eval_relation_names(db)?;
    reject_unsupported_schema_drift(&existing)?;
    reject_child_plan_row_drift(db, &existing)?;
    reject_scheduler_node_row_drift(db, &existing)?;
    reject_runner_row_drift(db, &existing)?;

    TransitionEventSchema::SCHEMA.ensure_installed(db, "schema.eval_transition_event")?;
    RecordRefSchema::SCHEMA.ensure_installed(db, "schema.eval_record_ref")?;
    LogRefSchema::SCHEMA.ensure_installed(db, "schema.eval_log_ref")?;
    AttemptSchema::SCHEMA.ensure_installed(db, "schema.eval_attempt")?;
    InvocationSchema::SCHEMA.ensure_installed(db, "schema.eval_invocation")?;
    ChannelMessageSchema::SCHEMA.ensure_installed(db, "schema.eval_channel_message")?;
    ChannelReceiptSchema::SCHEMA.ensure_installed(db, "schema.eval_channel_receipt")?;
    ImportEventSchema::SCHEMA.ensure_installed(db, "schema.eval_import_event")?;
    TraceEventSchema::SCHEMA.ensure_installed(db, "schema.eval_trace_event")?;

    ensure_setup_schema(db)?;
    ensure_evaluation_schema(db)?;
    ensure_continuation_schema(db)?;
    ensure_selection_schema(db)?;
    ensure_artifact_schema(db)?;
    ensure_build_schema(db)?;
    ensure_operation_schema(db)?;
    ensure_child_plan_schema(db)?;
    ensure_scheduler_node_schema(db)?;
    ensure_runner_io_schema(db)?;
    ensure_harness_schema(db)?;
    ensure_agent_turn_schema(db)?;

    Ok(())
}

pub(super) fn eval_relation_exists<D: EvalDb + ?Sized>(
    db: &D,
    relation: &str,
) -> Result<bool, EvalStoreError> {
    Ok(eval_relation_names(db)?.contains(relation))
}

fn reject_unsupported_schema_drift(existing: &BTreeSet<String>) -> Result<(), EvalStoreError> {
    if !existing.iter().any(|name| name.starts_with("eval_")) {
        return Ok(());
    }
    let required = [
        ChildPlanSchema::RELATION,
        ChildPlanChildSchema::RELATION,
        ChildPlanRejectedSchema::RELATION,
        SchedulerNodeSchema::RELATION,
        SchedulerNodeStatusSchema::RELATION,
        SchedulerNodeTargetSchema::RELATION,
        RunnerRequestSchema::RELATION,
        RunnerRequestArgSchema::RELATION,
        RunnerRequestTargetSchema::RELATION,
        RunnerResultSchema::RELATION,
        RunProfilePolicySchema::RELATION,
        HarnessRequestSchema::RELATION,
        HarnessDiagnosticSchema::RELATION,
        HarnessWorkspaceSchema::RELATION,
        HarnessWorkspaceChangeSchema::RELATION,
    ];
    let missing = required
        .into_iter()
        .filter(|relation| !existing.contains(*relation))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    Err(EvalStoreError::DbSetup {
        phase: "schema.eval_store.no_migration",
        detail: format!(
            "existing eval DB is missing current relation(s) {}; regenerate the owner eval DB instead of adding schema to an old backup",
            missing.join(", ")
        ),
    })
}

fn reject_child_plan_row_drift<D: EvalDb + ?Sized>(
    db: &D,
    existing: &BTreeSet<String>,
) -> Result<(), EvalStoreError> {
    if !existing.contains(ChildPlanSchema::RELATION) {
        return Ok(());
    }
    let mut params = BTreeMap::new();
    params.insert(
        "schema_version".to_string(),
        CHILD_PLAN_SCHEMA_VERSION.to_string().into(),
    );
    let query = r#"
?[plan_id, actual_schema_version] :=
  *eval_child_plan { plan_id: plan_id, schema_version: actual_schema_version },
  actual_schema_version != $schema_version
:limit 1
"#;
    let result = db
        .eval_query_params(query, params)
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_child_plan.version",
            source,
        })?;
    if result.rows.is_empty() {
        return Ok(());
    }
    let details = result
        .rows
        .first()
        .map(|row| format!("row={row:?}"))
        .unwrap_or_else(|| "row=<unavailable>".to_string());
    Err(EvalStoreError::DbSetup {
        phase: "schema.eval_child_plan.version",
        detail: format!(
            "existing eval DB contains eval_child_plan rows with an unsupported schema_version; expected {CHILD_PLAN_SCHEMA_VERSION}; regenerate the owner eval DB instead of reusing this backup ({details})"
        ),
    })
}

fn reject_scheduler_node_row_drift<D: EvalDb + ?Sized>(
    db: &D,
    existing: &BTreeSet<String>,
) -> Result<(), EvalStoreError> {
    if !existing.contains(SchedulerNodeSchema::RELATION) {
        return Ok(());
    }
    let mut params = BTreeMap::new();
    params.insert(
        "schema_version".to_string(),
        SCHEDULER_NODE_SCHEMA_VERSION.to_string().into(),
    );
    let query = r#"
?[campaign_id, node_id, actual_schema_version] :=
  *eval_scheduler_node { campaign_id: campaign_id, node_id: node_id, projection_schema_version: actual_schema_version },
  actual_schema_version != $schema_version
:limit 1
"#;
    let result = db
        .eval_query_params(query, params)
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_scheduler_node.version",
            source,
        })?;
    if result.rows.is_empty() {
        return Ok(());
    }
    let details = result
        .rows
        .first()
        .map(|row| format!("row={row:?}"))
        .unwrap_or_else(|| "row=<unavailable>".to_string());
    Err(EvalStoreError::DbSetup {
        phase: "schema.eval_scheduler_node.version",
        detail: format!(
            "existing eval DB contains eval_scheduler_node rows with an unsupported projection_schema_version; expected {SCHEDULER_NODE_SCHEMA_VERSION}; regenerate the owner eval DB instead of reusing this backup ({details})"
        ),
    })
}

fn reject_runner_row_drift<D: EvalDb + ?Sized>(
    db: &D,
    existing: &BTreeSet<String>,
) -> Result<(), EvalStoreError> {
    if existing.contains(RunnerRequestSchema::RELATION) {
        reject_projection_version(
            db,
            RunnerRequestSchema::RELATION,
            "projection_schema_version",
            RUNNER_REQUEST_SCHEMA_VERSION,
            "schema.eval_runner_request.version",
        )?;
    }
    if existing.contains(RunnerResultSchema::RELATION) {
        reject_projection_version(
            db,
            RunnerResultSchema::RELATION,
            "projection_schema_version",
            RUNNER_RESULT_SCHEMA_VERSION,
            "schema.eval_runner_result.version",
        )?;
    }
    Ok(())
}

fn reject_projection_version<D: EvalDb + ?Sized>(
    db: &D,
    relation: &'static str,
    field: &'static str,
    expected: &'static str,
    phase: &'static str,
) -> Result<(), EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("schema_version".to_string(), expected.to_string().into());
    let query = format!(
        r#"
?[campaign_id, node_id, actual_schema_version] :=
  *{relation} {{ campaign_id: campaign_id, node_id: node_id, {field}: actual_schema_version }},
  actual_schema_version != $schema_version
:limit 1
"#
    );
    let result = db
        .eval_query_params(&query, params)
        .map_err(|source| EvalStoreError::Db { phase, source })?;
    if result.rows.is_empty() {
        return Ok(());
    }
    let details = result
        .rows
        .first()
        .map(|row| format!("row={row:?}"))
        .unwrap_or_else(|| "row=<unavailable>".to_string());
    Err(EvalStoreError::DbSetup {
        phase,
        detail: format!(
            "existing eval DB contains {relation} rows with an unsupported {field}; expected {expected}; regenerate the owner eval DB instead of reusing this backup ({details})"
        ),
    })
}

fn eval_relation_names<D: EvalDb + ?Sized>(db: &D) -> Result<BTreeSet<String>, EvalStoreError> {
    let result = db
        .eval_query_params("::relations", BTreeMap::new())
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.relations",
            source,
        })?;
    Ok(result
        .rows
        .iter()
        .filter_map(|row| match row.first() {
            Some(DataValue::Str(value)) => Some(value.to_string()),
            _ => None,
        })
        .collect())
}
