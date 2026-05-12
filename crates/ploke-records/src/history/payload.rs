use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{ActorRefRecord, EvidenceRefRecord, ProcedureRefRecord, SubjectRefRecord};
use crate::branch::ResolvedTreatmentBranch;
use crate::evaluation::{EvalSet, Evaluator, RunMetrics};
use crate::ids::{
    ArtifactId, BlockHash, BlockId, CandidateMembershipId, CandidateOccurrenceId, HistoryHash,
    LineageId, PatchId, RecordedAt,
};
use crate::scheduler::NodeRecord;
use crate::selection;

/// Import disposition committed to the History entry produced by ingress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportDispositionRecord {
    AcceptedAsObservation,
    AcceptedAsLateTerminalStatus,
    AcceptedAsDiagnosticOnly,
}

/// Ingress chain-of-custody payload sealed with an imported entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngressImportRecord {
    pub ingress_id: String,
    pub prior_block_hash: BlockHash,
    pub original_payload_ref: EvidenceRefRecord,
    pub original_payload_hash: HistoryHash,
    pub observed_by: ActorRefRecord,
    pub observed_at: RecordedAt,
    pub recorded_by: ActorRefRecord,
    pub recorded_at: RecordedAt,
    pub imported_by: ActorRefRecord,
    pub import_policy: ProcedureRefRecord,
    pub imported_at: RecordedAt,
    pub import_disposition: ImportDispositionRecord,
    pub imported_into_lineage: LineageId,
    pub imported_into_block: BlockId,
    pub imported_into_height: u64,
}

/// Explicit candidate coordinate for sealed selection replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateCoordinateRecord {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_runtime_id: Option<String>,
}

/// Lifecycle outcome labels from the planner and persisted node status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateLifecycleRecord {
    pub planner_outcome: String,
    pub node_status: String,
}

/// Citation to a typed evidence source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCitationRecord {
    pub ref_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<HistoryHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_name: Option<String>,
}

/// Role classification for one concrete run attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunRoleRecord {
    Control,
    Treatment,
}

/// Stable artifact references for one run attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunArtifactRefsRecord {
    pub run_manifest: PathBuf,
    pub run_root: PathBuf,
    pub repo_state: PathBuf,
    pub execution_log: PathBuf,
    pub indexing_status: PathBuf,
    pub parse_failure: PathBuf,
    pub snapshot_status: PathBuf,
    pub indexing_checkpoint_db: PathBuf,
    pub indexing_failure_db: PathBuf,
    pub record_path: PathBuf,
    pub final_snapshot: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_trace: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_summary: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_response_trace: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msb_submission: Option<PathBuf>,
    pub protocol_artifacts_dir: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_anchor: Option<PathBuf>,
}

/// Protocol artifact summary associated with run evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolArtifactSummaryRecord {
    pub path: PathBuf,
    pub schema_version: String,
    pub procedure_name: String,
    pub subject_id: String,
    pub run_id: String,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
}

/// Diagnostic attached to protocol evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDiagnosticRecord {
    pub severity: String,
    pub field: String,
    pub message: String,
}

/// Protocol artifact evidence associated with one run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolArtifactsRecord {
    pub artifacts_dir: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ProtocolArtifactSummaryRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ProtocolDiagnosticRecord>,
}

/// Typed run evidence referenced from sealed compared-run evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEvidenceRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration_path: Option<PathBuf>,
    pub run_id: String,
    pub task_id: String,
    pub run_role: RunRoleRecord,
    pub spec_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    pub artifacts: RunArtifactRefsRecord,
    #[serde(default)]
    pub protocol: ProtocolArtifactsRecord,
}

/// Compared-run slice carried from child evaluation evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparedRunEvidenceRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_citation: Option<EvidenceCitationRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_citation: Option<EvidenceCitationRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_metrics: Option<RunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_metrics: Option<RunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_protocol: Option<ProtocolMetricsRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_protocol: Option<ProtocolMetricsRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_run: Option<RunEvidenceRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_run: Option<RunEvidenceRecord>,
}

/// Protocol aggregate metrics considered by selection and History traversal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolMetricsRecord {
    pub scanned_artifact_count: usize,
    pub artifact_counts: std::collections::BTreeMap<String, usize>,
    pub total_calls_in_run: usize,
    pub total_segments_in_anchor: usize,
    pub reviewed_call_count: usize,
    pub reviewed_segment_count: usize,
    pub missing_call_count: usize,
    pub missing_segment_count: usize,
    pub skipped_segment_review_count: usize,
    pub segment_anchor_mismatch_count: usize,
    pub call_review_overall_counts: std::collections::BTreeMap<String, usize>,
    pub segment_review_overall_counts: std::collections::BTreeMap<String, usize>,
    pub call_review_confidence_counts: std::collections::BTreeMap<String, usize>,
    pub segment_review_confidence_counts: std::collections::BTreeMap<String, usize>,
    pub calls_with_segment_crosswalk: usize,
    pub calls_without_segment_crosswalk: usize,
    pub average_calls_per_anchor_segment_x1000: u64,
    pub review_signal_totals: std::collections::BTreeMap<String, usize>,
}

/// One branch evaluation report worth of sealed material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationEvidenceRecord {
    pub branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_procedure_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluator_identity: Option<Evaluator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_set_identity: Option<EvalSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_artifact_citation: Option<EvidenceCitationRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_disposition: Option<String>,
    pub primary_report_citation: EvidenceCitationRecord,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compared_runs: Vec<ComparedRunEvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEvidenceRecord {
    pub runtime_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub document_citations: Vec<EvidenceCitationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub journal_citations: Vec<EvidenceCitationRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchEvidenceRecord {
    pub branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_state_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_evidence_citations: Vec<EvidenceCitationRecord>,
}

/// Snapshot mirroring grouped child/evaluation evidence for sealed History.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateEvidenceRecord {
    pub schema_version: u32,
    pub coordinate: CandidateCoordinateRecord,
    pub lifecycle: CandidateLifecycleRecord,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evaluations: Vec<EvaluationEvidenceRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtimes: Vec<RuntimeEvidenceRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<BranchEvidenceRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_document_citations: Vec<EvidenceCitationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_journal_citations: Vec<EvidenceCitationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceArtifactRefRecord {
    pub artifact_id: ArtifactId,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceTouchRecord {
    pub target_relpath: PathBuf,
    pub target_name: String,
    pub span_relpath: PathBuf,
    pub start: usize,
    pub end: usize,
    pub base_hash: String,
    pub replacement: String,
    pub replacement_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceCheckStatusRecord {
    Checked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceApplyStatusRecord {
    Applied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RequestPolicyOriginRecord {
    Explicit,
    #[default]
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveRequestPolicyRecord<T> {
    pub value: T,
    pub origin: RequestPolicyOriginRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestPolicyResponseFormatRecord {
    None,
    JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestPolicyProviderRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<RequestPolicyMaxPriceRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RequestPolicyMaxPriceRecord {
    pub prompt_tokens: Option<f64>,
    pub completion_tokens: Option<f64>,
    pub request: Option<f64>,
}

impl Eq for RequestPolicyMaxPriceRecord {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RequestParameterPolicyRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<EffectiveRequestPolicyRecord<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<EffectiveRequestPolicyRecord<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<EffectiveRequestPolicyRecord<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<EffectiveRequestPolicyRecord<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<EffectiveRequestPolicyRecord<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<EffectiveRequestPolicyRecord<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<EffectiveRequestPolicyRecord<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<EffectiveRequestPolicyRecord<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<EffectiveRequestPolicyRecord<Vec<(i32, String)>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<EffectiveRequestPolicyRecord<i32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_p: Option<EffectiveRequestPolicyRecord<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_a: Option<EffectiveRequestPolicyRecord<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<EffectiveRequestPolicyRecord<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestObjectiveBindingRecord {
    pub summary: String,
    pub target_metric: String,
    pub writable_intent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RequestProposalBindingRecord {
    pub proposal_id: String,
    pub run_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestPayloadHashRecord {
    Known { value: String },
    Unknown { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestPolicyReceiptRecord {
    pub schema_version: u32,
    pub base_artifact_id: ArtifactId,
    pub objective: RequestObjectiveBindingRecord,
    #[serde(default)]
    pub proposal: RequestProposalBindingRecord,
    pub router: String,
    pub model: EffectiveRequestPolicyRecord<String>,
    pub response_format: EffectiveRequestPolicyRecord<RequestPolicyResponseFormatRecord>,
    pub stop: EffectiveRequestPolicyRecord<Vec<String>>,
    pub stream: EffectiveRequestPolicyRecord<bool>,
    pub parameters: RequestParameterPolicyRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<RequestPolicyProviderRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_payload_hash: Option<RequestPayloadHashRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_payload_hash: Option<RequestPayloadHashRecord>,
    pub client_policy_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SurfaceProposalProducerRecord {
    NonRouter,
    Router {
        request_policy: RequestPolicyReceiptRecord,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratorSourceKindRecord {
    Named,
    Inline,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorSurfaceVersionRecord {
    pub projection_id: String,
    pub projection_hash: String,
    pub bounds_digest: String,
    pub source_kind: GeneratorSourceKindRecord,
    pub source_id: String,
    pub source_version: String,
}

/// Serializable evidence for one checked edit-surface candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceEvidenceRecord {
    pub schema_version: u32,
    pub producer_id: String,
    pub proposal_id: String,
    pub run_id: String,
    pub policy: String,
    pub target_relpath: PathBuf,
    pub base: SurfaceArtifactRefRecord,
    pub after: SurfaceArtifactRefRecord,
    pub patch_id: PatchId,
    pub source_content_hash: String,
    pub proposed_content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_producer: Option<SurfaceProposalProducerRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_surface: Option<GeneratorSurfaceVersionRecord>,
    pub touches: Vec<SurfaceTouchRecord>,
    pub touches_digest: HistoryHash,
    pub delta_id: String,
    pub delta_digest: HistoryHash,
    pub check_status: SurfaceCheckStatusRecord,
    pub apply_status: SurfaceApplyStatusRecord,
}

static DEFAULT_SURFACE_PROPOSAL_PRODUCER: SurfaceProposalProducerRecord =
    SurfaceProposalProducerRecord::NonRouter;

impl SurfaceEvidenceRecord {
    pub fn effective_proposal_producer(&self) -> &SurfaceProposalProducerRecord {
        self.proposal_producer
            .as_ref()
            .unwrap_or(&DEFAULT_SURFACE_PROPOSAL_PRODUCER)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SurfaceAttemptOutcomeRecord {
    Applied,
    Rejected { reason: String },
}

/// Durable attempt evidence for one edit-surface proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceAttemptRecord {
    pub schema_version: u32,
    pub producer_id: String,
    pub proposal_id: String,
    pub run_id: String,
    pub policy: String,
    pub target_relpath: PathBuf,
    pub outcome: SurfaceAttemptOutcomeRecord,
}

/// Candidate-local artifact payload sealed with selection evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateArtifactRecord {
    pub schema_version: u32,
    pub node: NodeRecord,
    pub resolved: ResolvedTreatmentBranch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<SurfaceEvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectionFailureIdRecord(pub HistoryHash);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionFailureKindRecord {
    MissingSelectionInput,
    ChildEvidenceStoreLoadFailed,
    SelectionInputBindingInvalid,
    SelectionProcedureMismatch,
    CandidateSetMembershipMissing,
    CandidateSetPayloadHashMismatch,
    CandidateSetProofInvalid,
    DecisionGradeIneligible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionFailureRecord {
    pub id: ProjectionFailureIdRecord,
    pub kind: ProjectionFailureKindRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate: Option<SubjectRefRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_message: Option<String>,
}

/// Inline-first per-candidate evaluation payload suitable for sealing in History.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationPayloadRecord {
    pub schema_version: u32,
    pub candidate: SubjectRefRecord,
    pub procedure: ProcedureRefRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_input: Option<selection::Input>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_input_hash: Option<HistoryHash>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection_failures: Vec<ProjectionFailureRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<EvidenceRefRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_hashes: Vec<HistoryHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_evidence: Option<CandidateEvidenceRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<CandidateArtifactRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_attempt: Option<SurfaceAttemptRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CandidateSetRootRecord(pub HistoryHash);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateSetProofRecord {
    pub key: [u8; 32],
    pub value: [u8; 32],
    pub program: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateSetMembershipRecord {
    pub candidate: SubjectRefRecord,
    pub payload_hash: HistoryHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_id: Option<CandidateOccurrenceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub membership_id: Option<CandidateMembershipId>,
    pub proof: CandidateSetProofRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateSetRecord {
    pub root: CandidateSetRootRecord,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memberships: Vec<CandidateSetMembershipRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SelectionScopeRecord {
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraversalCandidateSourceRecord {
    History,
    CurrentGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TraversalMetricInputsRecord {
    #[default]
    Operational,
    OperationalAndProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TraversalStrategyRecord {
    FrontierMax {
        normalize_frontier: bool,
        #[serde(default)]
        metrics: TraversalMetricInputsRecord,
    },
    ScoreChildProp {
        top_m: usize,
        lambda_millis: u32,
        #[serde(default)]
        metrics: TraversalMetricInputsRecord,
    },
}

impl Default for TraversalStrategyRecord {
    fn default() -> Self {
        Self::FrontierMax {
            normalize_frontier: true,
            metrics: TraversalMetricInputsRecord::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversalEvidenceRecord {
    pub seed: u64,
    #[serde(default)]
    pub strategy: TraversalStrategyRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_source: Option<TraversalCandidateSourceRecord>,
}

/// Selection decision entry sealed in a History entry payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionDecisionEntryRecord {
    pub schema_version: u32,
    pub procedure_or_policy: ProcedureRefRecord,
    pub scope: SelectionScopeRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_candidate: Option<SubjectRefRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_occurrence_id: Option<CandidateOccurrenceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_membership_id: Option<CandidateMembershipId>,
    #[serde(default)]
    pub considered: Vec<EvaluationPayloadRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub considered_sources: Vec<TraversalCandidateSourceRecord>,
    pub considered_order_hash: HistoryHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_set: Option<CandidateSetRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection_failures: Vec<ProjectionFailureRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traversal: Option<TraversalEvidenceRecord>,
    pub decision: selection::Decision,
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::{Value, json};

    use super::{
        GeneratorSourceKindRecord, RequestPayloadHashRecord, RequestPolicyOriginRecord,
        RequestPolicyResponseFormatRecord, SelectionDecisionEntryRecord, SurfaceEvidenceRecord,
        SurfaceProposalProducerRecord, TraversalCandidateSourceRecord, TraversalEvidenceRecord,
        TraversalMetricInputsRecord, TraversalStrategyRecord,
    };

    fn minimal_surface_evidence_value() -> Value {
        json!({
            "schema_version": 1,
            "producer_id": "prototype1:edit-surface:v1",
            "proposal_id": "proposal-1",
            "run_id": "run-1",
            "policy": "workspace_except_ploke_eval",
            "target_relpath": "src/lib.rs",
            "base": {
                "artifact_id": "git-tree:source",
                "hash": "sha256:source"
            },
            "after": {
                "artifact_id": "git-commit:derived",
                "hash": "sha256:applied"
            },
            "patch_id": "patch:1",
            "source_content_hash": "sha256:source",
            "proposed_content_hash": "sha256:proposed",
            "touches": [{
                "target_relpath": "src/lib.rs",
                "target_name": "direct-splice:eof-comment",
                "span_relpath": "src/lib.rs",
                "start": 12,
                "end": 12,
                "base_hash": "sha256:source",
                "replacement": " println!(\"hi\");",
                "replacement_hash": "sha256:replacement"
            }],
            "touches_digest": "sha256:touches",
            "delta_id": "surface-delta:sha256:delta",
            "delta_digest": "sha256:delta",
            "check_status": "checked",
            "apply_status": "applied"
        })
    }

    #[test]
    fn traversal_evidence_accepts_structured_strategy_shape() {
        let value = json!({
            "seed": 0,
            "strategy": {
                "kind": "score_child_prop",
                "top_m": 3,
                "lambda_millis": 10000,
                "metrics": "operational_and_protocol"
            },
            "selected_source": "current_generation"
        });

        let parsed: TraversalEvidenceRecord =
            serde_json::from_value(value).expect("parse traversal evidence");
        assert_eq!(parsed.seed, 0);
        assert_eq!(
            parsed.strategy,
            TraversalStrategyRecord::ScoreChildProp {
                top_m: 3,
                lambda_millis: 10_000,
                metrics: TraversalMetricInputsRecord::OperationalAndProtocol,
            }
        );
        assert_eq!(
            parsed.selected_source,
            Some(TraversalCandidateSourceRecord::CurrentGeneration)
        );
    }

    #[test]
    fn selection_decision_accepts_occurrence_membership_identity_shape() {
        let occurrence_id = "a".repeat(64);
        let membership_id = "b".repeat(64);
        let payload_hash = "c".repeat(64);
        let root = "d".repeat(64);
        let considered_order_hash = "e".repeat(64);
        let value = json!({
            "schema_version": 3,
            "procedure_or_policy": {"value": "prototype1.successor_selection.history_traversal.v1"},
            "scope": "history",
            "selected_candidate": {"value": "candidate:node-a:plan_index=0"},
            "selected_occurrence_id": occurrence_id,
            "selected_membership_id": membership_id,
            "considered": [{
                "schema_version": 1,
                "candidate": {"value": "candidate:node-a:plan_index=0"},
                "procedure": {"value": "prototype1.successor_selection.v1"}
            }],
            "considered_sources": ["current_generation"],
            "considered_order_hash": considered_order_hash,
            "candidate_set": {
                "root": root,
                "memberships": [{
                    "candidate": {"value": "candidate:node-a:plan_index=0"},
                    "payload_hash": payload_hash,
                    "occurrence_id": "a".repeat(64),
                    "membership_id": "b".repeat(64),
                    "proof": {
                        "key": vec![0; 32],
                        "value": vec![1; 32],
                        "program": []
                    }
                }]
            },
            "decision": {
                "procedure_id": "prototype1.successor_selection.history_traversal.v1",
                "candidate_node_id": "node-a",
                "selected_branch_id": "branch-a",
                "branch_disposition": "keep",
                "outcome": "accepted"
            }
        });

        let parsed: SelectionDecisionEntryRecord =
            serde_json::from_value(value).expect("parse occurrence-aware selection decision");
        assert_eq!(parsed.schema_version, 3);
        assert_eq!(
            parsed
                .selected_occurrence_id
                .as_ref()
                .map(|id| id.0.as_str()),
            Some(occurrence_id.as_str())
        );
        assert_eq!(
            parsed
                .selected_membership_id
                .as_ref()
                .map(|id| id.0.as_str()),
            Some(membership_id.as_str())
        );
        let member = parsed
            .candidate_set
            .as_ref()
            .and_then(|set| set.memberships.first())
            .expect("membership");
        assert_eq!(
            member.occurrence_id.as_ref().map(|id| id.0.as_str()),
            Some(occurrence_id.as_str())
        );
        assert_eq!(
            member.membership_id.as_ref().map(|id| id.0.as_str()),
            Some(membership_id.as_str())
        );
    }

    #[test]
    fn surface_evidence_roundtrips_router_proposal_producer() {
        let value = json!({
            "schema_version": 2,
            "producer_id": "prototype1:router-edit-surface:v1",
            "proposal_id": "proposal-router-1",
            "run_id": "run-router-1",
            "policy": "workspace_except_ploke_eval",
            "target_relpath": "src/lib.rs",
            "base": {
                "artifact_id": "git-tree:source",
                "hash": "sha256:source"
            },
            "after": {
                "artifact_id": "git-commit:derived",
                "hash": "sha256:applied"
            },
            "patch_id": "patch:router-1",
            "source_content_hash": "sha256:source",
            "proposed_content_hash": "sha256:proposed",
            "proposal_producer": {
                "kind": "router",
                "request_policy": {
                    "schema_version": 1,
                    "base_artifact_id": "git-tree:source",
                    "objective": {
                        "summary": "Resolve invalid candidate generation",
                        "target_metric": "candidate_generation_validity",
                        "writable_intent": "semantic_edit_resolution"
                    },
                    "proposal": {
                        "proposal_id": "proposal-router-1",
                        "run_id": "run-router-1"
                    },
                    "router": "openrouter",
                    "model": {
                        "value": "openai/gpt-5",
                        "origin": "explicit"
                    },
                    "response_format": {
                        "value": "json_object",
                        "origin": "explicit"
                    },
                    "stop": {
                        "value": ["</patch>"],
                        "origin": "default"
                    },
                    "stream": {
                        "value": false,
                        "origin": "default"
                    },
                    "parameters": {
                        "max_tokens": {
                            "value": 2048,
                            "origin": "explicit"
                        },
                        "temperature": {
                            "value": "0.2",
                            "origin": "explicit"
                        },
                        "seed": {
                            "value": 7,
                            "origin": "default"
                        }
                    },
                    "provider": {
                        "order": ["openai"],
                        "allow_fallbacks": false,
                        "require_parameters": true,
                        "max_price": {
                            "prompt_tokens": 0.000002,
                            "completion_tokens": 0.000008,
                            "request": null
                        }
                    },
                    "request_payload_hash": {
                        "kind": "known",
                        "value": "sha256:request"
                    },
                    "response_payload_hash": {
                        "kind": "unknown",
                        "reason": "response not received"
                    },
                    "client_policy_hash": "sha256:client-policy"
                }
            },
            "generator_surface": {
                "projection_id": "router-generator-projection",
                "projection_hash": "sha256:projection",
                "bounds_digest": "sha256:bounds",
                "source_kind": "derived",
                "source_id": "router-edit-surface",
                "source_version": "request-policy-v1"
            },
            "touches": [{
                "target_relpath": "src/lib.rs",
                "target_name": "direct-splice:eof-comment",
                "span_relpath": "src/lib.rs",
                "start": 12,
                "end": 12,
                "base_hash": "sha256:source",
                "replacement": " println!(\"hi\");",
                "replacement_hash": "sha256:replacement"
            }],
            "touches_digest": "sha256:touches",
            "delta_id": "surface-delta:sha256:delta",
            "delta_digest": "sha256:delta",
            "check_status": "checked",
            "apply_status": "applied"
        });

        let parsed: SurfaceEvidenceRecord =
            serde_json::from_value(value.clone()).expect("parse router surface evidence");

        assert_eq!(parsed.schema_version, 2);
        assert_eq!(
            parsed
                .generator_surface
                .as_ref()
                .expect("generator surface is present")
                .source_kind,
            GeneratorSourceKindRecord::Derived
        );
        match parsed.effective_proposal_producer() {
            SurfaceProposalProducerRecord::Router { request_policy } => {
                assert_eq!(request_policy.base_artifact_id.0, parsed.base.artifact_id.0);
                assert_eq!(request_policy.proposal.proposal_id, parsed.proposal_id);
                assert_eq!(request_policy.proposal.run_id, parsed.run_id);
                assert_eq!(
                    request_policy.model.origin,
                    RequestPolicyOriginRecord::Explicit
                );
                assert_eq!(
                    request_policy.response_format.value,
                    RequestPolicyResponseFormatRecord::JsonObject
                );
                assert_eq!(
                    request_policy.request_payload_hash,
                    Some(RequestPayloadHashRecord::Known {
                        value: "sha256:request".to_string()
                    })
                );
                assert!(request_policy.provider.is_some());
            }
            SurfaceProposalProducerRecord::NonRouter => panic!("expected router proposal producer"),
        }

        let roundtrip = serde_json::to_value(&parsed).expect("serialize router surface evidence");
        assert_eq!(roundtrip, value);
    }

    #[test]
    fn surface_evidence_accepts_missing_default_provenance_fields() {
        let value = minimal_surface_evidence_value();

        let parsed: SurfaceEvidenceRecord =
            serde_json::from_value(value.clone()).expect("parse legacy surface evidence");

        assert_eq!(
            parsed.effective_proposal_producer(),
            &SurfaceProposalProducerRecord::NonRouter
        );
        assert!(parsed.proposal_producer.is_none());
        assert!(parsed.generator_surface.is_none());

        let roundtrip = serde_json::to_value(&parsed).expect("serialize legacy surface evidence");
        assert_eq!(roundtrip, value);
    }

    #[test]
    fn surface_evidence_preserves_explicit_non_router_proposal_producer() {
        let mut value = minimal_surface_evidence_value();
        value
            .as_object_mut()
            .expect("surface evidence object")
            .insert(
                "proposal_producer".to_string(),
                json!({ "kind": "non_router" }),
            );

        let parsed: SurfaceEvidenceRecord =
            serde_json::from_value(value.clone()).expect("parse explicit non-router evidence");

        assert_eq!(
            parsed.proposal_producer.as_ref(),
            Some(&SurfaceProposalProducerRecord::NonRouter)
        );
        assert_eq!(
            parsed.effective_proposal_producer(),
            &SurfaceProposalProducerRecord::NonRouter
        );

        let roundtrip =
            serde_json::to_value(&parsed).expect("serialize explicit non-router evidence");
        assert_eq!(roundtrip, value);
    }

    #[test]
    fn legacy_string_strategy_fails_at_strategy_path() {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct LegacyTraversalEvidenceRecord {
            seed: u64,
            strategy: String,
            selected_source: Option<TraversalCandidateSourceRecord>,
        }

        let json = r#"{
            "seed": 0,
            "strategy": {
                "kind": "score_child_prop",
                "top_m": 3,
                "lambda_millis": 10000,
                "metrics": "operational_and_protocol"
            },
            "selected_source": "current_generation"
        }"#;
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let error =
            serde_path_to_error::deserialize::<_, LegacyTraversalEvidenceRecord>(&mut deserializer)
                .expect_err("legacy traversal strategy should fail");

        assert_eq!(error.path().to_string(), "strategy");
    }
}
