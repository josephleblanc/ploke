//! Typed passive payloads for known protocol artifact procedures.

use ploke_protocol::{
    FanOutArtifact, ForkState, LocalAnalysisAssessment, LocalAnalysisContext, ProcedureArtifact,
    SegmentReviewSubject, SegmentationJudgment, SegmentedToolCallSequence, SequenceArtifact,
    SequenceReviewContext, StepArtifact, ToolCallNeighborhood, ToolCallSequence,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::provenance::{JsonLlmProvenanceMirror, MechanizedProvenanceMirror};

#[derive(Debug, Serialize)]
pub struct ArtifactWriteRecord<'a, Input, Output, ProcedureArtifact>
where
    Input: Serialize,
    Output: Serialize,
    ProcedureArtifact: Serialize,
{
    pub schema_version: &'a str,
    pub procedure_name: &'a str,
    pub subject_id: &'a str,
    pub run_id: &'a str,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<&'a str>,
    pub input: &'a Input,
    pub output: &'a Output,
    pub artifact: &'a ProcedureArtifact,
}

pub type IntentSegmentationArtifactMirror = ProcedureArtifact<
    SequenceArtifact<
        StepArtifact<ToolCallSequence, SequenceReviewContext, MechanizedProvenanceMirror>,
        ploke_protocol::MergeArtifact<
            FanOutArtifact<
                SequenceReviewContext,
                StepArtifact<
                    SequenceReviewContext,
                    SequenceReviewContext,
                    MechanizedProvenanceMirror,
                >,
                StepArtifact<SequenceReviewContext, SegmentationJudgment, JsonLlmProvenanceMirror>,
            >,
            StepArtifact<
                ForkState<SequenceReviewContext, SequenceReviewContext, SegmentationJudgment>,
                SegmentedToolCallSequence,
                MechanizedProvenanceMirror,
            >,
        >,
    >,
>;

pub type ToolCallReviewArtifactMirror = ProcedureArtifact<
    SequenceArtifact<
        StepArtifact<ToolCallNeighborhood, LocalAnalysisContext, MechanizedProvenanceMirror>,
        ploke_protocol::MergeArtifact<
            FanOutArtifact<
                LocalAnalysisContext,
                FanOutArtifact<
                    LocalAnalysisContext,
                    StepArtifact<
                        LocalAnalysisContext,
                        ploke_protocol::UsefulnessAssessment,
                        JsonLlmProvenanceMirror,
                    >,
                    StepArtifact<
                        LocalAnalysisContext,
                        ploke_protocol::RedundancyAssessment,
                        JsonLlmProvenanceMirror,
                    >,
                >,
                StepArtifact<
                    LocalAnalysisContext,
                    ploke_protocol::RecoverabilityAssessment,
                    JsonLlmProvenanceMirror,
                >,
            >,
            StepArtifact<
                ForkState<
                    LocalAnalysisContext,
                    ForkState<
                        LocalAnalysisContext,
                        ploke_protocol::UsefulnessAssessment,
                        ploke_protocol::RedundancyAssessment,
                    >,
                    ploke_protocol::RecoverabilityAssessment,
                >,
                LocalAnalysisAssessment,
                MechanizedProvenanceMirror,
            >,
        >,
    >,
>;

pub type ToolCallSegmentReviewArtifactMirror = ProcedureArtifact<
    SequenceArtifact<
        StepArtifact<SegmentReviewSubject, LocalAnalysisContext, MechanizedProvenanceMirror>,
        ploke_protocol::MergeArtifact<
            FanOutArtifact<
                LocalAnalysisContext,
                FanOutArtifact<
                    LocalAnalysisContext,
                    StepArtifact<
                        LocalAnalysisContext,
                        ploke_protocol::UsefulnessAssessment,
                        JsonLlmProvenanceMirror,
                    >,
                    StepArtifact<
                        LocalAnalysisContext,
                        ploke_protocol::RedundancyAssessment,
                        JsonLlmProvenanceMirror,
                    >,
                >,
                StepArtifact<
                    LocalAnalysisContext,
                    ploke_protocol::RecoverabilityAssessment,
                    JsonLlmProvenanceMirror,
                >,
            >,
            StepArtifact<
                ForkState<
                    LocalAnalysisContext,
                    ForkState<
                        LocalAnalysisContext,
                        ploke_protocol::UsefulnessAssessment,
                        ploke_protocol::RedundancyAssessment,
                    >,
                    ploke_protocol::RecoverabilityAssessment,
                >,
                LocalAnalysisAssessment,
                MechanizedProvenanceMirror,
            >,
        >,
    >,
>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(
    serialize = "PrimaryIssue: Serialize",
    deserialize = "PrimaryIssue: Deserialize<'de>"
))]
pub struct InterventionIssueDetectionArtifact<PrimaryIssue> {
    pub case_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_issue: Option<PrimaryIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueDetectionArtifactInputMirror {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub subject_id: String,
    #[serde(default)]
    pub total_calls_in_run: usize,
    #[serde(default)]
    pub anchor_segment_count: usize,
    #[serde(default)]
    pub protocol_reviewed_call_count: usize,
    #[serde(default)]
    pub protocol_reviewed_segment_count: usize,
    #[serde(default)]
    pub protocol_artifact_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueDetectionOutputMirror {
    pub cases: Vec<InterventionIssueCaseMirror>,
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct InterventionApplyArtifact<'a, Output: Serialize + ?Sized>(pub &'a Output);

pub type InterventionSynthesisArtifactMirror = ProcedureArtifact<
    SequenceArtifact<
        StepArtifact<
            InterventionSynthesisInputMirror,
            InterventionSynthesisContextMirror,
            MechanizedProvenanceMirror,
        >,
        ploke_protocol::MergeArtifact<
            FanOutArtifact<
                InterventionSynthesisContextMirror,
                FanOutArtifact<
                    InterventionSynthesisContextMirror,
                    StepArtifact<
                        InterventionSynthesisContextMirror,
                        SynthesizedInterventionDraftMirror,
                        JsonLlmProvenanceMirror,
                    >,
                    StepArtifact<
                        InterventionSynthesisContextMirror,
                        SynthesizedInterventionDraftMirror,
                        JsonLlmProvenanceMirror,
                    >,
                >,
                StepArtifact<
                    InterventionSynthesisContextMirror,
                    SynthesizedInterventionDraftMirror,
                    JsonLlmProvenanceMirror,
                >,
            >,
            StepArtifact<
                ForkState<
                    InterventionSynthesisContextMirror,
                    ForkState<
                        InterventionSynthesisContextMirror,
                        SynthesizedInterventionDraftMirror,
                        SynthesizedInterventionDraftMirror,
                    >,
                    SynthesizedInterventionDraftMirror,
                >,
                InterventionSynthesisOutputMirror,
                MechanizedProvenanceMirror,
            >,
        >,
    >,
>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSynthesisInputMirror {
    pub issue: InterventionIssueCaseMirror,
    pub source_state_id: String,
    pub source_content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_target: Option<OperationTargetMirror>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSynthesisContextMirror {
    pub issue: InterventionIssueCaseMirror,
    pub source_state_id: String,
    pub source_content: String,
    pub target_relpath: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_target: Option<OperationTargetMirror>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SynthesizedInterventionDraftMirror {
    pub proposed_content: String,
    pub intended_effect: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSynthesisOutputMirror {
    pub candidate_set: InterventionCandidateSetMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionCandidateSetMirror {
    pub source_state_id: String,
    pub target_relpath: PathBuf,
    pub source_content: String,
    pub candidates: Vec<InterventionCandidateMirror>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_target: Option<OperationTargetMirror>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionCandidateMirror {
    pub candidate_id: String,
    pub branch_label: String,
    pub proposed_content: String,
    pub spec: InterventionSpecMirror,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum InterventionSpecMirror {
    ToolGuidanceMutation {
        spec_id: String,
        evidence_basis: String,
        intended_effect: String,
        tool: String,
        edit: ArtifactEditMirror,
        validation_policy: ValidationPolicyMirror,
    },
    PolicyConfigMutation {
        spec_id: String,
        evidence_basis: String,
        intended_effect: String,
        relpath: PathBuf,
        edit: ArtifactEditMirror,
        validation_policy: ValidationPolicyMirror,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ArtifactEditMirror {
    ReplaceWholeText {
        new_text: String,
    },
    AppendText {
        text: String,
    },
    ReplaceSection {
        start_marker: String,
        end_marker: String,
        replacement: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationPolicyMirror {
    pub allowed_relpaths: Vec<PathBuf>,
    pub require_target_exists: bool,
    pub require_nonempty_result: bool,
    pub require_utf8: bool,
    pub require_content_change: bool,
    pub require_markers_after_apply: Vec<String>,
    pub require_cargo_check: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionIssueCaseMirror {
    pub selection_basis: IssueSelectionBasisMirror,
    pub target_tool: String,
    pub evidence: IssueEvidenceMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueSelectionBasisMirror {
    ProtocolReviewedIssueCalls,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueEvidenceMirror {
    pub reviewed_call_count: usize,
    pub reviewed_issue_call_count: usize,
    pub protocol: IssueProtocolEvidenceMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueProtocolEvidenceMirror {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewed_call_indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewed_segment_indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_concerns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nearby_segment_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum OperationTargetMirror {
    Artifact {
        artifact_id: String,
    },
    PatchSet {
        base_artifact_id: String,
        patch_ids: Vec<String>,
    },
    ArtifactSet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_artifact_id: Option<String>,
        artifact_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionApplyInputMirror {
    pub source_state_id: String,
    pub candidate: InterventionCandidateMirror,
    pub target_relpath: PathBuf,
    pub expected_source_content: String,
    pub repo_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionApplyOutputMirror {
    pub treatment_state: TreatmentStateRefMirror,
    pub candidate_id: String,
    pub target_relpath: PathBuf,
    pub absolute_path: PathBuf,
    pub changed: bool,
    pub source_content_hash: String,
    pub applied_content_hash: String,
    pub validation: ValidationResultMirror,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_artifact_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreatmentStateRefMirror {
    pub source_state_id: String,
    pub apply_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationResultMirror {
    pub ok: bool,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentSegmentationPayload {
    pub input: ToolCallSequence,
    pub output: SegmentedToolCallSequence,
    pub artifact: IntentSegmentationArtifactMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallReviewPayload {
    pub input: ToolCallNeighborhood,
    pub output: LocalAnalysisAssessment,
    pub artifact: ToolCallReviewArtifactMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallSegmentReviewPayload {
    pub input: SegmentReviewSubject,
    pub output: LocalAnalysisAssessment,
    pub artifact: ToolCallSegmentReviewArtifactMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionIssueDetectionPayload {
    pub input: IssueDetectionArtifactInputMirror,
    pub output: IssueDetectionOutputMirror,
    pub artifact: InterventionIssueDetectionArtifact<InterventionIssueCaseMirror>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSynthesisPayload {
    pub input: InterventionSynthesisInputMirror,
    pub output: InterventionSynthesisOutputMirror,
    pub artifact: InterventionSynthesisArtifactMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionApplyPayload {
    pub input: InterventionApplyInputMirror,
    pub output: InterventionApplyOutputMirror,
    pub artifact: InterventionApplyOutputMirror,
}
