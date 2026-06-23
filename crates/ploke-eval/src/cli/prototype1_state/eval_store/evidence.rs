use std::{fs, path::PathBuf};

use ploke_records::ids::CampaignId;
use sha2::{Digest, Sha256};

use super::super::{
    event::{RecordedAt, RuntimeId},
    identity::ParentIdentity,
    journal::JournalAppendReceipt,
};
use super::error::EvalStoreError;

pub(super) const EVENT_REL: &str = "eval_transition_event";
pub(super) const RECORD_REL: &str = "eval_record_ref";
pub(super) const TRACE_EVENT_REL: &str = "eval_trace_event";
pub(super) const LOG_REF_REL: &str = "eval_log_ref";
pub(super) const PARENT_STARTED_TRANSITION: &str = "r4c_to_r5";
pub(super) const PARENT_STARTED_PHASE: &str = "parent_started";
pub(super) const PARENT_STARTED_OUTCOME: &str = "recorded";
pub(super) const STORE_SCOPE: &str = "parent";
pub(super) const PRODUCER_ROLE_PARENT: &str = "parent";
pub(super) const VISIBILITY_SCOPE: &str = "parent_visible";
pub(super) const JOURNAL_SOURCE_CLASS: &str = "direct_write";
pub(super) const COMPATIBILITY_IMPORT_CLASS: &str = "compatibility_import";
pub(super) const TYPED_TRANSITION_CLASS: &str = "typed_transition";
pub(super) const DIAGNOSTIC_CLASS: &str = "diagnostic";
pub(super) const COMPATIBILITY_CLASS: &str = "compatibility";
pub(super) const VALID_STATUS: &str = "valid";
pub(super) const JOURNAL_SCHEMA: &str = "prototype1-transition-journal.jsonl";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParentStartedDbReceipt {
    pub(crate) event_id: String,
    pub(crate) parent_ref_id: String,
    pub(crate) resource_ref_id: String,
    pub(crate) semantic_hash: String,
}

#[derive(Debug, Clone)]
pub(super) struct ParentStartedRows {
    pub(super) event: EvalTransitionEventRow,
    pub(super) records: Vec<EvalRecordRefRow>,
    pub(super) receipt: ParentStartedDbReceipt,
}

#[derive(Debug, Clone)]
pub(super) struct EvalTransitionEventRow {
    pub(super) event_id: String,
    pub(super) campaign_id: String,
    pub(super) parent_id: String,
    pub(super) runtime_id: String,
    pub(super) node_id: String,
    pub(super) generation: i64,
    pub(super) transition: String,
    pub(super) phase: String,
    pub(super) outcome: String,
    pub(super) store_scope: String,
    pub(super) producer_role: String,
    pub(super) visibility_scope: String,
    pub(super) source_class: String,
    pub(super) evidence_class: String,
    pub(super) validation_status: String,
    pub(super) source_stream_id: String,
    pub(super) source_event_index: i64,
    pub(super) source_line: i64,
    pub(super) source_ref: String,
    pub(super) content_sha256: String,
    pub(super) semantic_hash: String,
    pub(super) recorded_at: i64,
    pub(super) ingested_at: String,
}

#[derive(Debug, Clone)]
pub(super) struct EvalRecordRefRow {
    pub(super) record_ref_id: String,
    pub(super) campaign_id: String,
    pub(super) family: String,
    pub(super) schema_version: String,
    pub(super) store_scope: String,
    pub(super) producer_role: String,
    pub(super) producer_id: String,
    pub(super) source_class: String,
    pub(super) evidence_class: String,
    pub(super) visibility_scope: String,
    pub(super) validation_status: String,
    pub(super) source_stream_id: String,
    pub(super) source_event_index: i64,
    pub(super) source_line: i64,
    pub(super) source_ref: String,
    pub(super) content_sha256: String,
    pub(super) payload_json: String,
    pub(super) recorded_at: i64,
    pub(super) ingested_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogRefEvidence {
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) runtime_id: Option<RuntimeId>,
    pub(crate) store_scope: String,
    pub(crate) log_kind: String,
    pub(crate) source_ref: String,
    pub(crate) byte_start: Option<i64>,
    pub(crate) byte_len: Option<i64>,
    pub(crate) content_sha256: Option<String>,
    pub(crate) sensitivity: Option<String>,
    pub(crate) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogRefReceipt {
    pub(crate) log_ref_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordRefEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) family: String,
    pub(crate) schema_version: String,
    pub(crate) store_scope: String,
    pub(crate) producer_role: String,
    pub(crate) producer_id: String,
    pub(crate) source_class: String,
    pub(crate) evidence_class: String,
    pub(crate) visibility_scope: String,
    pub(crate) validation_status: String,
    pub(crate) source_stream_id: String,
    pub(crate) source_event_index: i64,
    pub(crate) source_line: i64,
    pub(crate) source_ref: String,
    pub(crate) payload_json: String,
    pub(crate) recorded_at: i64,
}

impl RecordRefEvidence {
    pub(crate) fn compatibility_import(
        campaign_id: CampaignId,
        family: impl Into<String>,
        schema_version: impl Into<String>,
        producer_id: impl Into<String>,
        source_stream_id: impl Into<String>,
        source_event_index: i64,
        source_line: i64,
        source_ref: impl Into<String>,
        payload_json: impl Into<String>,
        recorded_at: i64,
    ) -> Self {
        Self::compatibility_import_with_axes(
            campaign_id,
            family,
            schema_version,
            STORE_SCOPE,
            PRODUCER_ROLE_PARENT,
            producer_id,
            VISIBILITY_SCOPE,
            source_stream_id,
            source_event_index,
            source_line,
            source_ref,
            payload_json,
            recorded_at,
        )
    }

    pub(crate) fn compatibility_import_with_axes(
        campaign_id: CampaignId,
        family: impl Into<String>,
        schema_version: impl Into<String>,
        store_scope: impl Into<String>,
        producer_role: impl Into<String>,
        producer_id: impl Into<String>,
        visibility_scope: impl Into<String>,
        source_stream_id: impl Into<String>,
        source_event_index: i64,
        source_line: i64,
        source_ref: impl Into<String>,
        payload_json: impl Into<String>,
        recorded_at: i64,
    ) -> Self {
        Self {
            campaign_id,
            family: family.into(),
            schema_version: schema_version.into(),
            store_scope: store_scope.into(),
            producer_role: producer_role.into(),
            producer_id: producer_id.into(),
            source_class: COMPATIBILITY_IMPORT_CLASS.to_string(),
            evidence_class: COMPATIBILITY_CLASS.to_string(),
            visibility_scope: visibility_scope.into(),
            validation_status: VALID_STATUS.to_string(),
            source_stream_id: source_stream_id.into(),
            source_event_index,
            source_line,
            source_ref: source_ref.into(),
            payload_json: payload_json.into(),
            recorded_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordRefReceipt {
    pub(crate) record_ref_id: String,
    pub(crate) content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObservationJsonlImport {
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraceImportReceipt {
    pub(crate) log_ref_id: String,
    pub(crate) trace_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraceEventEvidence {
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) parent_id: Option<String>,
    pub(crate) runtime_id: Option<String>,
    pub(crate) node_id: Option<String>,
    pub(crate) generation: Option<i64>,
    pub(crate) branch_id: Option<String>,
    pub(crate) role: Option<String>,
    pub(crate) pipeline: Option<String>,
    pub(crate) stage: Option<String>,
    pub(crate) authority: Option<String>,
    pub(crate) transition: Option<String>,
    pub(crate) event_name: Option<String>,
    pub(crate) span_name: Option<String>,
    pub(crate) target: String,
    pub(crate) level: String,
    pub(crate) outcome: Option<String>,
    pub(crate) duration_ms: Option<i64>,
    pub(crate) record_access: Option<String>,
    pub(crate) record_kind: Option<String>,
    pub(crate) record_path: Option<String>,
    pub(crate) record_index: Option<i64>,
    pub(crate) record_count: Option<i64>,
    pub(crate) program: Option<String>,
    pub(crate) exit_code: Option<i64>,
    pub(crate) error: Option<String>,
    pub(crate) source_log_ref: Option<String>,
    pub(crate) source_event_index: Option<i64>,
    pub(crate) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraceEventReceipt {
    pub(crate) trace_event_id: String,
}

#[derive(Debug, Clone)]
pub(super) struct ParsedObservationJsonl {
    pub(super) log: EvalLogRefRow,
    pub(super) traces: Vec<EvalTraceEventRow>,
}

#[derive(Debug, Clone)]
pub(super) struct EvalLogRefRow {
    pub(super) log_ref_id: String,
    pub(super) campaign_id: Option<String>,
    pub(super) runtime_id: Option<String>,
    pub(super) store_scope: String,
    pub(super) log_kind: String,
    pub(super) source_ref: String,
    pub(super) byte_start: Option<i64>,
    pub(super) byte_len: Option<i64>,
    pub(super) content_sha256: Option<String>,
    pub(super) sensitivity: Option<String>,
    pub(super) recorded_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct EvalTraceEventRow {
    pub(super) trace_event_id: String,
    pub(super) campaign_id: Option<String>,
    pub(super) parent_id: Option<String>,
    pub(super) runtime_id: Option<String>,
    pub(super) node_id: Option<String>,
    pub(super) generation: Option<i64>,
    pub(super) branch_id: Option<String>,
    pub(super) role: Option<String>,
    pub(super) pipeline: Option<String>,
    pub(super) stage: Option<String>,
    pub(super) authority: Option<String>,
    pub(super) transition: Option<String>,
    pub(super) event_name: Option<String>,
    pub(super) span_name: Option<String>,
    pub(super) target: String,
    pub(super) level: String,
    pub(super) outcome: Option<String>,
    pub(super) duration_ms: Option<i64>,
    pub(super) record_access: Option<String>,
    pub(super) record_kind: Option<String>,
    pub(super) record_path: Option<String>,
    pub(super) record_index: Option<i64>,
    pub(super) record_count: Option<i64>,
    pub(super) program: Option<String>,
    pub(super) exit_code: Option<i64>,
    pub(super) error: Option<String>,
    pub(super) source_log_ref: Option<String>,
    pub(super) source_event_index: Option<i64>,
    pub(super) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParentStartedEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) parent_identity: ParentIdentity,
    pub(crate) repo_root: PathBuf,
    pub(crate) handoff_runtime_id: Option<RuntimeId>,
    pub(crate) pid: u32,
    pub(crate) parent_recorded_at: RecordedAt,
    pub(crate) resource_recorded_at: RecordedAt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParentStartedReceipt {
    pub(crate) parent: JournalAppendReceipt,
    pub(crate) resource: JournalAppendReceipt,
}

pub(super) fn parent_started_db_receipt(
    evidence: &ParentStartedEvidence,
    receipt: &ParentStartedReceipt,
) -> Result<ParentStartedDbReceipt, EvalStoreError> {
    Ok(parent_started_rows(evidence, receipt)?.receipt)
}

pub(super) fn parent_started_rows(
    evidence: &ParentStartedEvidence,
    receipt: &ParentStartedReceipt,
) -> Result<ParentStartedRows, EvalStoreError> {
    validate_parent_started_receipt(receipt)?;
    let stream = receipt.parent.path.display().to_string();
    let parent_index = usize_to_i64(
        receipt.parent.source_event_index,
        "parent.source_event_index",
    )?;
    let parent_line = usize_to_i64(receipt.parent.source_line, "parent.source_line")?;
    let resource_index = usize_to_i64(
        receipt.resource.source_event_index,
        "resource.source_event_index",
    )?;
    let resource_line = usize_to_i64(receipt.resource.source_line, "resource.source_line")?;
    let runtime_id = evidence
        .handoff_runtime_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let ingested_at = chrono::Utc::now().to_rfc3339();
    let source_stream_id = source_stream_id(&evidence.campaign_id, &stream);
    let content_sha256 = hash_parts(&[
        "p1.eval.parent_started.content.v1",
        &receipt.parent.content_sha256,
        &receipt.resource.content_sha256,
    ]);
    let semantic_hash = hash_parts(&[
        "p1.eval.parent_started.semantic.v1",
        evidence.campaign_id.as_str(),
        evidence.parent_identity.parent_id(),
        evidence.parent_identity.node_id(),
        &evidence.parent_identity.generation().to_string(),
        evidence.parent_identity.branch_id(),
        &evidence.repo_root.display().to_string(),
        &runtime_id,
        &evidence.pid.to_string(),
        &evidence.parent_recorded_at.0.to_string(),
        &evidence.resource_recorded_at.0.to_string(),
        &receipt.parent.payload_json,
        &receipt.resource.payload_json,
    ]);
    let event_id = hash_parts(&[
        "p1.eval.transition_event.v1",
        evidence.campaign_id.as_str(),
        evidence.parent_identity.parent_id(),
        PARENT_STARTED_TRANSITION,
        &source_stream_id,
        &parent_index.to_string(),
        &receipt.parent.content_sha256,
    ]);
    let parent_ref_id = record_ref_id(
        evidence.campaign_id.as_str(),
        "parent_started",
        &source_stream_id,
        parent_index,
        &receipt.parent.content_sha256,
    );
    let resource_ref_id = record_ref_id(
        evidence.campaign_id.as_str(),
        "resource_parent_start",
        &source_stream_id,
        resource_index,
        &receipt.resource.content_sha256,
    );
    let event = EvalTransitionEventRow {
        event_id: event_id.clone(),
        campaign_id: evidence.campaign_id.to_string(),
        parent_id: evidence.parent_identity.parent_id().to_string(),
        runtime_id,
        node_id: evidence.parent_identity.node_id().to_string(),
        generation: i64::from(evidence.parent_identity.generation()),
        transition: PARENT_STARTED_TRANSITION.to_string(),
        phase: PARENT_STARTED_PHASE.to_string(),
        outcome: PARENT_STARTED_OUTCOME.to_string(),
        store_scope: STORE_SCOPE.to_string(),
        producer_role: PRODUCER_ROLE_PARENT.to_string(),
        visibility_scope: VISIBILITY_SCOPE.to_string(),
        source_class: JOURNAL_SOURCE_CLASS.to_string(),
        evidence_class: TYPED_TRANSITION_CLASS.to_string(),
        validation_status: VALID_STATUS.to_string(),
        source_stream_id: source_stream_id.clone(),
        source_event_index: parent_index,
        source_line: parent_line,
        source_ref: event_source_ref(&stream, parent_line, resource_line),
        content_sha256: content_sha256.clone(),
        semantic_hash: semantic_hash.clone(),
        recorded_at: evidence.parent_recorded_at.0,
        ingested_at: ingested_at.clone(),
    };
    let parent_record = record_ref_row(
        parent_ref_id.clone(),
        evidence,
        "parent_started",
        TYPED_TRANSITION_CLASS,
        &source_stream_id,
        &stream,
        parent_index,
        parent_line,
        &receipt.parent,
        evidence.parent_recorded_at.0,
        &ingested_at,
    );
    let resource_record = record_ref_row(
        resource_ref_id.clone(),
        evidence,
        "resource_parent_start",
        DIAGNOSTIC_CLASS,
        &source_stream_id,
        &stream,
        resource_index,
        resource_line,
        &receipt.resource,
        evidence.resource_recorded_at.0,
        &ingested_at,
    );
    let receipt = ParentStartedDbReceipt {
        event_id,
        parent_ref_id,
        resource_ref_id,
        semantic_hash,
    };
    Ok(ParentStartedRows {
        event,
        records: vec![parent_record, resource_record],
        receipt,
    })
}

fn record_ref_row(
    record_ref_id: String,
    evidence: &ParentStartedEvidence,
    family: &str,
    evidence_class: &str,
    source_stream_id: &str,
    source_path: &str,
    index: i64,
    line: i64,
    receipt: &JournalAppendReceipt,
    recorded_at: i64,
    ingested_at: &str,
) -> EvalRecordRefRow {
    EvalRecordRefRow {
        record_ref_id,
        campaign_id: evidence.campaign_id.to_string(),
        family: family.to_string(),
        schema_version: JOURNAL_SCHEMA.to_string(),
        store_scope: STORE_SCOPE.to_string(),
        producer_role: PRODUCER_ROLE_PARENT.to_string(),
        producer_id: evidence.parent_identity.parent_id().to_string(),
        source_class: JOURNAL_SOURCE_CLASS.to_string(),
        evidence_class: evidence_class.to_string(),
        visibility_scope: VISIBILITY_SCOPE.to_string(),
        validation_status: VALID_STATUS.to_string(),
        source_stream_id: source_stream_id.to_string(),
        source_event_index: index,
        source_line: line,
        source_ref: source_ref(source_path, line),
        content_sha256: receipt.content_sha256.clone(),
        payload_json: receipt.payload_json.clone(),
        recorded_at,
        ingested_at: ingested_at.to_string(),
    }
}

pub(super) fn log_ref_row(evidence: LogRefEvidence) -> Result<EvalLogRefRow, EvalStoreError> {
    require_non_empty("log_ref.store_scope", &evidence.store_scope)?;
    require_non_empty("log_ref.log_kind", &evidence.log_kind)?;
    require_non_empty("log_ref.source_ref", &evidence.source_ref)?;
    let campaign_id = evidence.campaign_id.map(|id| id.to_string());
    let runtime_id = evidence.runtime_id.map(|id| id.to_string());
    let byte_start = evidence
        .byte_start
        .map(|value| validate_non_negative_i64(value, "log_ref.byte_start"))
        .transpose()?;
    let byte_len = evidence
        .byte_len
        .map(|value| validate_non_negative_i64(value, "log_ref.byte_len"))
        .transpose()?;
    let log_ref_id = log_ref_id(
        campaign_id.as_deref(),
        runtime_id.as_deref(),
        &evidence.store_scope,
        &evidence.log_kind,
        &evidence.source_ref,
        byte_start,
        byte_len,
        evidence.content_sha256.as_deref(),
    );
    Ok(EvalLogRefRow {
        log_ref_id,
        campaign_id,
        runtime_id,
        store_scope: evidence.store_scope,
        log_kind: evidence.log_kind,
        source_ref: evidence.source_ref,
        byte_start,
        byte_len,
        content_sha256: evidence.content_sha256,
        sensitivity: evidence.sensitivity,
        recorded_at: evidence.recorded_at,
    })
}

pub(super) fn record_ref_row_from_evidence(
    evidence: RecordRefEvidence,
) -> Result<EvalRecordRefRow, EvalStoreError> {
    require_non_empty("record_ref.family", &evidence.family)?;
    require_non_empty("record_ref.schema_version", &evidence.schema_version)?;
    require_non_empty("record_ref.store_scope", &evidence.store_scope)?;
    require_non_empty("record_ref.producer_role", &evidence.producer_role)?;
    require_non_empty("record_ref.producer_id", &evidence.producer_id)?;
    require_non_empty("record_ref.source_class", &evidence.source_class)?;
    require_non_empty("record_ref.evidence_class", &evidence.evidence_class)?;
    require_non_empty("record_ref.visibility_scope", &evidence.visibility_scope)?;
    require_non_empty("record_ref.validation_status", &evidence.validation_status)?;
    require_non_empty("record_ref.source_stream_id", &evidence.source_stream_id)?;
    require_non_empty("record_ref.source_ref", &evidence.source_ref)?;
    require_non_empty("record_ref.payload_json", &evidence.payload_json)?;
    let source_event_index =
        validate_non_negative_i64(evidence.source_event_index, "record_ref.source_event_index")?;
    if evidence.source_line <= 0 {
        return Err(EvalStoreError::Validation {
            field: "record_ref.source_line",
            detail: "record-ref source lines are one-based and must be non-zero".to_string(),
        });
    }
    let recorded_at = validate_non_negative_i64(evidence.recorded_at, "record_ref.recorded_at")?;
    let content_sha256 = sha256_bytes(evidence.payload_json.as_bytes());
    let record_ref_id = record_ref_id(
        evidence.campaign_id.as_str(),
        &evidence.family,
        &evidence.source_stream_id,
        source_event_index,
        &content_sha256,
    );
    let ingested_at = chrono::Utc::now().to_rfc3339();
    Ok(EvalRecordRefRow {
        record_ref_id,
        campaign_id: evidence.campaign_id.to_string(),
        family: evidence.family,
        schema_version: evidence.schema_version,
        store_scope: evidence.store_scope,
        producer_role: evidence.producer_role,
        producer_id: evidence.producer_id,
        source_class: evidence.source_class,
        evidence_class: evidence.evidence_class,
        visibility_scope: evidence.visibility_scope,
        validation_status: evidence.validation_status,
        source_stream_id: evidence.source_stream_id,
        source_event_index,
        source_line: evidence.source_line,
        source_ref: evidence.source_ref,
        content_sha256,
        payload_json: evidence.payload_json,
        recorded_at,
        ingested_at,
    })
}

pub(super) fn trace_event_row(
    evidence: TraceEventEvidence,
) -> Result<EvalTraceEventRow, EvalStoreError> {
    require_non_empty("trace.target", &evidence.target)?;
    require_non_empty("trace.level", &evidence.level)?;
    let source_event_index =
        evidence
            .source_event_index
            .ok_or_else(|| EvalStoreError::Validation {
                field: "trace.source_event_index",
                detail: "direct trace events require a scoped source event index".to_string(),
            })?;
    let recorded_at = evidence
        .recorded_at
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let campaign_id = evidence.campaign_id.map(|id| id.to_string());
    let source_index = source_event_index.to_string();
    let generation = evidence
        .generation
        .map(|value| value.to_string())
        .unwrap_or_default();
    let duration_ms = evidence
        .duration_ms
        .map(|value| value.to_string())
        .unwrap_or_default();
    let record_index = evidence
        .record_index
        .map(|value| value.to_string())
        .unwrap_or_default();
    let record_count = evidence
        .record_count
        .map(|value| value.to_string())
        .unwrap_or_default();
    let exit_code = evidence
        .exit_code
        .map(|value| value.to_string())
        .unwrap_or_default();
    let trace_event_id = hash_parts(&[
        "p1.eval.trace_event.direct.v1",
        campaign_id.as_deref().unwrap_or(""),
        evidence.parent_id.as_deref().unwrap_or(""),
        evidence.runtime_id.as_deref().unwrap_or(""),
        evidence.node_id.as_deref().unwrap_or(""),
        &generation,
        evidence.branch_id.as_deref().unwrap_or(""),
        evidence.role.as_deref().unwrap_or(""),
        evidence.pipeline.as_deref().unwrap_or(""),
        evidence.stage.as_deref().unwrap_or(""),
        evidence.authority.as_deref().unwrap_or(""),
        evidence.transition.as_deref().unwrap_or(""),
        evidence.event_name.as_deref().unwrap_or(""),
        evidence.span_name.as_deref().unwrap_or(""),
        &evidence.target,
        &evidence.level,
        evidence.outcome.as_deref().unwrap_or(""),
        &duration_ms,
        evidence.record_access.as_deref().unwrap_or(""),
        evidence.record_kind.as_deref().unwrap_or(""),
        evidence.record_path.as_deref().unwrap_or(""),
        &record_index,
        &record_count,
        evidence.program.as_deref().unwrap_or(""),
        &exit_code,
        evidence.error.as_deref().unwrap_or(""),
        evidence.source_log_ref.as_deref().unwrap_or(""),
        &source_index,
        &recorded_at,
    ]);
    Ok(EvalTraceEventRow {
        trace_event_id,
        campaign_id,
        parent_id: evidence.parent_id,
        runtime_id: evidence.runtime_id,
        node_id: evidence.node_id,
        generation: evidence.generation,
        branch_id: evidence.branch_id,
        role: evidence.role,
        pipeline: evidence.pipeline,
        stage: evidence.stage,
        authority: evidence.authority,
        transition: evidence.transition,
        event_name: evidence.event_name,
        span_name: evidence.span_name,
        target: evidence.target,
        level: evidence.level,
        outcome: evidence.outcome,
        duration_ms: evidence.duration_ms,
        record_access: evidence.record_access,
        record_kind: evidence.record_kind,
        record_path: evidence.record_path,
        record_index: evidence.record_index,
        record_count: evidence.record_count,
        program: evidence.program,
        exit_code: evidence.exit_code,
        error: evidence.error,
        source_log_ref: evidence.source_log_ref,
        source_event_index: Some(source_event_index),
        recorded_at: Some(recorded_at),
    })
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

fn validate_parent_started_receipt(receipt: &ParentStartedReceipt) -> Result<(), EvalStoreError> {
    let stream = receipt.parent.path.display().to_string();
    require_non_empty("source_stream_id", &stream)?;
    if receipt.parent.path != receipt.resource.path {
        return Err(EvalStoreError::Validation {
            field: "source_stream_id",
            detail: "parent and resource receipts must reference the same journal path".to_string(),
        });
    }
    require_non_empty("parent.content_sha256", &receipt.parent.content_sha256)?;
    require_non_empty("resource.content_sha256", &receipt.resource.content_sha256)?;
    require_non_empty("parent.payload_json", &receipt.parent.payload_json)?;
    require_non_empty("resource.payload_json", &receipt.resource.payload_json)?;
    if receipt.parent.source_line == 0 || receipt.resource.source_line == 0 {
        return Err(EvalStoreError::Validation {
            field: "source_line",
            detail: "journal source lines are one-based and must be non-zero".to_string(),
        });
    }
    Ok(())
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), EvalStoreError> {
    if value.is_empty() {
        return Err(EvalStoreError::Validation {
            field,
            detail: "required eval-store field is empty".to_string(),
        });
    }
    Ok(())
}

fn validate_non_negative_i64(value: i64, field: &'static str) -> Result<i64, EvalStoreError> {
    if value < 0 {
        return Err(EvalStoreError::Validation {
            field,
            detail: format!("value {value} must be non-negative"),
        });
    }
    Ok(value)
}

fn usize_to_i64(value: usize, field: &'static str) -> Result<i64, EvalStoreError> {
    i64::try_from(value).map_err(|_| EvalStoreError::Validation {
        field,
        detail: format!("value {value} does not fit in Cozo Int"),
    })
}

fn source_ref(stream: &str, line: i64) -> String {
    format!("{stream}:L{line}")
}

fn event_source_ref(stream: &str, parent_line: i64, resource_line: i64) -> String {
    format!("{stream}:L{parent_line}-L{resource_line}")
}

fn source_stream_id(campaign_id: &CampaignId, stream: &str) -> String {
    format!("prototype1-transition-journal:{campaign_id}:{stream}")
}

fn log_ref_id(
    campaign_id: Option<&str>,
    runtime_id: Option<&str>,
    store_scope: &str,
    log_kind: &str,
    source_ref: &str,
    byte_start: Option<i64>,
    byte_len: Option<i64>,
    content_sha256: Option<&str>,
) -> String {
    hash_parts(&[
        "p1.eval.log_ref.v1",
        campaign_id.unwrap_or(""),
        runtime_id.unwrap_or(""),
        store_scope,
        log_kind,
        source_ref,
        &byte_start
            .map(|value| value.to_string())
            .unwrap_or_default(),
        &byte_len.map(|value| value.to_string()).unwrap_or_default(),
        content_sha256.unwrap_or(""),
    ])
}

fn record_ref_id(
    campaign_id: &str,
    family: &str,
    source_stream_id: &str,
    index: i64,
    content_sha256: &str,
) -> String {
    hash_parts(&[
        "p1.eval.record_ref.v1",
        campaign_id,
        family,
        source_stream_id,
        &index.to_string(),
        content_sha256,
    ])
}

fn hash_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hex_lower(&hasher.finalize())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
