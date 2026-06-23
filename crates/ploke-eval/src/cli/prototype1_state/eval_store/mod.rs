//! Prototype 1 eval evidence storage port.
#![cfg_attr(not(test), allow(dead_code))]
//!
//! This module is Domain-C only: it records ordinary eval evidence/projections
//! and must not replace History, channel transport, MessageBox authority,
//! invocation/bootstrap, or artifact/worktree mutation.

mod api;
mod artifact;
mod continuation;
mod cozo_params;
mod cozo_schema;
mod cozo_store;
mod error;
mod evaluation;
mod evidence;
mod observation;
mod selection;

#[cfg(test)]
mod tests;

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
pub(crate) use continuation::CONTINUATION_DECISION_REL;
pub(crate) use continuation::{
    ContinuationDecisionEvidence, write_continuation_decision_to_owner_db,
};
#[cfg(test)]
pub(crate) use cozo_store::{DbEvalStore, load_owner_eval_database};
pub(crate) use cozo_store::{
    owner_eval_db_file_for_record_path, prototype1_eval_store_db_path,
    write_channel_message_to_owner_db, write_channel_receipt_to_owner_db,
    write_import_event_to_owner_db, write_invocation_to_owner_db, write_record_ref_to_owner_db,
    write_trace_event_to_owner_db,
};
pub(crate) use error::EvalStoreError;
#[cfg(test)]
pub(crate) use evaluation::{EVALUATION_INSTANCE_REL, EVALUATION_REL};
pub(crate) use evaluation::{
    EvaluationEvidence, EvaluationInstanceEvidence, write_evaluation_to_owner_db,
};
pub(crate) use evidence::{
    ChannelMessageEvidence, ChannelReceiptEvidence, ImportEventEvidence, InvocationEvidence,
    ParentStartedEvidence, RecordRefEvidence, TraceEventEvidence,
};
#[cfg(test)]
pub(crate) use evidence::{LogRefEvidence, ParentStartedReceipt};
#[cfg(test)]
pub(crate) use observation::ObservationJsonlImport;
#[cfg(test)]
pub(crate) use selection::{
    SELECTION_CANDIDATE_REL, SELECTION_DECISION_REL, SELECTION_FINDING_REL, SELECTION_SCORE_REL,
};
pub(crate) use selection::{SelectionDecisionEvidence, write_selection_decision_to_owner_db};
