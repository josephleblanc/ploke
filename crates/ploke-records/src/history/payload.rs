use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{ActorRefRecord, EvidenceRefRecord, ProcedureRefRecord, SubjectRefRecord};
use crate::branch::ResolvedTreatmentBranch;
use crate::evaluation::{EvalSet, Evaluator, RunMetrics};
use crate::ids::{ArtifactId, BlockHash, BlockId, HistoryHash, LineageId, PatchId, RecordedAt};
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
    pub touches: Vec<SurfaceTouchRecord>,
    pub touches_digest: HistoryHash,
    pub delta_id: String,
    pub delta_digest: HistoryHash,
    pub check_status: SurfaceCheckStatusRecord,
    pub apply_status: SurfaceApplyStatusRecord,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversalEvidenceRecord {
    pub seed: u64,
    #[serde(default)]
    pub strategy: String,
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
