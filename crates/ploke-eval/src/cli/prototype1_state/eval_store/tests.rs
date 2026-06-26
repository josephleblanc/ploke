use std::{collections::BTreeMap, fs, path::PathBuf};

use cozo::DataValue;
use ploke_db::{Database, QueryResult};

use ploke_records::{
    agent_turn::{
        AgentTurnArtifactRecord, AgentTurnSummaryRecord, AgentTurnTraceRecord,
        MessageSnapshotRecord, ModelRouteRecord, ObservedTurnEventRecord, PatchArtifactRecord,
        RequestMessageRecord, RequestRoleRecord, ToolCompletedRecord, ToolRequestRecord,
        TurnFinishedRecord,
    },
    identity::ParentIdentityRecord,
    ids::CampaignId,
    llm_response::{FULL_RESPONSE_TRACE_FILE, RawFullResponseRecord},
    tool_contracts::ToolArgumentsJson,
};
use sha2::{Digest, Sha256};

use super::*;
use super::{
    AGENT_TURN_EVENT_REL, AGENT_TURN_REL, APPLY_EVENT_REL, ARTIFACT_REF_REL, ARTIFACT_REL,
    ARTIFACT_SURFACE_REL, BASELINE_REL, BINARY_REF_REL, BUILD_EVENT_REL, CAMPAIGN_REL,
    CLOSURE_REF_REL, CONTINUATION_DECISION_REL, EVALUATION_INSTANCE_REL, EVALUATION_REL,
    MESSAGE_EVENT_REL, MODEL_EXCHANGE_REL, OPERATION_REL, PATCH_REL, PROFILE_COMMITMENT_REL,
    SELECTION_CANDIDATE_REL, SELECTION_DECISION_REL, SELECTION_FINDING_REL, SELECTION_SCORE_REL,
    TOOL_EVENT_REL,
    api::EvalStorageMode,
    cozo_schema::eval_relation_exists,
    error::EvalStoreError,
    evidence::{
        ATTEMPT_REL, CHANNEL_MESSAGE_REL, CHANNEL_RECEIPT_REL, EVENT_REL, IMPORT_EVENT_REL,
        INVOCATION_REL, LOG_REF_REL, PARENT_STARTED_OUTCOME, PARENT_STARTED_PHASE,
        PARENT_STARTED_TRANSITION, RECORD_REL, STORE_SCOPE, TRACE_EVENT_REL,
        parent_started_db_receipt,
    },
    schema::EvalRelationSchema,
};
use crate::cli::prototype1_state::{
    event::RecordedAt,
    identity::{PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentity},
    journal::{self, JournalAppendReceipt, JournalEntry, ParentStartedEntry, PrototypeJournal},
    profile,
};
use crate::intervention::{BaselineInstance, CompleteBaseline, RecordStore};
use crate::{
    BenchmarkFamily, CampaignManifest, ClosureClass, EvalCampaignPolicy, OperationalRunMetrics,
    PatchApplyState, ProtocolCampaignPolicy, record::SubmissionArtifactState,
};

#[test]
fn eval_store_production_code_uses_schema_generated_cozo_scripts() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/prototype1_state/eval_store");
    let mut offenders = Vec::new();

    for entry in fs::read_dir(&dir).expect("eval_store dir reads") {
        let path = entry.expect("eval_store entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf-8 file name");
        if matches!(file, "schema.rs" | "tests.rs") {
            continue;
        }

        let mut text = fs::read_to_string(&path).expect("eval_store source reads");
        if let Some(test_start) = text.find("#[cfg(test)]") {
            text.truncate(test_start);
        }
        for (index, line) in text.lines().enumerate() {
            if line.contains(":create eval_") || line.contains(":put eval_") {
                offenders.push(format!("{}:{}: {}", file, index + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "production eval-store Cozo scripts must be schema-generated:\n{}",
        offenders.join("\n")
    );
}

fn eval_schema_params<S: super::schema::EvalRelationSchema>(
    schema: &S,
) -> BTreeMap<String, DataValue> {
    schema
        .all_fields()
        .into_iter()
        .map(|field| (field.name().to_string(), DataValue::Null))
        .collect()
}

fn non_agent_schema_scripts() -> Vec<(&'static str, String, String)> {
    vec![
        {
            let schema = &super::setup::CampaignSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::setup::ProfileCommitmentSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::setup::ClosureRefSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::setup::BaselineSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::TransitionEventSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::RecordRefSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::LogRefSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::AttemptSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::InvocationSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::ChannelMessageSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::ChannelReceiptSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::ImportEventSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::cozo_schema::TraceEventSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::artifact::ArtifactSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::artifact::ArtifactSurfaceSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::artifact::ArtifactRefSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::build::BinaryRefSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::build::BuildEventSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::evaluation::EvaluationSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::evaluation::EvaluationInstanceSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::continuation::ContinuationDecisionSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::operation::OperationSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::operation::PatchSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::operation::ApplyEventSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::selection::SelectionDecisionSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::selection::SelectionCandidateSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::selection::SelectionFindingSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
        {
            let schema = &super::selection::SelectionScoreSchema::SCHEMA;
            (
                schema.relation(),
                schema.script_create(),
                schema.script_put(&eval_schema_params(schema)),
            )
        },
    ]
}

#[test]
fn eval_store_non_agent_schema_scripts_are_stable() {
    let actual = non_agent_schema_scripts();
    let expected = vec![
        (
            "eval_campaign",
            r#":create eval_campaign { campaign_id: String => schema_version: String, manifest_ref: String, prototype_root: String, manifest_sha256: String, profile_ref_id: String?, storage_backend: String?, ingested_at: String }"#,
            r#"?[campaign_id, schema_version, manifest_ref, prototype_root, manifest_sha256, profile_ref_id, storage_backend, ingested_at] <- [[$campaign_id, $schema_version, $manifest_ref, $prototype_root, $manifest_sha256, $profile_ref_id, $storage_backend, $ingested_at]] :put eval_campaign { campaign_id => schema_version, manifest_ref, prototype_root, manifest_sha256, profile_ref_id, storage_backend, ingested_at }"#,
        ),
        (
            "eval_profile_commitment",
            r#":create eval_profile_commitment { profile_ref_id: String => campaign_id: String, schema_version: String, profile_name: String, source_ref: String, profile_path: String, content_sha256: String, source_path: String?, admitted_at: String, storage_ref: String, ingested_at: String }"#,
            r#"?[profile_ref_id, campaign_id, schema_version, profile_name, source_ref, profile_path, content_sha256, source_path, admitted_at, storage_ref, ingested_at] <- [[$profile_ref_id, $campaign_id, $schema_version, $profile_name, $source_ref, $profile_path, $content_sha256, $source_path, $admitted_at, $storage_ref, $ingested_at]] :put eval_profile_commitment { profile_ref_id => campaign_id, schema_version, profile_name, source_ref, profile_path, content_sha256, source_path, admitted_at, storage_ref, ingested_at }"#,
        ),
        (
            "eval_closure_ref",
            r#":create eval_closure_ref { closure_ref_id: String => campaign_id: String, run_id: String?, store_scope: String, source_ref: String, content_sha256: String, summary_json: String, recorded_at: String, ingested_at: String }"#,
            r#"?[closure_ref_id, campaign_id, run_id, store_scope, source_ref, content_sha256, summary_json, recorded_at, ingested_at] <- [[$closure_ref_id, $campaign_id, $run_id, $store_scope, $source_ref, $content_sha256, $summary_json, $recorded_at, $ingested_at]] :put eval_closure_ref { closure_ref_id => campaign_id, run_id, store_scope, source_ref, content_sha256, summary_json, recorded_at, ingested_at }"#,
        ),
        (
            "eval_baseline",
            r#":create eval_baseline { baseline_id: String => campaign_id: String, parent_id: String, parent_node_id: String, parent_branch_id: String, source_kind: String, closure_ref_id: String?, evaluation_id: String?, record_ref: String?, eval_set_id: String, status: String, instance_count: Int, summary_json: String, recorded_at: String, ingested_at: String }"#,
            r#"?[baseline_id, campaign_id, parent_id, parent_node_id, parent_branch_id, source_kind, closure_ref_id, evaluation_id, record_ref, eval_set_id, status, instance_count, summary_json, recorded_at, ingested_at] <- [[$baseline_id, $campaign_id, $parent_id, $parent_node_id, $parent_branch_id, $source_kind, $closure_ref_id, $evaluation_id, $record_ref, $eval_set_id, $status, $instance_count, $summary_json, $recorded_at, $ingested_at]] :put eval_baseline { baseline_id => campaign_id, parent_id, parent_node_id, parent_branch_id, source_kind, closure_ref_id, evaluation_id, record_ref, eval_set_id, status, instance_count, summary_json, recorded_at, ingested_at }"#,
        ),
        (
            "eval_transition_event",
            r#":create eval_transition_event { event_id: String => campaign_id: String, parent_id: String, runtime_id: String, node_id: String, generation: Int, transition: String, phase: String, outcome: String, store_scope: String, producer_role: String, visibility_scope: String, source_class: String, evidence_class: String, validation_status: String, source_stream_id: String, source_event_index: Int, source_line: Int, source_ref: String, content_sha256: String, semantic_hash: String, recorded_at: Int, ingested_at: String }"#,
            r#"?[event_id, campaign_id, parent_id, runtime_id, node_id, generation, transition, phase, outcome, store_scope, producer_role, visibility_scope, source_class, evidence_class, validation_status, source_stream_id, source_event_index, source_line, source_ref, content_sha256, semantic_hash, recorded_at, ingested_at] <- [[$event_id, $campaign_id, $parent_id, $runtime_id, $node_id, $generation, $transition, $phase, $outcome, $store_scope, $producer_role, $visibility_scope, $source_class, $evidence_class, $validation_status, $source_stream_id, $source_event_index, $source_line, $source_ref, $content_sha256, $semantic_hash, $recorded_at, $ingested_at]] :put eval_transition_event { event_id => campaign_id, parent_id, runtime_id, node_id, generation, transition, phase, outcome, store_scope, producer_role, visibility_scope, source_class, evidence_class, validation_status, source_stream_id, source_event_index, source_line, source_ref, content_sha256, semantic_hash, recorded_at, ingested_at }"#,
        ),
        (
            "eval_record_ref",
            r#":create eval_record_ref { record_ref_id: String => campaign_id: String, family: String, schema_version: String, store_scope: String, producer_role: String, producer_id: String, source_class: String, evidence_class: String, visibility_scope: String, validation_status: String, source_stream_id: String, source_event_index: Int, source_line: Int, source_ref: String, content_sha256: String, payload_json: String, recorded_at: Int, ingested_at: String }"#,
            r#"?[record_ref_id, campaign_id, family, schema_version, store_scope, producer_role, producer_id, source_class, evidence_class, visibility_scope, validation_status, source_stream_id, source_event_index, source_line, source_ref, content_sha256, payload_json, recorded_at, ingested_at] <- [[$record_ref_id, $campaign_id, $family, $schema_version, $store_scope, $producer_role, $producer_id, $source_class, $evidence_class, $visibility_scope, $validation_status, $source_stream_id, $source_event_index, $source_line, $source_ref, $content_sha256, $payload_json, $recorded_at, $ingested_at]] :put eval_record_ref { record_ref_id => campaign_id, family, schema_version, store_scope, producer_role, producer_id, source_class, evidence_class, visibility_scope, validation_status, source_stream_id, source_event_index, source_line, source_ref, content_sha256, payload_json, recorded_at, ingested_at }"#,
        ),
        (
            "eval_log_ref",
            r#":create eval_log_ref { log_ref_id: String => campaign_id: String?, runtime_id: String?, store_scope: String, log_kind: String, source_ref: String, byte_start: Int?, byte_len: Int?, content_sha256: String?, sensitivity: String?, recorded_at: String? }"#,
            r#"?[log_ref_id, campaign_id, runtime_id, store_scope, log_kind, source_ref, byte_start, byte_len, content_sha256, sensitivity, recorded_at] <- [[$log_ref_id, $campaign_id, $runtime_id, $store_scope, $log_kind, $source_ref, $byte_start, $byte_len, $content_sha256, $sensitivity, $recorded_at]] :put eval_log_ref { log_ref_id => campaign_id, runtime_id, store_scope, log_kind, source_ref, byte_start, byte_len, content_sha256, sensitivity, recorded_at }"#,
        ),
        (
            "eval_attempt",
            r#":create eval_attempt { attempt_id: String => campaign_id: String, runtime_id: String, role: String, parent_id: String?, node_id: String?, invocation_id: String?, channel_id: String?, artifact_id: String?, binary_ref: String?, started_at: String?, status: String? }"#,
            r#"?[attempt_id, campaign_id, runtime_id, role, parent_id, node_id, invocation_id, channel_id, artifact_id, binary_ref, started_at, status] <- [[$attempt_id, $campaign_id, $runtime_id, $role, $parent_id, $node_id, $invocation_id, $channel_id, $artifact_id, $binary_ref, $started_at, $status]] :put eval_attempt { attempt_id => campaign_id, runtime_id, role, parent_id, node_id, invocation_id, channel_id, artifact_id, binary_ref, started_at, status }"#,
        ),
        (
            "eval_invocation",
            r#":create eval_invocation { invocation_id: String => campaign_id: String, node_id: String, runtime_id: String, role: String, store_scope: String, producer_role: String, visibility_scope: String, source_class: String, evidence_class: String, validation_status: String, invocation_path: String, source_ref: String, content_sha256: String, recorded_at: String, ingested_at: String }"#,
            r#"?[invocation_id, campaign_id, node_id, runtime_id, role, store_scope, producer_role, visibility_scope, source_class, evidence_class, validation_status, invocation_path, source_ref, content_sha256, recorded_at, ingested_at] <- [[$invocation_id, $campaign_id, $node_id, $runtime_id, $role, $store_scope, $producer_role, $visibility_scope, $source_class, $evidence_class, $validation_status, $invocation_path, $source_ref, $content_sha256, $recorded_at, $ingested_at]] :put eval_invocation { invocation_id => campaign_id, node_id, runtime_id, role, store_scope, producer_role, visibility_scope, source_class, evidence_class, validation_status, invocation_path, source_ref, content_sha256, recorded_at, ingested_at }"#,
        ),
        (
            "eval_channel_message",
            r#":create eval_channel_message { channel_message_id: String => campaign_id: String, node_id: String, runtime_id: String, direction: String, message_kind: String, message_id: String, store_scope: String, producer_role: String, visibility_scope: String, source_class: String, evidence_class: String, validation_status: String, endpoint_path: String, cursor_offset: Int, bytes_written: Int, body_hash: String, content_sha256: String, source_ref: String, recorded_at: String, ingested_at: String }"#,
            r#"?[channel_message_id, campaign_id, node_id, runtime_id, direction, message_kind, message_id, store_scope, producer_role, visibility_scope, source_class, evidence_class, validation_status, endpoint_path, cursor_offset, bytes_written, body_hash, content_sha256, source_ref, recorded_at, ingested_at] <- [[$channel_message_id, $campaign_id, $node_id, $runtime_id, $direction, $message_kind, $message_id, $store_scope, $producer_role, $visibility_scope, $source_class, $evidence_class, $validation_status, $endpoint_path, $cursor_offset, $bytes_written, $body_hash, $content_sha256, $source_ref, $recorded_at, $ingested_at]] :put eval_channel_message { channel_message_id => campaign_id, node_id, runtime_id, direction, message_kind, message_id, store_scope, producer_role, visibility_scope, source_class, evidence_class, validation_status, endpoint_path, cursor_offset, bytes_written, body_hash, content_sha256, source_ref, recorded_at, ingested_at }"#,
        ),
        (
            "eval_channel_receipt",
            r#":create eval_channel_receipt { receipt_id: String => channel_id: String, message_id: String, campaign_id: String, node_id: String, runtime_id: String, observed_by: String?, direction: String, validation_status: String, imported_ref: String?, observed_at: String }"#,
            r#"?[receipt_id, channel_id, message_id, campaign_id, node_id, runtime_id, observed_by, direction, validation_status, imported_ref, observed_at] <- [[$receipt_id, $channel_id, $message_id, $campaign_id, $node_id, $runtime_id, $observed_by, $direction, $validation_status, $imported_ref, $observed_at]] :put eval_channel_receipt { receipt_id => channel_id, message_id, campaign_id, node_id, runtime_id, observed_by, direction, validation_status, imported_ref, observed_at }"#,
        ),
        (
            "eval_import_event",
            r#":create eval_import_event { import_id: String => campaign_id: String, importer_id: String, source_runtime_id: String?, source_scope: String, target_scope: String, evidence_ref: String, receipt_id: String?, validation_status: String, imported_at: String }"#,
            r#"?[import_id, campaign_id, importer_id, source_runtime_id, source_scope, target_scope, evidence_ref, receipt_id, validation_status, imported_at] <- [[$import_id, $campaign_id, $importer_id, $source_runtime_id, $source_scope, $target_scope, $evidence_ref, $receipt_id, $validation_status, $imported_at]] :put eval_import_event { import_id => campaign_id, importer_id, source_runtime_id, source_scope, target_scope, evidence_ref, receipt_id, validation_status, imported_at }"#,
        ),
        (
            "eval_trace_event",
            r#":create eval_trace_event { trace_event_id: String => campaign_id: String?, parent_id: String?, runtime_id: String?, node_id: String?, generation: Int?, branch_id: String?, role: String?, pipeline: String?, stage: String?, authority: String?, transition: String?, event_name: String?, span_name: String?, target: String, level: String, outcome: String?, duration_ms: Int?, record_access: String?, record_kind: String?, record_path: String?, record_index: Int?, record_count: Int?, program: String?, exit_code: Int?, error: String?, source_log_ref: String?, source_event_index: Int?, recorded_at: String? }"#,
            r#"?[trace_event_id, campaign_id, parent_id, runtime_id, node_id, generation, branch_id, role, pipeline, stage, authority, transition, event_name, span_name, target, level, outcome, duration_ms, record_access, record_kind, record_path, record_index, record_count, program, exit_code, error, source_log_ref, source_event_index, recorded_at] <- [[$trace_event_id, $campaign_id, $parent_id, $runtime_id, $node_id, $generation, $branch_id, $role, $pipeline, $stage, $authority, $transition, $event_name, $span_name, $target, $level, $outcome, $duration_ms, $record_access, $record_kind, $record_path, $record_index, $record_count, $program, $exit_code, $error, $source_log_ref, $source_event_index, $recorded_at]] :put eval_trace_event { trace_event_id => campaign_id, parent_id, runtime_id, node_id, generation, branch_id, role, pipeline, stage, authority, transition, event_name, span_name, target, level, outcome, duration_ms, record_access, record_kind, record_path, record_index, record_count, program, exit_code, error, source_log_ref, source_event_index, recorded_at }"#,
        ),
        (
            "eval_artifact",
            r#":create eval_artifact { artifact_id: String => campaign_id: String, tree_hash: String?, git_branch: String?, git_commit: String?, source: String, store_scope: String, created_by: String?, parent_artifact_id: String? }"#,
            r#"?[artifact_id, campaign_id, tree_hash, git_branch, git_commit, source, store_scope, created_by, parent_artifact_id] <- [[$artifact_id, $campaign_id, $tree_hash, $git_branch, $git_commit, $source, $store_scope, $created_by, $parent_artifact_id]] :put eval_artifact { artifact_id => campaign_id, tree_hash, git_branch, git_commit, source, store_scope, created_by, parent_artifact_id }"#,
        ),
        (
            "eval_artifact_surface",
            r#":create eval_artifact_surface { surface_id: String => campaign_id: String, artifact_id: String, immutable_root: String?, mutated_root: String?, ambient_root: String?, surface_hash: String?, source_ref: String?, recorded_at: String? }"#,
            r#"?[surface_id, campaign_id, artifact_id, immutable_root, mutated_root, ambient_root, surface_hash, source_ref, recorded_at] <- [[$surface_id, $campaign_id, $artifact_id, $immutable_root, $mutated_root, $ambient_root, $surface_hash, $source_ref, $recorded_at]] :put eval_artifact_surface { surface_id => campaign_id, artifact_id, immutable_root, mutated_root, ambient_root, surface_hash, source_ref, recorded_at }"#,
        ),
        (
            "eval_artifact_ref",
            r#":create eval_artifact_ref { artifact_ref_id: String => campaign_id: String, artifact_id: String?, kind: String, source_ref: String, content_sha256: String?, recorded_at: String? }"#,
            r#"?[artifact_ref_id, campaign_id, artifact_id, kind, source_ref, content_sha256, recorded_at] <- [[$artifact_ref_id, $campaign_id, $artifact_id, $kind, $source_ref, $content_sha256, $recorded_at]] :put eval_artifact_ref { artifact_ref_id => campaign_id, artifact_id, kind, source_ref, content_sha256, recorded_at }"#,
        ),
        (
            "eval_binary_ref",
            r#":create eval_binary_ref { binary_ref_id: String => campaign_id: String, artifact_id: String?, built_by: String?, source_ref: String, content_sha256: String?, protocol_digest: String?, recorded_at: String? }"#,
            r#"?[binary_ref_id, campaign_id, artifact_id, built_by, source_ref, content_sha256, protocol_digest, recorded_at] <- [[$binary_ref_id, $campaign_id, $artifact_id, $built_by, $source_ref, $content_sha256, $protocol_digest, $recorded_at]] :put eval_binary_ref { binary_ref_id => campaign_id, artifact_id, built_by, source_ref, content_sha256, protocol_digest, recorded_at }"#,
        ),
        (
            "eval_build_event",
            r#":create eval_build_event { build_id: String => campaign_id: String, node_id: String, runtime_id: String?, artifact_id: String?, phase: String, outcome: String, binary_ref: String?, log_ref: String?, recorded_at: String }"#,
            r#"?[build_id, campaign_id, node_id, runtime_id, artifact_id, phase, outcome, binary_ref, log_ref, recorded_at] <- [[$build_id, $campaign_id, $node_id, $runtime_id, $artifact_id, $phase, $outcome, $binary_ref, $log_ref, $recorded_at]] :put eval_build_event { build_id => campaign_id, node_id, runtime_id, artifact_id, phase, outcome, binary_ref, log_ref, recorded_at }"#,
        ),
        (
            "eval_evaluation",
            r#":create eval_evaluation { evaluation_id: String => campaign_id: String, parent_id: String?, branch_id: String, baseline_id: String?, treatment_id: String?, procedure_id: String?, evaluator_id: String?, eval_set_id: String?, policy_ref: String?, disposition: String, record_ref: String?, recorded_at: String? }"#,
            r#"?[evaluation_id, campaign_id, parent_id, branch_id, baseline_id, treatment_id, procedure_id, evaluator_id, eval_set_id, policy_ref, disposition, record_ref, recorded_at] <- [[$evaluation_id, $campaign_id, $parent_id, $branch_id, $baseline_id, $treatment_id, $procedure_id, $evaluator_id, $eval_set_id, $policy_ref, $disposition, $record_ref, $recorded_at]] :put eval_evaluation { evaluation_id => campaign_id, parent_id, branch_id, baseline_id, treatment_id, procedure_id, evaluator_id, eval_set_id, policy_ref, disposition, record_ref, recorded_at }"#,
        ),
        (
            "eval_evaluation_instance",
            r#":create eval_evaluation_instance { evaluation_id: String, instance_id: String => baseline_run_id: String?, treatment_run_id: String?, baseline_ref: String?, treatment_ref: String?, status: String, outcome: String?, oracle_ref: String? }"#,
            r#"?[evaluation_id, instance_id, baseline_run_id, treatment_run_id, baseline_ref, treatment_ref, status, outcome, oracle_ref] <- [[$evaluation_id, $instance_id, $baseline_run_id, $treatment_run_id, $baseline_ref, $treatment_ref, $status, $outcome, $oracle_ref]] :put eval_evaluation_instance { evaluation_id, instance_id => baseline_run_id, treatment_run_id, baseline_ref, treatment_ref, status, outcome, oracle_ref }"#,
        ),
        (
            "eval_continuation_decision",
            r#":create eval_continuation_decision { decision_id: String => campaign_id: String, parent_id: String, disposition: String, selected_branch_id: String?, next_generation: Int, total_nodes: Int, policy_ref: String?, recorded_at: String? }"#,
            r#"?[decision_id, campaign_id, parent_id, disposition, selected_branch_id, next_generation, total_nodes, policy_ref, recorded_at] <- [[$decision_id, $campaign_id, $parent_id, $disposition, $selected_branch_id, $next_generation, $total_nodes, $policy_ref, $recorded_at]] :put eval_continuation_decision { decision_id => campaign_id, parent_id, disposition, selected_branch_id, next_generation, total_nodes, policy_ref, recorded_at }"#,
        ),
        (
            "eval_operation",
            r#":create eval_operation { operation_id: String => campaign_id: String, generator_id: String, target_kind: String, target_ref: String, procedure_id: String?, output_artifact_id: String?, output_patch_id: String?, recorded_at: String? }"#,
            r#"?[operation_id, campaign_id, generator_id, target_kind, target_ref, procedure_id, output_artifact_id, output_patch_id, recorded_at] <- [[$operation_id, $campaign_id, $generator_id, $target_kind, $target_ref, $procedure_id, $output_artifact_id, $output_patch_id, $recorded_at]] :put eval_operation { operation_id => campaign_id, generator_id, target_kind, target_ref, procedure_id, output_artifact_id, output_patch_id, recorded_at }"#,
        ),
        (
            "eval_patch",
            r#":create eval_patch { patch_id: String => campaign_id: String, base_artifact_id: String?, creator_id: String?, tool_call_id: String?, target_relpath: String?, patch_ref: String?, content_sha256: String?, status: String? }"#,
            r#"?[patch_id, campaign_id, base_artifact_id, creator_id, tool_call_id, target_relpath, patch_ref, content_sha256, status] <- [[$patch_id, $campaign_id, $base_artifact_id, $creator_id, $tool_call_id, $target_relpath, $patch_ref, $content_sha256, $status]] :put eval_patch { patch_id => campaign_id, base_artifact_id, creator_id, tool_call_id, target_relpath, patch_ref, content_sha256, status }"#,
        ),
        (
            "eval_apply_event",
            r#":create eval_apply_event { apply_id: String => campaign_id: String, patch_id: String, runtime_id: String?, artifact_id: String?, outcome: String, output_artifact_id: String?, recorded_at: String }"#,
            r#"?[apply_id, campaign_id, patch_id, runtime_id, artifact_id, outcome, output_artifact_id, recorded_at] <- [[$apply_id, $campaign_id, $patch_id, $runtime_id, $artifact_id, $outcome, $output_artifact_id, $recorded_at]] :put eval_apply_event { apply_id => campaign_id, patch_id, runtime_id, artifact_id, outcome, output_artifact_id, recorded_at }"#,
        ),
        (
            "eval_selection_decision",
            r#":create eval_selection_decision { decision_id: String => campaign_id: String, parent_id: String, set_id: String, procedure_id: String, selected_node_id: String?, selected_artifact_id: String?, outcome: String, disposition: String?, decision_ref: String?, decision_hash: String?, recorded_at: String? }"#,
            r#"?[decision_id, campaign_id, parent_id, set_id, procedure_id, selected_node_id, selected_artifact_id, outcome, disposition, decision_ref, decision_hash, recorded_at] <- [[$decision_id, $campaign_id, $parent_id, $set_id, $procedure_id, $selected_node_id, $selected_artifact_id, $outcome, $disposition, $decision_ref, $decision_hash, $recorded_at]] :put eval_selection_decision { decision_id => campaign_id, parent_id, set_id, procedure_id, selected_node_id, selected_artifact_id, outcome, disposition, decision_ref, decision_hash, recorded_at }"#,
        ),
        (
            "eval_selection_candidate",
            r#":create eval_selection_candidate { decision_id: String, member_id: String => node_id: String, branch_id: String, selectable: Bool, selected: Bool, exclusion_ref: String? }"#,
            r#"?[decision_id, member_id, node_id, branch_id, selectable, selected, exclusion_ref] <- [[$decision_id, $member_id, $node_id, $branch_id, $selectable, $selected, $exclusion_ref]] :put eval_selection_candidate { decision_id, member_id => node_id, branch_id, selectable, selected, exclusion_ref }"#,
        ),
        (
            "eval_selection_finding",
            r#":create eval_selection_finding { finding_id: String => decision_id: String, member_id: String?, domain: String, verdict: String, confidence: String, evidence_ref: String?, rationale_ref: String? }"#,
            r#"?[finding_id, decision_id, member_id, domain, verdict, confidence, evidence_ref, rationale_ref] <- [[$finding_id, $decision_id, $member_id, $domain, $verdict, $confidence, $evidence_ref, $rationale_ref]] :put eval_selection_finding { finding_id => decision_id, member_id, domain, verdict, confidence, evidence_ref, rationale_ref }"#,
        ),
        (
            "eval_selection_score",
            r#":create eval_selection_score { decision_id: String, member_id: String => formula_id: String, score_json: String, weight: Float?, rank: Int?, selected: Bool }"#,
            r#"?[decision_id, member_id, formula_id, score_json, weight, rank, selected] <- [[$decision_id, $member_id, $formula_id, $score_json, $weight, $rank, $selected]] :put eval_selection_score { decision_id, member_id => formula_id, score_json, weight, rank, selected }"#,
        ),
    ];

    assert_eq!(actual.len(), expected.len());
    for ((actual_rel, actual_create, actual_put), (expected_rel, expected_create, expected_put)) in
        actual.iter().zip(expected)
    {
        assert_eq!(actual_rel, &expected_rel, "relation order drifted");
        assert_eq!(
            actual_create, expected_create,
            "create script drifted for {expected_rel}"
        );
        assert_eq!(
            actual_put, expected_put,
            "put script drifted for {expected_rel}"
        );
    }
}

#[test]
fn prototype1_eval_store_parent_start_fs_appends_expected_entries() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).expect("repo dir");
    let path = tmp.path().join("transition-journal.jsonl");
    let mut journal = PrototypeJournal::new(&path);
    let evidence = parent_started_evidence(repo);
    let mut store = FsEvalStore::new(&mut journal);

    let receipt = store
        .put_parent_started(evidence.clone())
        .expect("parent start writes");

    assert_eq!(receipt.parent.source_event_index, 0);
    assert_eq!(receipt.parent.source_line, 1);
    assert_eq!(receipt.resource.source_event_index, 1);
    assert_eq!(receipt.resource.source_line, 2);
    let entries = PrototypeJournal::new(&path)
        .load_entries()
        .expect("journal loads");
    assert_eq!(entries.len(), 2);
    match &entries[0] {
        JournalEntry::ParentStarted(entry) => {
            assert_eq!(entry.campaign_id, evidence.campaign_id);
            assert_eq!(entry.parent_identity, evidence.parent_identity);
            assert_eq!(entry.repo_root, evidence.repo_root);
            assert_eq!(entry.pid, evidence.pid);
        }
        other => panic!("unexpected first entry: {other:?}"),
    }
    match &entries[1] {
        JournalEntry::Resource(sample) => {
            assert_eq!(sample.campaign_id, evidence.campaign_id);
            assert_eq!(sample.parent_id, evidence.parent_identity.parent_id());
            assert_eq!(sample.phase, journal::resource::Phase::ParentStart);
            assert_eq!(sample.status, journal::resource::Status::Missing);
            assert_eq!(sample.path, evidence.repo_root.join("target"));
        }
        other => panic!("unexpected second entry: {other:?}"),
    }
}

#[test]
fn append_with_receipt_preserves_record_store_bytes() {
    let tmp = tempfile::tempdir().expect("tmp");
    let old_path = tmp.path().join("old.jsonl");
    let new_path = tmp.path().join("new.jsonl");
    let entry = JournalEntry::ParentStarted(parent_entry(tmp.path().join("repo")));

    let mut old = PrototypeJournal::new(&old_path);
    old.append(entry.clone()).expect("old append");

    let mut new = PrototypeJournal::new(&new_path);
    let receipt = new.append_with_receipt(entry).expect("receipt append");

    let old_bytes = fs::read(&old_path).expect("old bytes");
    let new_bytes = fs::read(&new_path).expect("new bytes");
    assert_eq!(new_bytes, old_bytes);
    assert_eq!(receipt.byte_start, 0);
    assert_eq!(receipt.byte_len, receipt.payload_json.len());
    assert_eq!(new_bytes, format!("{}\n", receipt.payload_json).as_bytes());
    assert!(!receipt.content_sha256.is_empty());
}

#[test]
fn prototype1_eval_store_parent_start_db_schema_installs_idempotently() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);

    store.install_schema().expect("schema install");
    store
        .install_schema()
        .expect("schema install is idempotent");

    assert!(eval_relation_exists(&db, CAMPAIGN_REL).expect("campaign rel exists"));
    assert!(
        eval_relation_exists(&db, PROFILE_COMMITMENT_REL).expect("profile commitment rel exists")
    );
    assert!(eval_relation_exists(&db, CLOSURE_REF_REL).expect("closure ref rel exists"));
    assert!(eval_relation_exists(&db, BASELINE_REL).expect("baseline rel exists"));
    assert!(eval_relation_exists(&db, EVENT_REL).expect("event rel exists"));
    assert!(eval_relation_exists(&db, ATTEMPT_REL).expect("attempt rel exists"));
    assert!(eval_relation_exists(&db, INVOCATION_REL).expect("invocation rel exists"));
    assert!(eval_relation_exists(&db, CHANNEL_MESSAGE_REL).expect("channel message rel exists"));
    assert!(eval_relation_exists(&db, CHANNEL_RECEIPT_REL).expect("channel receipt rel exists"));
    assert!(eval_relation_exists(&db, IMPORT_EVENT_REL).expect("import event rel exists"));
    assert!(eval_relation_exists(&db, EVALUATION_REL).expect("evaluation rel exists"));
    assert!(
        eval_relation_exists(&db, EVALUATION_INSTANCE_REL).expect("evaluation instance rel exists")
    );
    assert!(
        eval_relation_exists(&db, CONTINUATION_DECISION_REL)
            .expect("continuation decision rel exists")
    );
    assert!(
        eval_relation_exists(&db, SELECTION_DECISION_REL).expect("selection decision rel exists")
    );
    assert!(
        eval_relation_exists(&db, SELECTION_CANDIDATE_REL).expect("selection candidate rel exists")
    );
    assert!(
        eval_relation_exists(&db, SELECTION_FINDING_REL).expect("selection finding rel exists")
    );
    assert!(eval_relation_exists(&db, SELECTION_SCORE_REL).expect("selection score rel exists"));
    assert!(eval_relation_exists(&db, ARTIFACT_REL).expect("artifact rel exists"));
    assert!(eval_relation_exists(&db, ARTIFACT_SURFACE_REL).expect("artifact surface rel exists"));
    assert!(eval_relation_exists(&db, ARTIFACT_REF_REL).expect("artifact ref rel exists"));
    assert!(eval_relation_exists(&db, BINARY_REF_REL).expect("binary ref rel exists"));
    assert!(eval_relation_exists(&db, BUILD_EVENT_REL).expect("build event rel exists"));
    assert!(eval_relation_exists(&db, OPERATION_REL).expect("operation rel exists"));
    assert!(eval_relation_exists(&db, PATCH_REL).expect("patch rel exists"));
    assert!(eval_relation_exists(&db, APPLY_EVENT_REL).expect("apply event rel exists"));
    assert!(eval_relation_exists(&db, RECORD_REL).expect("record rel exists"));
    assert!(eval_relation_exists(&db, LOG_REF_REL).expect("log rel exists"));
    assert!(eval_relation_exists(&db, TRACE_EVENT_REL).expect("trace rel exists"));
    assert!(eval_relation_exists(&db, AGENT_TURN_REL).expect("agent turn rel exists"));
    assert!(eval_relation_exists(&db, AGENT_TURN_EVENT_REL).expect("agent turn event rel exists"));
    assert!(eval_relation_exists(&db, MODEL_EXCHANGE_REL).expect("model exchange rel exists"));
    assert!(eval_relation_exists(&db, MESSAGE_EVENT_REL).expect("message event rel exists"));
    assert!(eval_relation_exists(&db, TOOL_EVENT_REL).expect("tool event rel exists"));
}

#[test]
fn prototype1_eval_store_agent_turn_rows_are_queryable() {
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join("prototype1/broad/request-1.turn-live");
    fs::create_dir_all(&dir).expect("turn live dir");
    let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
    let trace_path = dir.join("agent-turn-trace.json");
    let summary_path = dir.join("agent-turn-summary.json");
    let response_path = dir.join("llm-full-responses.jsonl");
    let artifact = sample_agent_turn_artifact();
    let response = sample_raw_full_response();

    let receipt = write_agent_turn_to_owner_db(
        &db_path,
        AgentTurnEvidence {
            campaign_id: Some(CampaignId::from("campaign")),
            trace_path: trace_path.display().to_string(),
            summary_path: summary_path.display().to_string(),
            full_response_path: Some(response_path.display().to_string()),
            trace_record: AgentTurnTraceRecord(artifact.clone()),
            summary_record: AgentTurnSummaryRecord(artifact),
            full_responses: vec![response],
            recorded_at: "2026-06-25T00:00:00Z".to_string(),
        },
    )
    .expect("agent turn rows write");

    assert_eq!(receipt.event_ids.len(), 4);
    assert_eq!(receipt.exchange_ids.len(), 1);
    assert_eq!(receipt.tool_ids.len(), 2);

    let db = load_owner_eval_database(&db_path).expect("reload owner db");
    let turn = query_agent_turn(&db, &receipt.turn_id);
    assert_eq!(turn.rows.len(), 1);
    let row = turn.row_refs().next().expect("agent turn row");
    assert_eq!(
        row.get::<String>("campaign_id").expect("campaign"),
        "campaign"
    );
    assert_eq!(
        row.get::<String>("request_id").expect("request"),
        "request-1"
    );
    assert_eq!(row.get::<i64>("event_count").expect("event count"), 4);
    assert_eq!(row.get::<i64>("response_count").expect("response count"), 1);
    assert_eq!(
        row.get::<String>("terminal_outcome").expect("outcome"),
        "applied"
    );

    assert_eq!(query_agent_turn_events(&db, &receipt.turn_id).rows.len(), 4);
    assert_eq!(query_model_exchanges(&db, &receipt.turn_id).rows.len(), 1);
    assert_eq!(query_message_events(&db, &receipt.turn_id).rows.len(), 3);
    assert_eq!(query_tool_events(&db, &receipt.turn_id).rows.len(), 2);
}

#[test]
fn prototype1_eval_store_agent_turn_bundle_dual_strict_writes_files_and_rows() {
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join("prototype1/broad/request-1.turn-live");
    let trace_path = dir.join("agent-turn-trace.json");
    let summary_path = dir.join("agent-turn-summary.json");
    let response_path = dir.join(FULL_RESPONSE_TRACE_FILE);
    let artifact = sample_agent_turn_artifact();
    let response = sample_raw_full_response();
    let mut store = ConfiguredEvalStore::for_record_backend(
        profile::EvalStorageBackend::DualStrict,
        &trace_path,
    )
    .expect("configured eval store");

    let receipt = store
        .put_agent_turn_bundle(AgentTurnBundleEvidence {
            campaign_id: Some(CampaignId::from("campaign")),
            trace_path: trace_path.clone(),
            summary_path: summary_path.clone(),
            full_response_path: response_path.clone(),
            trace_record: AgentTurnTraceRecord(artifact.clone()),
            summary_record: AgentTurnSummaryRecord(artifact),
            full_responses: vec![response],
            recorded_at: "2026-06-25T00:00:00Z".to_string(),
        })
        .expect("agent turn bundle writes");

    assert!(trace_path.is_file());
    assert!(summary_path.is_file());
    assert!(response_path.is_file());
    let db_receipt = receipt.db_receipt.expect("dual-strict db receipt");
    assert_eq!(db_receipt.event_ids.len(), 4);
    assert_eq!(db_receipt.exchange_ids.len(), 1);

    let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
    let db = load_owner_eval_database(&db_path).expect("reload owner db");
    assert_eq!(query_agent_turn(&db, &db_receipt.turn_id).rows.len(), 1);
    assert_eq!(
        query_model_exchanges(&db, &db_receipt.turn_id).rows.len(),
        1
    );
}

#[test]
fn prototype1_eval_store_owner_db_serializes_parallel_agent_turn_writes() {
    let tmp = tempfile::tempdir().expect("tmp");
    let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
    let count = 8;

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for index in 0..count {
            let db_path = db_path.clone();
            handles.push(scope.spawn(move || {
                let mut artifact = sample_agent_turn_artifact();
                artifact.task_id = format!("request-{index}");
                artifact.user_message_id = format!("user-{index}");
                let trace_path = format!("/tmp/prototype1/request-{index}/agent-turn-trace.json");
                let summary_path =
                    format!("/tmp/prototype1/request-{index}/agent-turn-summary.json");
                let response_path =
                    format!("/tmp/prototype1/request-{index}/{FULL_RESPONSE_TRACE_FILE}");
                write_agent_turn_to_owner_db(
                    &db_path,
                    AgentTurnEvidence {
                        campaign_id: Some(CampaignId::from("campaign")),
                        trace_path,
                        summary_path,
                        full_response_path: Some(response_path),
                        trace_record: AgentTurnTraceRecord(artifact.clone()),
                        summary_record: AgentTurnSummaryRecord(artifact),
                        full_responses: vec![sample_raw_full_response()],
                        recorded_at: "2026-06-25T00:00:00Z".to_string(),
                    },
                )
                .expect("parallel agent turn row writes");
            }));
        }
        for handle in handles {
            handle.join().expect("parallel writer thread");
        }
    });

    let db = load_owner_eval_database(&db_path).expect("reload owner db");
    let result = db
        .raw_query_params(
            r#"
?[turn_id] :=
    *eval_agent_turn { turn_id }
"#,
            BTreeMap::new(),
        )
        .expect("query agent turn rows");
    assert_eq!(result.rows.len(), count);
}

#[test]
fn prototype1_eval_store_setup_relations_round_trip_actual_loop_types() {
    let tmp = tempfile::tempdir().expect("tmp");
    let campaign_id = CampaignId::from("campaign");
    let manifest_path = tmp.path().join("campaign.json");
    let closure_path = tmp.path().join("closure-state.json");
    let manifest = sample_campaign_manifest(campaign_id.clone());
    let closure = sample_closure_state(campaign_id.clone());
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("manifest json"),
    )
    .expect("manifest file");
    fs::write(
        &closure_path,
        serde_json::to_vec_pretty(&closure).expect("closure json"),
    )
    .expect("closure file");
    let admitted = sample_admitted_profile(tmp.path());
    let baseline = sample_complete_baseline(campaign_id.clone());
    let parent = parent_identity();
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);

    store
        .put_r0_context(
            &manifest_path,
            &manifest,
            profile::EvalStorageBackend::DualStrict,
            Some(&admitted),
            &closure_path,
            &closure,
        )
        .expect("r0 context rows write");
    let baseline_id = store
        .put_baseline(
            &parent,
            &baseline,
            Some((&closure_path, &closure)),
            None,
            None,
            "2026-06-23T00:00:00Z".to_string(),
        )
        .expect("baseline row writes");

    let campaign = query_campaign(&db, &campaign_id);
    assert_eq!(campaign.rows.len(), 1);
    let row = campaign.row_refs().next().expect("campaign row");
    assert_eq!(
        row.get::<String>("storage_backend")
            .expect("storage backend"),
        "dual-strict"
    );
    assert!(
        !row.get::<String>("profile_ref_id")
            .expect("profile ref")
            .is_empty()
    );

    let profiles = query_profile_commitments(&db, &campaign_id);
    assert_eq!(profiles.rows.len(), 1);
    let profile_row = profiles.row_refs().next().expect("profile row");
    assert_eq!(
        profile_row
            .get::<String>("profile_name")
            .expect("profile name"),
        admitted.profile.name
    );

    let closures = query_closure_refs(&db, &campaign_id);
    assert_eq!(closures.rows.len(), 1);
    let closure_row = closures.row_refs().next().expect("closure row");
    assert_eq!(
        closure_row
            .get::<String>("recorded_at")
            .expect("recorded at"),
        closure.updated_at
    );

    let baselines = query_baselines(&db, &campaign_id);
    assert_eq!(baselines.rows.len(), 1);
    let baseline_row = baselines.row_refs().next().expect("baseline row");
    assert_eq!(
        baseline_row
            .get::<String>("baseline_id")
            .expect("baseline id"),
        baseline_id
    );
    assert_eq!(
        baseline_row
            .get::<String>("source_kind")
            .expect("source kind"),
        "generation0_closure"
    );
    assert_eq!(
        baseline_row
            .get::<i64>("instance_count")
            .expect("instance count"),
        1
    );
}

#[test]
fn prototype1_eval_store_trace_log_ref_round_trips_row() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);

    let receipt = store
        .put_log_ref(LogRefEvidence {
            campaign_id: Some(CampaignId::from("campaign")),
            runtime_id: None,
            store_scope: STORE_SCOPE.to_string(),
            log_kind: "observation_jsonl".to_string(),
            source_ref: "/tmp/prototype1-observation.jsonl".to_string(),
            byte_start: Some(0),
            byte_len: Some(12),
            content_sha256: Some("abc123".to_string()),
            sensitivity: Some("internal_diagnostic".to_string()),
            recorded_at: Some("2026-06-23T00:00:00Z".to_string()),
        })
        .expect("log ref writes");

    let refs = query_log_refs(&db);
    assert_eq!(refs.rows.len(), 1);
    let row = refs.row_refs().next().expect("log ref row");
    assert_eq!(
        row.get::<String>("log_ref_id").expect("id"),
        receipt.log_ref_id
    );
    assert_eq!(
        row.get::<String>("log_kind").expect("kind"),
        "observation_jsonl"
    );
    assert_eq!(
        row.get::<String>("source_ref").expect("source"),
        "/tmp/prototype1-observation.jsonl"
    );
    assert_eq!(row.get::<String>("content_sha256").expect("hash"), "abc123");
}

#[test]
fn prototype1_eval_store_record_ref_compatibility_import_round_trips_axes() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    let payload = r#"{"schema_version":"prototype1-node.v1","node_id":"node-1"}"#;

    let receipt = store
        .put_record_ref(RecordRefEvidence::compatibility_import(
            CampaignId::from("campaign"),
            "scheduler_node",
            "prototype1-node.v1",
            "parent",
            "prototype1-record:campaign",
            7,
            8,
            "/tmp/prototype1/nodes/node-1/node.json:L8",
            payload,
            1234,
        ))
        .expect("compatibility record ref writes");

    let refs = query_record_refs(&db, &CampaignId::from("campaign"));
    assert_eq!(refs.rows.len(), 1);
    let row = refs.row_refs().next().expect("record ref row");
    assert_eq!(
        row.get::<String>("record_ref_id").expect("id"),
        receipt.record_ref_id
    );
    assert_eq!(
        row.get::<String>("content_sha256").expect("hash"),
        receipt.content_sha256
    );
    assert_eq!(
        row.get::<String>("family").expect("family"),
        "scheduler_node"
    );
    assert_eq!(
        row.get::<String>("schema_version").expect("schema"),
        "prototype1-node.v1"
    );
    assert_eq!(row.get::<String>("store_scope").expect("scope"), "parent");
    assert_eq!(row.get::<String>("producer_role").expect("role"), "parent");
    assert_eq!(
        row.get::<String>("source_class").expect("source"),
        "compatibility_import"
    );
    assert_eq!(
        row.get::<String>("evidence_class").expect("evidence"),
        "compatibility"
    );
    assert_eq!(
        row.get::<String>("visibility_scope").expect("visibility"),
        "parent_visible"
    );
    assert_eq!(
        row.get::<String>("validation_status").expect("status"),
        "valid"
    );
    assert_eq!(row.get::<String>("payload_json").expect("payload"), payload);
    assert!(
        query_all_transition_events(&db).rows.is_empty(),
        "compatibility record refs must not fabricate transition authority"
    );
}

#[test]
fn prototype1_eval_store_record_ref_duplicate_identical_is_idempotent() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    let evidence = RecordRefEvidence::compatibility_import(
        CampaignId::from("campaign"),
        "scheduler_node",
        "prototype1-node.v1",
        "parent",
        "prototype1-record:campaign",
        7,
        8,
        "/tmp/prototype1/nodes/node-1/node.json:L8",
        r#"{"schema_version":"prototype1-node.v1","node_id":"node-1"}"#,
        1234,
    );

    let first = store
        .put_record_ref(evidence.clone())
        .expect("first record ref");
    let second = store.put_record_ref(evidence).expect("same record ref");

    assert_eq!(second, first);
    assert_eq!(
        query_record_refs(&db, &CampaignId::from("campaign"))
            .rows
            .len(),
        1
    );
}

#[test]
fn prototype1_eval_store_record_ref_missing_axis_fails_without_rows() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    store.install_schema().expect("schema");
    let evidence = RecordRefEvidence::compatibility_import(
        CampaignId::from("campaign"),
        "",
        "prototype1-node.v1",
        "parent",
        "prototype1-record:campaign",
        7,
        8,
        "/tmp/prototype1/nodes/node-1/node.json:L8",
        r#"{"schema_version":"prototype1-node.v1","node_id":"node-1"}"#,
        1234,
    );

    let err = store
        .put_record_ref(evidence)
        .expect_err("missing family fails");

    match err {
        EvalStoreError::Validation { field, detail } => {
            assert_eq!(field, "record_ref.family");
            assert!(detail.contains("required eval-store field"));
        }
        other => panic!("unexpected record ref validation error: {other:?}"),
    }
    assert!(
        query_record_refs(&db, &CampaignId::from("campaign"))
            .rows
            .is_empty()
    );
}

#[test]
fn prototype1_eval_store_trace_observation_jsonl_imports_rows_idempotently() {
    let tmp = tempfile::tempdir().expect("tmp");
    let log_path = tmp.path().join("prototype1-observation.jsonl");
    fs::write(
            &log_path,
            concat!(
                r#"{"timestamp":"2026-06-23T00:00:00Z","target":"ploke_exec","level":"INFO","event":"typestate_transition","role":"parent","pipeline":"prototype1.child_plan_authority","phase":"typestate_transition","transition":"R7->R8","outcome":"committed","campaign_id":"campaign","parent_id":"parent","node_id":"parent","generation":0,"branch_id":"main","record_access":"write","record_kind":"child_plan_file","record_path":"prototype1/messages/child-plan.json","record_index":0,"record_count":1,"duration_ms":17}"#,
                "\n",
                r#"{"timestamp":"2026-06-23T00:00:01Z","target":"ploke_exec","level":"INFO","span":{"name":"child-build"},"outcome":"rejected","program":"cargo","exit_code":101,"duration_ms":22}"#,
                "\n"
            ),
        )
        .expect("write observation jsonl");
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    let import = ObservationJsonlImport {
        campaign_id: Some(CampaignId::from("campaign")),
        path: log_path.clone(),
    };

    let first = store
        .import_observation_jsonl(import.clone())
        .expect("import observation jsonl");
    let second = store
        .import_observation_jsonl(import)
        .expect("reimport observation jsonl");

    assert_eq!(second, first);
    assert_eq!(query_log_refs(&db).rows.len(), 1);
    let traces = query_trace_events(&db, &first.log_ref_id);
    assert_eq!(traces.rows.len(), 2);
    let mut by_index = std::collections::BTreeMap::new();
    for row in traces.row_refs() {
        by_index.insert(
            row.get::<i64>("source_event_index").expect("index"),
            (
                row.get::<String>("event_name").ok(),
                row.get::<String>("stage").ok(),
                row.get::<String>("transition").ok(),
                row.get::<String>("span_name").ok(),
                row.get::<String>("program").ok(),
                row.get::<i64>("exit_code").ok(),
            ),
        );
    }
    assert_eq!(
        by_index.get(&0).expect("first trace").0.as_deref(),
        Some("typestate_transition")
    );
    assert_eq!(
        by_index.get(&0).expect("first trace").1.as_deref(),
        Some("typestate_transition")
    );
    assert_eq!(
        by_index.get(&0).expect("first trace").2.as_deref(),
        Some("R7->R8")
    );
    assert_eq!(
        by_index.get(&1).expect("second trace").3.as_deref(),
        Some("child-build")
    );
    assert_eq!(
        by_index.get(&1).expect("second trace").4.as_deref(),
        Some("cargo")
    );
    assert_eq!(by_index.get(&1).expect("second trace").5, Some(101));
}

#[test]
fn prototype1_eval_store_trace_observation_jsonl_invalid_line_fails_without_rows() {
    let tmp = tempfile::tempdir().expect("tmp");
    let log_path = tmp.path().join("bad-observation.jsonl");
    fs::write(
        &log_path,
        concat!(
            r#"{"target":"ploke_exec","level":"INFO","event":"typestate_transition"}"#,
            "\n",
            "not json\n"
        ),
    )
    .expect("write bad observation jsonl");
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    store
        .install_schema()
        .expect("schema for empty-row assertions");

    let err = store
        .import_observation_jsonl(ObservationJsonlImport {
            campaign_id: Some(CampaignId::from("campaign")),
            path: log_path,
        })
        .expect_err("invalid jsonl fails loudly");

    match err {
        EvalStoreError::Validation { field, detail } => {
            assert_eq!(field, "observation_jsonl.line");
            assert!(detail.contains("invalid JSONL line 2"), "{detail}");
        }
        other => panic!("unexpected import error: {other:?}"),
    }
    assert!(query_log_refs(&db).rows.is_empty());
    assert!(query_all_trace_events(&db).rows.is_empty());
}

#[test]
fn prototype1_eval_store_parent_start_db_round_trips_rows() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (evidence, receipt) = parent_started_fixture(tmp.path());
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);

    let db_receipt = store
        .put_parent_started_from_receipt(&evidence, &receipt)
        .expect("db parent start write");

    let event = query_transition_event(&db, &db_receipt.event_id);
    assert_eq!(event.rows.len(), 1);
    let row = event.row_refs().next().expect("event row");
    assert_eq!(
        row.get::<String>("campaign_id").expect("campaign"),
        "campaign"
    );
    assert_eq!(row.get::<String>("parent_id").expect("parent"), "parent");
    assert_eq!(row.get::<String>("node_id").expect("node"), "parent");
    assert_eq!(row.get::<i64>("generation").expect("generation"), 0);
    assert_eq!(
        row.get::<String>("transition").expect("transition"),
        PARENT_STARTED_TRANSITION
    );
    assert_eq!(
        row.get::<String>("phase").expect("phase"),
        PARENT_STARTED_PHASE
    );
    assert_eq!(
        row.get::<String>("outcome").expect("outcome"),
        PARENT_STARTED_OUTCOME
    );
    assert_eq!(
        row.get::<i64>("source_event_index").expect("event index"),
        receipt.parent.source_event_index as i64
    );
    assert_eq!(
        row.get::<i64>("source_line").expect("source line"),
        receipt.parent.source_line as i64
    );
    assert_eq!(
        row.get::<String>("semantic_hash").expect("semantic hash"),
        db_receipt.semantic_hash
    );

    let records = query_record_refs(&db, &evidence.campaign_id);
    assert_eq!(records.rows.len(), 2);
    let mut families = std::collections::BTreeMap::new();
    for row in records.row_refs() {
        families.insert(
            row.get::<String>("family").expect("family"),
            (
                row.get::<i64>("source_event_index").expect("index"),
                row.get::<String>("content_sha256").expect("hash"),
                row.get::<String>("payload_json").expect("payload"),
            ),
        );
    }
    assert_eq!(
        families.get("parent_started").expect("parent ref").0,
        receipt.parent.source_event_index as i64
    );
    assert_eq!(
        families.get("parent_started").expect("parent ref").1,
        receipt.parent.content_sha256
    );
    assert_eq!(
        families
            .get("resource_parent_start")
            .expect("resource ref")
            .0,
        receipt.resource.source_event_index as i64
    );
    assert_eq!(
        families
            .get("resource_parent_start")
            .expect("resource ref")
            .1,
        receipt.resource.content_sha256
    );
}

#[test]
fn prototype1_eval_store_parent_start_db_duplicate_identical_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (evidence, receipt) = parent_started_fixture(tmp.path());
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);

    let first = store
        .put_parent_started_from_receipt(&evidence, &receipt)
        .expect("first db write");
    let second = store
        .put_parent_started_from_receipt(&evidence, &receipt)
        .expect("second identical write");

    assert_eq!(second, first);
    assert_eq!(query_transition_event(&db, &first.event_id).rows.len(), 1);
    assert_eq!(query_record_refs(&db, &evidence.campaign_id).rows.len(), 2);
}

#[test]
fn prototype1_eval_store_parent_start_db_duplicate_semantic_mismatch_fails() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (evidence, receipt) = parent_started_fixture(tmp.path());
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    let first = store
        .put_parent_started_from_receipt(&evidence, &receipt)
        .expect("first db write");
    let mut changed_evidence = evidence.clone();
    changed_evidence.pid = evidence.pid + 1;

    let err = store
        .put_parent_started_from_receipt(&changed_evidence, &receipt)
        .expect_err("semantic mismatch fails");

    match err {
        EvalStoreError::SemanticConflict {
            event_id,
            existing_semantic_hash,
            attempted_semantic_hash,
        } => {
            assert_eq!(event_id, first.event_id);
            assert_eq!(existing_semantic_hash, first.semantic_hash);
            assert_ne!(attempted_semantic_hash, first.semantic_hash);
        }
        other => panic!("unexpected mismatch error: {other:?}"),
    }
    assert_eq!(query_transition_event(&db, &first.event_id).rows.len(), 1);
    assert_eq!(query_record_refs(&db, &evidence.campaign_id).rows.len(), 2);
}

#[test]
fn prototype1_eval_store_parent_start_db_missing_required_hash_fails_before_rows() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (evidence, mut receipt) = parent_started_fixture(tmp.path());
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    receipt.parent.content_sha256.clear();

    let err = store
        .put_parent_started_from_receipt(&evidence, &receipt)
        .expect_err("missing hash fails");

    match err {
        EvalStoreError::Validation { field, detail } => {
            assert_eq!(field, "parent.content_sha256");
            assert!(detail.contains("required eval-store field"));
        }
        other => panic!("unexpected validation error: {other:?}"),
    }
    assert!(query_all_transition_events(&db).rows.is_empty());
    assert!(
        query_record_refs(&db, &evidence.campaign_id)
            .rows
            .is_empty()
    );
}

#[test]
fn prototype1_eval_store_parent_start_dual_strict_persists_owner_db() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).expect("repo dir");
    let journal_path = tmp.path().join("prototype1/transition-journal.jsonl");
    let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
    let mut journal = PrototypeJournal::new(&journal_path);
    let evidence = parent_started_evidence(repo);
    let mut store =
        FileDbEvalStore::new(&mut journal, db_path.clone(), EvalStorageMode::DualStrict);

    let receipt = store
        .put_parent_started(evidence.clone())
        .expect("dual-strict parent start writes");

    assert!(db_path.is_file());
    let db = load_owner_eval_database(&db_path).expect("owner eval db loads");
    let expected = parent_started_db_receipt(&evidence, &receipt).expect("expected receipt");
    assert_eq!(
        query_transition_event(&db, &expected.event_id).rows.len(),
        1
    );
    assert_eq!(query_record_refs(&db, &evidence.campaign_id).rows.len(), 2);
}

#[test]
fn prototype1_eval_store_parent_start_dual_strict_failure_keeps_repairable_journal() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).expect("repo dir");
    let journal_path = tmp.path().join("prototype1/transition-journal.jsonl");
    let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
    fs::create_dir_all(&db_path).expect("poison db path as directory");
    let mut journal = PrototypeJournal::new(&journal_path);
    let evidence = parent_started_evidence(repo);
    let mut store =
        FileDbEvalStore::new(&mut journal, db_path.clone(), EvalStorageMode::DualStrict);

    let err = store
        .put_parent_started(evidence.clone())
        .expect_err("db failure after fs append fails loudly");
    let receipt = receipt_from_journal(&journal_path);
    let expected = parent_started_db_receipt(&evidence, &receipt).expect("expected receipt");
    match err {
        EvalStoreError::PostFsDb {
            backend,
            journal_path: err_journal_path,
            parent_started_source_event_index,
            resource_source_event_index,
            parent_started_content_sha256,
            resource_content_sha256,
            expected_semantic_hash,
            suggested_recovery,
            ..
        } => {
            assert_eq!(backend, "dual-strict");
            assert_eq!(err_journal_path, journal_path.display().to_string());
            assert_eq!(parent_started_source_event_index, 0);
            assert_eq!(resource_source_event_index, 1);
            assert_eq!(parent_started_content_sha256, receipt.parent.content_sha256);
            assert_eq!(resource_content_sha256, receipt.resource.content_sha256);
            assert_eq!(expected_semantic_hash, Some(expected.semantic_hash.clone()));
            assert_eq!(
                suggested_recovery,
                "re-run deterministic import for these source indices"
            );
        }
        other => panic!("unexpected dual-strict failure: {other:?}"),
    }

    let db = Database::new_init().expect("repair db");
    let repaired = DbEvalStore::new(&db)
        .put_parent_started_from_receipt(&evidence, &receipt)
        .expect("deterministic repair import");
    assert_eq!(repaired.semantic_hash, expected.semantic_hash);
    assert_eq!(
        query_transition_event(&db, &repaired.event_id).rows.len(),
        1
    );
    assert_eq!(query_record_refs(&db, &evidence.campaign_id).rows.len(), 2);
}

fn sample_campaign_manifest(campaign_id: CampaignId) -> CampaignManifest {
    CampaignManifest {
        schema_version: crate::campaign::CAMPAIGN_MANIFEST_SCHEMA_VERSION.to_string(),
        campaign_id,
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        dataset_sources: Vec::new(),
        model_id: None,
        provider_slug: None,
        route_source: None,
        required_procedures: Vec::new(),
        instances_root: None,
        batches_root: None,
        eval: EvalCampaignPolicy::default(),
        protocol: ProtocolCampaignPolicy::default(),
        framework: crate::spec::FrameworkConfig::default(),
    }
}

fn sample_admitted_profile(root: &std::path::Path) -> profile::AdmittedRunProfile {
    profile::AdmittedRunProfile {
        commitment: profile::RunProfileCommitment {
            schema_version: profile::RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_string(),
            profile_path: root.join("prototype1/run-profile.toml"),
            sha256: "profile-sha256".to_string(),
            source_path: Some(root.join("operator-profile.toml")),
            admitted_at: "2026-06-23T00:00:00Z".to_string(),
        },
        profile: profile::Prototype1RunProfile {
            schema_version: profile::RUN_PROFILE_SCHEMA_VERSION.to_string(),
            name: "test-profile".to_string(),
            storage: profile::Storage {
                worktree_root: root.join("worktrees"),
                eval: profile::EvalStorage {
                    backend: profile::EvalStorageBackend::DualStrict,
                },
            },
            target: profile::Target::default(),
            model: profile::ModelDefaults::default(),
            search: profile::Search::default(),
            generation: profile::Generation::default(),
            selection: profile::Selection::default(),
            protocol: profile::Protocol::default(),
            execution: profile::Execution::default(),
            control: profile::Control::default(),
        },
    }
}

fn sample_closure_state(campaign_id: CampaignId) -> crate::closure::ClosureState {
    crate::closure::ClosureState {
        schema_version: crate::closure::CLOSURE_STATE_SCHEMA_VERSION.to_string(),
        campaign_id,
        updated_at: "2026-06-23T00:00:00Z".to_string(),
        config: crate::closure::ClosureConfig {
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            model_id: None,
            provider_slug: None,
            route_source: None,
            registry_path: None,
            dataset_sources: Vec::new(),
            required_procedures: Vec::new(),
            instances_root: PathBuf::from("/tmp/instances"),
            batches_root: PathBuf::from("/tmp/batches"),
            framework: crate::spec::FrameworkConfig::default(),
        },
        registry: crate::closure::RegistryClosureSummary {
            expected_total: 1,
            mapped_total: 1,
            missing_total: 0,
            ambiguous_total: 0,
            status: ClosureClass::Complete,
        },
        eval: crate::closure::EvalClosureSummary {
            expected_total: 1,
            complete_total: 1,
            failed_total: 0,
            missing_total: 0,
            partial_total: 0,
            in_progress_total: 0,
            status: ClosureClass::Complete,
            last_transition_at: None,
        },
        protocol: crate::closure::ProtocolClosureSummary {
            expected_total: 1,
            full_total: 0,
            partial_total: 0,
            failed_total: 0,
            missing_total: 1,
            incompatible_total: 0,
            ineligible_total: 0,
            in_progress_total: 0,
            status: ClosureClass::Missing,
            required_procedures: Vec::new(),
            status_by_procedure: BTreeMap::new(),
            last_transition_at: None,
        },
        instances: vec![crate::closure::ClosureInstanceRow {
            instance_id: "instance".to_string(),
            dataset_label: "test".to_string(),
            repo_family: "test".to_string(),
            registry_status: crate::closure::RegistryInstanceStatus::Mapped,
            eval_status: ClosureClass::Complete,
            protocol_status: ClosureClass::Missing,
            eval_failure: None,
            protocol_failure: None,
            artifacts: crate::closure::ClosureArtifactRefs {
                registration_path: Some(PathBuf::from("/tmp/baseline/registration.json")),
                record_path: Some(PathBuf::from("/tmp/baseline/record.json.gz")),
                ..Default::default()
            },
            protocol_procedures: BTreeMap::new(),
            protocol_counts: None,
            last_event_at: None,
        }],
    }
}

fn sample_complete_baseline(campaign_id: CampaignId) -> CompleteBaseline {
    CompleteBaseline::complete(
        campaign_id,
        "parent".to_string(),
        "branch-parent".to_string(),
        "eval-set".to_string(),
        vec![BaselineInstance {
            instance_id: "instance".to_string(),
            registration_path: Some(PathBuf::from("/tmp/baseline/registration.json")),
            record_path: PathBuf::from("/tmp/baseline/record.json.gz"),
            metrics: sample_metrics(),
        }],
    )
    .expect("complete baseline")
}

fn sample_metrics() -> OperationalRunMetrics {
    OperationalRunMetrics {
        tool_calls_total: 1,
        tool_calls_failed: 0,
        patch_attempted: true,
        patch_apply_state: PatchApplyState::Applied,
        submission_artifact_state: SubmissionArtifactState::Nonempty,
        patch_projection_check_state: ploke_records::evaluation::PatchProjectionCheckState::Passed,
        partial_patch_failures: 0,
        same_file_patch_retry_count: 0,
        same_file_patch_max_streak: 0,
        aborted: false,
        aborted_repair_loop: false,
        nonempty_valid_patch: true,
        convergence: true,
        oracle_eligible: true,
    }
}

fn parent_started_evidence(repo_root: PathBuf) -> ParentStartedEvidence {
    ParentStartedEvidence {
        campaign_id: CampaignId::from("campaign"),
        parent_identity: parent_identity(),
        repo_root,
        handoff_runtime_id: None,
        pid: 42,
        parent_recorded_at: RecordedAt(1000),
        resource_recorded_at: RecordedAt(1001),
    }
}

fn parent_started_fixture(root: &std::path::Path) -> (ParentStartedEvidence, ParentStartedReceipt) {
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("repo dir");
    let path = root.join("transition-journal.jsonl");
    let mut journal = PrototypeJournal::new(&path);
    let evidence = parent_started_evidence(repo);
    let mut store = FsEvalStore::new(&mut journal);
    let receipt = store
        .put_parent_started(evidence.clone())
        .expect("fs parent start write");
    (evidence, receipt)
}

fn receipt_from_journal(path: &std::path::Path) -> ParentStartedReceipt {
    let text = fs::read_to_string(path).expect("journal text");
    let mut offset = 0_u64;
    let mut receipts = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let byte_len = line.len();
        receipts.push(JournalAppendReceipt {
            path: path.to_path_buf(),
            source_event_index: index,
            source_line: index + 1,
            byte_start: offset,
            byte_len,
            content_sha256: sha256_for_test(line.as_bytes()),
            payload_json: line.to_string(),
        });
        offset += byte_len as u64 + 1;
    }
    assert_eq!(receipts.len(), 2);
    ParentStartedReceipt {
        parent: receipts.remove(0),
        resource: receipts.remove(0),
    }
}

fn sha256_for_test(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower_for_test(&hasher.finalize())
}

fn hex_lower_for_test(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn query_campaign(db: &Database, campaign_id: &CampaignId) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), campaign_id.to_string().into());
    db.raw_query_params(
        r#"
?[campaign_id, storage_backend, profile_ref_id] :=
    *eval_campaign { campaign_id, storage_backend, profile_ref_id },
    campaign_id = $campaign_id
"#,
        params,
    )
    .expect("query campaign")
}

fn query_profile_commitments(db: &Database, campaign_id: &CampaignId) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), campaign_id.to_string().into());
    db.raw_query_params(
        r#"
?[profile_ref_id, profile_name, content_sha256, storage_ref] :=
    *eval_profile_commitment { profile_ref_id, campaign_id, profile_name, content_sha256, storage_ref },
    campaign_id = $campaign_id
"#,
        params,
    )
    .expect("query profile commitments")
}

fn query_closure_refs(db: &Database, campaign_id: &CampaignId) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), campaign_id.to_string().into());
    db.raw_query_params(
        r#"
?[closure_ref_id, recorded_at, summary_json] :=
    *eval_closure_ref { closure_ref_id, campaign_id, recorded_at, summary_json },
    campaign_id = $campaign_id
"#,
        params,
    )
    .expect("query closure refs")
}

fn query_baselines(db: &Database, campaign_id: &CampaignId) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), campaign_id.to_string().into());
    db.raw_query_params(
        r#"
?[baseline_id, source_kind, instance_count, closure_ref_id] :=
    *eval_baseline { baseline_id, campaign_id, source_kind, instance_count, closure_ref_id },
    campaign_id = $campaign_id
"#,
        params,
    )
    .expect("query baselines")
}

fn query_transition_event(db: &Database, event_id: &str) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert(
        "event_id".to_string(),
        DataValue::from(event_id.to_string()),
    );
    db.raw_query_params(
            r#"
?[event_id, campaign_id, parent_id, node_id, generation, transition, phase, outcome, source_event_index, source_line, content_sha256, semantic_hash] :=
    *eval_transition_event {
        event_id,
        campaign_id,
        parent_id,
        node_id,
        generation,
        transition,
        phase,
        outcome,
        source_event_index,
        source_line,
        content_sha256,
        semantic_hash
    },
    event_id = $event_id
"#,
            params,
        )
        .expect("query transition event")
}

fn query_all_transition_events(db: &Database) -> QueryResult {
    db.raw_query_params(
        r#"
?[event_id] :=
    *eval_transition_event { event_id }
"#,
        BTreeMap::new(),
    )
    .expect("query all transition events")
}

fn query_record_refs(db: &Database, campaign_id: &CampaignId) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        DataValue::from(campaign_id.to_string()),
    );
    db.raw_query_params(
        r#"
?[
    record_ref_id,
    family,
    schema_version,
    store_scope,
    producer_role,
    source_class,
    evidence_class,
    visibility_scope,
    validation_status,
    source_event_index,
    source_line,
    content_sha256,
    payload_json
] :=
    *eval_record_ref {
        record_ref_id,
        campaign_id,
        family,
        schema_version,
        store_scope,
        producer_role,
        source_class,
        evidence_class,
        visibility_scope,
        validation_status,
        source_event_index,
        source_line,
        content_sha256,
        payload_json
    },
    campaign_id = $campaign_id
"#,
        params,
    )
    .expect("query record refs")
}

fn query_log_refs(db: &Database) -> QueryResult {
    db.raw_query_params(
        r#"
?[log_ref_id, log_kind, source_ref, content_sha256] :=
    *eval_log_ref { log_ref_id, log_kind, source_ref, content_sha256 }
"#,
        BTreeMap::new(),
    )
    .expect("query log refs")
}

fn query_trace_events(db: &Database, log_ref_id: &str) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert(
        "source_log_ref".to_string(),
        DataValue::from(log_ref_id.to_string()),
    );
    db.raw_query_params(
            r#"
?[trace_event_id, source_event_index, event_name, stage, transition, span_name, program, exit_code] :=
    *eval_trace_event {
        trace_event_id,
        source_log_ref,
        source_event_index,
        event_name,
        stage,
        transition,
        span_name,
        program,
        exit_code
    },
    source_log_ref = $source_log_ref
"#,
            params,
        )
        .expect("query trace events")
}

fn query_all_trace_events(db: &Database) -> QueryResult {
    db.raw_query_params(
        r#"
?[trace_event_id] :=
    *eval_trace_event { trace_event_id }
"#,
        BTreeMap::new(),
    )
    .expect("query all trace events")
}

fn query_agent_turn(db: &Database, turn_id: &str) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert("turn_id".to_string(), DataValue::from(turn_id.to_string()));
    db.raw_query_params(
        r#"
?[campaign_id, request_id, event_count, response_count, terminal_outcome] :=
    *eval_agent_turn { turn_id, campaign_id, request_id, event_count, response_count, terminal_outcome },
    turn_id = $turn_id
"#,
        params,
    )
    .expect("query agent turn")
}

fn query_agent_turn_events(db: &Database, turn_id: &str) -> QueryResult {
    query_turn_ids(
        db,
        turn_id,
        "eval_agent_turn_event",
        "event_id",
        "query agent turn events",
    )
}

fn query_model_exchanges(db: &Database, turn_id: &str) -> QueryResult {
    query_turn_ids(
        db,
        turn_id,
        "eval_model_exchange",
        "exchange_id",
        "query model exchanges",
    )
}

fn query_message_events(db: &Database, turn_id: &str) -> QueryResult {
    query_turn_ids(
        db,
        turn_id,
        "eval_message_event",
        "message_event_id",
        "query message events",
    )
}

fn query_tool_events(db: &Database, turn_id: &str) -> QueryResult {
    query_turn_ids(
        db,
        turn_id,
        "eval_tool_event",
        "tool_event_id",
        "query tool events",
    )
}

fn query_turn_ids(db: &Database, turn_id: &str, rel: &str, id: &str, label: &str) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert("turn_id".to_string(), DataValue::from(turn_id.to_string()));
    db.raw_query_params(
        &format!(
            r#"
?[id] :=
    *{rel} {{ {id}: id, turn_id }},
    turn_id = $turn_id
"#
        ),
        params,
    )
    .expect(label)
}

fn sample_agent_turn_artifact() -> AgentTurnArtifactRecord {
    AgentTurnArtifactRecord {
        task_id: "request-1".to_string(),
        selected_model: "test/model".to_string(),
        model_route: Some(ModelRouteRecord {
            route_source: "direct-google".to_string(),
            router: "test-router".to_string(),
            provider_slug: Some("google".to_string()),
            endpoint_host: Some("example.invalid".to_string()),
        }),
        issue_prompt: "fix the test".to_string(),
        user_message_id: "user-1".to_string(),
        events: vec![
            ObservedTurnEventRecord::ToolRequested(ToolRequestRecord {
                request_id: "request-1".to_string(),
                parent_id: "parent-1".to_string(),
                call_id: "call-1".to_string(),
                tool: "apply_code_edit".to_string(),
                arguments: ToolArgumentsJson::from(r#"{"path":"src/lib.rs"}"#.to_string()),
            }),
            ObservedTurnEventRecord::ToolCompleted(ToolCompletedRecord {
                request_id: "request-1".to_string(),
                parent_id: "parent-1".to_string(),
                call_id: "call-1".to_string(),
                tool: "apply_code_edit".to_string(),
                content: "applied".to_string(),
                ui_payload: None,
                latency_ms: 7,
            }),
            ObservedTurnEventRecord::MessageUpdated(MessageSnapshotRecord {
                id: "assistant-1".to_string(),
                kind: "assistant".to_string(),
                status: "complete".to_string(),
                tool_call_id: None,
                content_len: 4,
                content_preview: "done".to_string(),
            }),
            ObservedTurnEventRecord::TurnFinished(TurnFinishedRecord {
                session_id: "session-1".to_string(),
                request_id: "request-1".to_string(),
                parent_id: "parent-1".to_string(),
                assistant_message_id: "assistant-1".to_string(),
                outcome: "applied".to_string(),
                error_id: None,
                summary: "done".to_string(),
                attempts: 1,
            }),
        ],
        prompt_debug: None,
        terminal_record: Some(TurnFinishedRecord {
            session_id: "session-1".to_string(),
            request_id: "request-1".to_string(),
            parent_id: "parent-1".to_string(),
            assistant_message_id: "assistant-1".to_string(),
            outcome: "applied".to_string(),
            error_id: None,
            summary: "done".to_string(),
            attempts: 1,
        }),
        final_assistant_message: Some(MessageSnapshotRecord {
            id: "assistant-1".to_string(),
            kind: "assistant".to_string(),
            status: "complete".to_string(),
            tool_call_id: None,
            content_len: 4,
            content_preview: "done".to_string(),
        }),
        patch_artifact: PatchArtifactRecord {
            edit_proposals: Vec::new(),
            create_proposals: Vec::new(),
            applied: true,
            all_proposals_applied: true,
            expected_file_changes: Vec::new(),
            any_expected_file_changed: true,
            all_expected_files_changed: true,
        },
        llm_prompt: vec![RequestMessageRecord {
            role: RequestRoleRecord::User,
            content: "fix the test".to_string(),
            tool_call_id: None,
            tool_calls: None,
        }],
        llm_response: Some("done".to_string()),
    }
}

fn sample_raw_full_response() -> RawFullResponseRecord {
    let response = serde_json::from_value(serde_json::json!({
        "id": "provider-response-1",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {
                "role": "assistant",
                "content": "done"
            }
        }],
        "created": 0,
        "model": "test/model",
        "object": "chat.completion",
        "usage": {
            "prompt_tokens": 5,
            "completion_tokens": 1,
            "total_tokens": 6
        }
    }))
    .expect("sample response");
    RawFullResponseRecord {
        assistant_message_id: uuid::Uuid::parse_str("8e32b33b-6de5-4e1c-9fa1-14bc2059913f")
            .expect("assistant uuid"),
        recorded_response: ploke_llm::manager::RecordedResponse::new(0, response),
    }
}

fn parent_entry(repo_root: PathBuf) -> ParentStartedEntry {
    let evidence = parent_started_evidence(repo_root);
    ParentStartedEntry {
        recorded_at: evidence.parent_recorded_at,
        campaign_id: evidence.campaign_id,
        parent_identity: evidence.parent_identity,
        repo_root: evidence.repo_root,
        handoff_runtime_id: evidence.handoff_runtime_id,
        pid: evidence.pid,
    }
}

fn parent_identity() -> ParentIdentity {
    ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
        campaign_id: CampaignId::from("campaign"),
        parent_id: "parent".to_string(),
        node_id: "parent".to_string(),
        generation: 0,
        instance_id: Some("instance".to_string()),
        previous_parent_id: None,
        parent_node_id: None,
        branch_id: "branch-parent".to_string(),
        artifact_branch: Some("artifact-parent".to_string()),
        created_at: "2026-06-22T00:00:00Z".to_string(),
    })
}
