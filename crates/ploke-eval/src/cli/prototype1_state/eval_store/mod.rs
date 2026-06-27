//! Prototype 1 eval evidence storage port.
#![cfg_attr(not(test), allow(dead_code))]
//!
//! This module is Domain-C only: it records ordinary eval evidence/projections
//! and must not replace History, channel transport, MessageBox authority,
//! invocation/bootstrap, or artifact/worktree mutation.

mod agent_turn;
mod api;
mod artifact;
mod build;
mod child_plan;
mod continuation;
mod cozo_params;
mod cozo_schema;
mod cozo_store;
mod error;
mod evaluation;
mod evidence;
mod observation;
mod operation;
mod schema;
mod selection;
mod setup;

#[cfg(test)]
mod tests;

pub(crate) use agent_turn::AgentTurnBundleEvidence;
#[cfg(test)]
pub(crate) use agent_turn::{
    AGENT_TURN_EVENT_REL, AGENT_TURN_REL, MESSAGE_EVENT_REL, MODEL_EXCHANGE_REL, TOOL_EVENT_REL,
};
#[cfg(test)]
pub(crate) use agent_turn::{AgentTurnEvidence, write_agent_turn_to_owner_db};
pub(crate) use api::{ConfiguredEvalStore, EvalStore};
#[cfg(test)]
pub(crate) use api::{FileDbEvalStore, FsEvalStore};
#[cfg(test)]
pub(crate) use artifact::{ARTIFACT_REF_REL, ARTIFACT_REL, ARTIFACT_SURFACE_REL};
pub(crate) use artifact::{
    ArtifactEvidence, ArtifactProvenanceEvidence, ArtifactRefEvidence, ArtifactSurfaceEvidence,
    artifact_surface_hash, write_artifact_provenance_to_owner_db,
};
#[cfg(test)]
pub(crate) use build::{BINARY_REF_REL, BUILD_EVENT_REL};
pub(crate) use build::{
    BinaryRefEvidence, BuildEventEvidence, BuildProvenanceEvidence, file_sha256,
    write_build_provenance_to_owner_db,
};
#[cfg(test)]
pub(crate) use child_plan::{CHILD_PLAN_CHILD_REL, CHILD_PLAN_REJECTED_REL, CHILD_PLAN_REL};
pub(crate) use child_plan::{
    CHILD_PLAN_SCHEMA_VERSION, ChildPlanEvidence, write_child_plan_to_owner_db,
};
#[cfg(test)]
pub(crate) use continuation::CONTINUATION_DECISION_REL;
pub(crate) use continuation::{
    ContinuationDecisionEvidence, write_continuation_decision_to_owner_db,
};
#[cfg(test)]
pub(crate) use cozo_store::DbEvalStore;
pub(crate) use cozo_store::{
    load_owner_eval_database, owner_eval_db_file_for_record_path, prototype1_eval_store_db_path,
    write_baseline_to_owner_db, write_channel_message_to_owner_db,
    write_channel_receipt_to_owner_db, write_closure_state_to_owner_db,
    write_import_event_to_owner_db, write_invocation_to_owner_db, write_log_ref_to_owner_db,
    write_r0_context_to_owner_db, write_record_ref_to_owner_db, write_trace_event_to_owner_db,
};
pub(crate) use error::EvalStoreError;
#[cfg(test)]
pub(crate) use evaluation::{EVALUATION_INSTANCE_REL, EVALUATION_REL};
pub(crate) use evaluation::{
    EvaluationEvidence, EvaluationInstanceEvidence, write_evaluation_to_owner_db,
};
#[cfg(test)]
pub(crate) use evidence::ParentStartedReceipt;
pub(crate) use evidence::{
    ChannelMessageEvidence, ChannelReceiptEvidence, ImportEventEvidence, InvocationEvidence,
    LogRefEvidence, ParentStartedEvidence, RecordRefEvidence, TraceEventEvidence,
};
#[cfg(test)]
pub(crate) use observation::ObservationJsonlImport;
#[cfg(test)]
pub(crate) use operation::{APPLY_EVENT_REL, OPERATION_REL, PATCH_REL};
pub(crate) use operation::{
    ApplyEventEvidence, OperationEvidence, OperationProvenanceEvidence, PatchEvidence,
    content_sha256, write_operation_provenance_to_owner_db,
};
#[cfg(test)]
pub(crate) use selection::{
    SELECTION_CANDIDATE_REL, SELECTION_DECISION_REL, SELECTION_FINDING_REL, SELECTION_SCORE_REL,
};
pub(crate) use selection::{SelectionDecisionEvidence, write_selection_decision_to_owner_db};
#[cfg(test)]
pub(crate) use setup::{
    BASELINE_INSTANCE_METRICS_REL, BASELINE_INSTANCE_REL, BASELINE_REL, CAMPAIGN_EVAL_BUDGET_REL,
    CAMPAIGN_EVAL_POLICY_REL, CAMPAIGN_PROTOCOL_POLICY_REL, CAMPAIGN_REL, CLOSURE_ARTIFACT_REF_REL,
    CLOSURE_INSTANCE_REL, CLOSURE_PROTOCOL_COUNTS_REL, CLOSURE_PROTOCOL_PROCEDURE_REL,
    CLOSURE_REF_REL, PROFILE_COMMITMENT_REL,
};
