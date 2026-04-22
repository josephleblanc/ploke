use std::collections::BTreeSet;
use std::convert::Infallible;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::core::{Confidence, EvidencePolicy, ForkState, Measurement};
use crate::llm::{JsonAdjudicationSpec, JsonAdjudicator, JsonChatPrompt, ProtocolLlmError};
use crate::procedure::{
    FanOut, FanOutError, Merge, MergeError, NamedProcedure, ObservedSubrequest, Procedure,
    ProcedureExt, Sequence, SequenceError, SubrequestDescriptor,
};
use crate::step::{MechanizedExecutor, MechanizedSpec, Step, StepSpec};

use super::segment::{IntentLabel, IntentSegment, SegmentStatus, SegmentationCoverage};
use super::trace::{NeighborhoodCall, ToolCallNeighborhood, ToolCallSequence, ToolKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Concern {
    RepeatedToolCluster,
    SearchThrashRisk,
    RecoveryOpportunity,
    FilePivot,
    AmbiguousScope,
    ResidualCoverageGap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalAnalysisTargetKind {
    FocalCall,
    IntentSegment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAnalysisPacket {
    pub subject_id: String,
    pub target_kind: LocalAnalysisTargetKind,
    pub target_id: String,
    pub scope_summary: String,
    pub total_calls_in_scope: usize,
    pub total_calls_in_run: usize,
    pub turn_span: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focal_call_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_status: Option<SegmentStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_label: Option<IntentLabel>,
    pub calls: Vec<NeighborhoodCall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAnalysisSignals {
    pub scope_turn_count: usize,
    pub repeated_tool_name_count: usize,
    pub distinct_tool_count: usize,
    pub search_calls_in_scope: usize,
    pub read_calls_in_scope: usize,
    pub browse_calls_in_scope: usize,
    pub edit_calls_in_scope: usize,
    pub execute_calls_in_scope: usize,
    pub failed_calls_in_scope: usize,
    pub similar_search_neighbors: usize,
    pub directory_pivots: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labeled_segments_in_source: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambiguous_segments_in_source: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncovered_calls_in_source: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_concerns: Vec<Concern>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAnalysisContext {
    pub packet: LocalAnalysisPacket,
    pub signals: LocalAnalysisSignals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsefulnessVerdict {
    KeyProgress,
    HelpfulButNonEssential,
    LowValue,
    NoValue,
    Unclear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsefulnessAssessment {
    pub verdict: UsefulnessVerdict,
    pub confidence: Confidence,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedundancyVerdict {
    Distinct,
    Overlapping,
    RedundantRepeat,
    SearchThrash,
    Unclear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedundancyAssessment {
    pub verdict: RedundancyVerdict,
    pub confidence: Confidence,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoverabilityVerdict {
    NoRecoveryNeeded,
    ClearNextStep,
    PartialNextStep,
    NoClearRecovery,
    Unclear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverabilityAssessment {
    pub verdict: RecoverabilityVerdict,
    pub confidence: Confidence,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverallVerdict {
    FocusedProgress,
    UsefulExploration,
    RecoverableDetour,
    RedundantThrash,
    Mixed,
    Unclear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAnalysisAssessment {
    pub packet: LocalAnalysisPacket,
    pub signals: LocalAnalysisSignals,
    pub usefulness: UsefulnessAssessment,
    pub redundancy: RedundancyAssessment,
    pub recoverability: RecoverabilityAssessment,
    pub overall: OverallVerdict,
    pub overall_confidence: Confidence,
    pub synthesis_rationale: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Metric;

impl Measurement for Metric {
    type Subject = ToolCallNeighborhood;
    type Value = LocalAnalysisAssessment;

    fn name(&self) -> &'static str {
        "tool_call_review"
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SegmentMetric;

impl Measurement for SegmentMetric {
    type Subject = SegmentReviewSubject;
    type Value = LocalAnalysisAssessment;

    fn name(&self) -> &'static str {
        "tool_call_segment_review"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentReviewSubject {
    pub subject_id: String,
    pub sequence: ToolCallSequence,
    pub segment: IntentSegment,
    pub coverage: SegmentationCoverage,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ContextualizeNeighborhood;

impl StepSpec for ContextualizeNeighborhood {
    type InputState = ToolCallNeighborhood;
    type OutputState = LocalAnalysisContext;

    fn step_id(&self) -> &'static str {
        "contextualize_tool_call_neighborhood"
    }

    fn step_name(&self) -> &'static str {
        "contextualize_tool_call_neighborhood"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec![
                "focal tool call".to_string(),
                "bounded neighborhood around the focal tool call".to_string(),
                "turn-local metadata".to_string(),
            ],
            forbidden: vec![
                "other run state not projected into the neighborhood".to_string(),
                "external repository context".to_string(),
            ],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

impl MechanizedSpec for ContextualizeNeighborhood {
    type Error = Infallible;

    fn execute_mechanized(
        &self,
        input: Self::InputState,
    ) -> Result<Self::OutputState, Self::Error> {
        let packet = LocalAnalysisPacket {
            subject_id: input.subject_id.clone(),
            target_kind: LocalAnalysisTargetKind::FocalCall,
            target_id: format!("call:{}", input.focal.index),
            scope_summary: format!(
                "focal call [{}] {} in bounded neighborhood",
                input.focal.index, input.focal.tool_name
            ),
            total_calls_in_scope: input.before.len() + 1 + input.after.len(),
            total_calls_in_run: input.total_calls_in_run,
            turn_span: vec![input.turn.turn],
            focal_call_index: Some(input.focal.index),
            segment_index: None,
            segment_status: None,
            segment_label: None,
            calls: input.all_calls().cloned().collect(),
        };
        let signals = derive_signals(&packet, None);

        Ok(LocalAnalysisContext { packet, signals })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ContextualizeSegment;

impl StepSpec for ContextualizeSegment {
    type InputState = SegmentReviewSubject;
    type OutputState = LocalAnalysisContext;

    fn step_id(&self) -> &'static str {
        "contextualize_intent_segment"
    }

    fn step_name(&self) -> &'static str {
        "contextualize_intent_segment"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec![
                "adjudicated segment from intent segmentation".to_string(),
                "calls inside the selected segment".to_string(),
                "mechanized segmentation coverage summary".to_string(),
            ],
            forbidden: vec![
                "calls outside the selected segment".to_string(),
                "external repository context".to_string(),
            ],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

impl MechanizedSpec for ContextualizeSegment {
    type Error = Infallible;

    fn execute_mechanized(
        &self,
        input: Self::InputState,
    ) -> Result<Self::OutputState, Self::Error> {
        let label = input.segment.label;
        let status = input.segment.status;
        let packet = LocalAnalysisPacket {
            subject_id: input.subject_id.clone(),
            target_kind: LocalAnalysisTargetKind::IntentSegment,
            target_id: format!("segment:{}", input.segment.segment_index),
            scope_summary: format!(
                "segment {} status={status:?} label={} range={}..={}",
                input.segment.segment_index,
                label
                    .map(|value| format!("{value:?}"))
                    .unwrap_or_else(|| "-".to_string()),
                input.segment.start_index,
                input.segment.end_index
            ),
            total_calls_in_scope: input.segment.calls.len(),
            total_calls_in_run: input.sequence.total_calls_in_run,
            turn_span: input.segment.turns.clone(),
            focal_call_index: None,
            segment_index: Some(input.segment.segment_index),
            segment_status: Some(status),
            segment_label: label,
            calls: input.segment.calls.clone(),
        };
        let signals = derive_signals(&packet, Some(&input.coverage));

        Ok(LocalAnalysisContext { packet, signals })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AssessLocalUsefulness;

impl StepSpec for AssessLocalUsefulness {
    type InputState = LocalAnalysisContext;
    type OutputState = UsefulnessAssessment;

    fn step_id(&self) -> &'static str {
        "assess_local_usefulness"
    }

    fn step_name(&self) -> &'static str {
        "assess_local_usefulness"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        branch_evidence_policy("local usefulness of the selected scope")
    }
}

impl JsonAdjudicationSpec for AssessLocalUsefulness {
    fn build_prompt(&self, input: &Self::InputState) -> JsonChatPrompt {
        JsonChatPrompt {
            system: "You are evaluating the local usefulness of one selected scope inside a bounded tool-analysis packet. Use only the provided calls and mechanized signals. Return exactly one JSON object with keys: verdict, confidence, rationale. Valid verdict values: key_progress, helpful_but_non_essential, low_value, no_value, unclear. Valid confidence values: low, medium, high.".to_string(),
            user: format!(
                "Task: judge the local usefulness of the selected scope.\n\n{}\n\nFocus on whether this scope materially advanced local progress within the visible procedure, not whether the overall run succeeded.",
                render_review_context(input)
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AssessRedundancy;

impl StepSpec for AssessRedundancy {
    type InputState = LocalAnalysisContext;
    type OutputState = RedundancyAssessment;

    fn step_id(&self) -> &'static str {
        "assess_redundancy"
    }

    fn step_name(&self) -> &'static str {
        "assess_redundancy"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        branch_evidence_policy("redundancy or search thrash around the selected scope")
    }
}

impl JsonAdjudicationSpec for AssessRedundancy {
    fn build_prompt(&self, input: &Self::InputState) -> JsonChatPrompt {
        JsonChatPrompt {
            system: "You are evaluating whether a selected local scope is distinct, overlapping, redundant, or part of local search thrash. Use only the provided calls and mechanized signals. Return exactly one JSON object with keys: verdict, confidence, rationale. Valid verdict values: distinct, overlapping, redundant_repeat, search_thrash, unclear. Valid confidence values: low, medium, high.".to_string(),
            user: format!(
                "Task: judge redundancy or thrash around the selected scope.\n\n{}\n\nFocus on repetition, overlap, and whether the selected scope appears to repeat work already present nearby or extend a low-yield search pattern without clear gain.",
                render_review_context(input)
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AssessRecoverability;

impl StepSpec for AssessRecoverability {
    type InputState = LocalAnalysisContext;
    type OutputState = RecoverabilityAssessment;

    fn step_id(&self) -> &'static str {
        "assess_recoverability"
    }

    fn step_name(&self) -> &'static str {
        "assess_recoverability"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        branch_evidence_policy("recoverability after or around the selected scope")
    }
}

impl JsonAdjudicationSpec for AssessRecoverability {
    fn build_prompt(&self, input: &Self::InputState) -> JsonChatPrompt {
        JsonChatPrompt {
            system: "You are evaluating whether the selected local scope had a clear recovery path or next step available from the visible packet. Use only the provided calls and mechanized signals. Return exactly one JSON object with keys: verdict, confidence, rationale. Valid verdict values: no_recovery_needed, clear_next_step, partial_next_step, no_clear_recovery, unclear. Valid confidence values: low, medium, high.".to_string(),
            user: format!(
                "Task: judge recoverability for the selected scope.\n\n{}\n\nIf the selected scope already looks appropriate, no_recovery_needed is acceptable. Otherwise judge whether the visible packet suggests a clear or partial next step that would have improved the local procedure.",
                render_review_context(input)
            ),
        }
    }
}

pub type BranchPair = FanOut<
    ObservedSubrequest<Step<AssessLocalUsefulness, JsonAdjudicator>>,
    ObservedSubrequest<Step<AssessRedundancy, JsonAdjudicator>>,
>;
pub type BranchSet =
    FanOut<BranchPair, ObservedSubrequest<Step<AssessRecoverability, JsonAdjudicator>>>;
pub type BranchJudgments = ForkState<
    LocalAnalysisContext,
    ForkState<LocalAnalysisContext, UsefulnessAssessment, RedundancyAssessment>,
    RecoverabilityAssessment,
>;

#[derive(Debug, Clone, Copy, Default)]
pub struct AssembleAssessment;

impl StepSpec for AssembleAssessment {
    type InputState = BranchJudgments;
    type OutputState = LocalAnalysisAssessment;

    fn step_id(&self) -> &'static str {
        "assemble_local_analysis_assessment"
    }

    fn step_name(&self) -> &'static str {
        "assemble_local_analysis_assessment"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec![
                "contextualized local analysis packet".to_string(),
                "branch-local usefulness judgment".to_string(),
                "branch-local redundancy judgment".to_string(),
                "branch-local recoverability judgment".to_string(),
            ],
            forbidden: vec![
                "new external evidence".to_string(),
                "branch-erasing synthesis".to_string(),
            ],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

impl MechanizedSpec for AssembleAssessment {
    type Error = Infallible;

    fn execute_mechanized(
        &self,
        input: Self::InputState,
    ) -> Result<Self::OutputState, Self::Error> {
        let context = input.source;
        let usefulness = input.left.left;
        let redundancy = input.left.right;
        let recoverability = input.right;
        let (overall, overall_confidence) =
            derive_overall(&usefulness, &redundancy, &recoverability);
        let synthesis_rationale = format!(
            "usefulness={:?} ({:?}); redundancy={:?} ({:?}); recoverability={:?} ({:?}). \
signals: repeated_tool_name_count={}, distinct_tool_count={}, similar_search_neighbors={}, directory_pivots={}, uncovered_calls_in_source={:?}. \
branch rationales: usefulness='{}' redundancy='{}' recoverability='{}'.",
            usefulness.verdict,
            usefulness.confidence,
            redundancy.verdict,
            redundancy.confidence,
            recoverability.verdict,
            recoverability.confidence,
            context.signals.repeated_tool_name_count,
            context.signals.distinct_tool_count,
            context.signals.similar_search_neighbors,
            context.signals.directory_pivots,
            context.signals.uncovered_calls_in_source,
            usefulness.rationale,
            redundancy.rationale,
            recoverability.rationale,
        );

        Ok(LocalAnalysisAssessment {
            packet: context.packet,
            signals: context.signals,
            usefulness,
            redundancy,
            recoverability,
            overall,
            overall_confidence,
            synthesis_rationale,
        })
    }
}

pub type ToolCallReviewInner = Sequence<
    Step<ContextualizeNeighborhood, MechanizedExecutor>,
    Merge<BranchSet, Step<AssembleAssessment, MechanizedExecutor>>,
>;

pub type ToolCallReviewArtifact = crate::core::ProcedureArtifact<
    crate::core::SequenceArtifact<
        crate::core::StepArtifact<
            ToolCallNeighborhood,
            LocalAnalysisContext,
            crate::step::MechanizedProvenance,
        >,
        crate::core::MergeArtifact<
            crate::core::FanOutArtifact<
                LocalAnalysisContext,
                crate::core::FanOutArtifact<
                    LocalAnalysisContext,
                    crate::core::StepArtifact<
                        LocalAnalysisContext,
                        UsefulnessAssessment,
                        crate::llm::JsonLlmProvenance,
                    >,
                    crate::core::StepArtifact<
                        LocalAnalysisContext,
                        RedundancyAssessment,
                        crate::llm::JsonLlmProvenance,
                    >,
                >,
                crate::core::StepArtifact<
                    LocalAnalysisContext,
                    RecoverabilityAssessment,
                    crate::llm::JsonLlmProvenance,
                >,
            >,
            crate::core::StepArtifact<
                BranchJudgments,
                LocalAnalysisAssessment,
                crate::step::MechanizedProvenance,
            >,
        >,
    >,
>;

pub type ToolCallReviewError = SequenceError<
    Infallible,
    MergeError<
        FanOutError<FanOutError<ProtocolLlmError, ProtocolLlmError>, ProtocolLlmError>,
        Infallible,
    >,
>;

#[derive(Debug, Clone)]
pub struct ToolCallReview {
    inner: NamedProcedure<ToolCallReviewInner>,
}

impl ToolCallReview {
    pub fn new(adjudicator: JsonAdjudicator) -> Self {
        let context = Step::new(ContextualizeNeighborhood, MechanizedExecutor);
        let usefulness = ObservedSubrequest::new(
            Step::new(AssessLocalUsefulness, adjudicator.clone()),
            SubrequestDescriptor {
                label: "usefulness",
                request_index: 1,
                request_total: 3,
            },
        );
        let redundancy = ObservedSubrequest::new(
            Step::new(AssessRedundancy, adjudicator.clone()),
            SubrequestDescriptor {
                label: "redundancy",
                request_index: 2,
                request_total: 3,
            },
        );
        let recoverability = ObservedSubrequest::new(
            Step::new(AssessRecoverability, adjudicator),
            SubrequestDescriptor {
                label: "recoverability",
                request_index: 3,
                request_total: 3,
            },
        );
        let branches = usefulness
            .fan_out_named("usefulness", "redundancy", redundancy)
            .fan_out_named("judgment_pair", "recoverability", recoverability)
            .merge(Step::new(AssembleAssessment, MechanizedExecutor));

        Self {
            inner: context.then(branches).named("tool_call_review"),
        }
    }
}

#[async_trait]
impl Procedure for ToolCallReview {
    type Subject = ToolCallNeighborhood;
    type Output = LocalAnalysisAssessment;
    type Artifact = ToolCallReviewArtifact;
    type Error = ToolCallReviewError;

    fn name(&self) -> &'static str {
        self.inner.name()
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<crate::core::ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        self.inner.run(subject).await
    }
}

pub type SegmentReviewInner = Sequence<
    Step<ContextualizeSegment, MechanizedExecutor>,
    Merge<BranchSet, Step<AssembleAssessment, MechanizedExecutor>>,
>;

pub type ToolCallSegmentReviewArtifact = crate::core::ProcedureArtifact<
    crate::core::SequenceArtifact<
        crate::core::StepArtifact<
            SegmentReviewSubject,
            LocalAnalysisContext,
            crate::step::MechanizedProvenance,
        >,
        crate::core::MergeArtifact<
            crate::core::FanOutArtifact<
                LocalAnalysisContext,
                crate::core::FanOutArtifact<
                    LocalAnalysisContext,
                    crate::core::StepArtifact<
                        LocalAnalysisContext,
                        UsefulnessAssessment,
                        crate::llm::JsonLlmProvenance,
                    >,
                    crate::core::StepArtifact<
                        LocalAnalysisContext,
                        RedundancyAssessment,
                        crate::llm::JsonLlmProvenance,
                    >,
                >,
                crate::core::StepArtifact<
                    LocalAnalysisContext,
                    RecoverabilityAssessment,
                    crate::llm::JsonLlmProvenance,
                >,
            >,
            crate::core::StepArtifact<
                BranchJudgments,
                LocalAnalysisAssessment,
                crate::step::MechanizedProvenance,
            >,
        >,
    >,
>;

pub type ToolCallSegmentReviewError = SequenceError<
    Infallible,
    MergeError<
        FanOutError<FanOutError<ProtocolLlmError, ProtocolLlmError>, ProtocolLlmError>,
        Infallible,
    >,
>;

#[derive(Debug, Clone)]
pub struct ToolCallSegmentReview {
    inner: NamedProcedure<SegmentReviewInner>,
}

impl ToolCallSegmentReview {
    pub fn new(adjudicator: JsonAdjudicator) -> Self {
        let context = Step::new(ContextualizeSegment, MechanizedExecutor);
        let usefulness = ObservedSubrequest::new(
            Step::new(AssessLocalUsefulness, adjudicator.clone()),
            SubrequestDescriptor {
                label: "usefulness",
                request_index: 1,
                request_total: 3,
            },
        );
        let redundancy = ObservedSubrequest::new(
            Step::new(AssessRedundancy, adjudicator.clone()),
            SubrequestDescriptor {
                label: "redundancy",
                request_index: 2,
                request_total: 3,
            },
        );
        let recoverability = ObservedSubrequest::new(
            Step::new(AssessRecoverability, adjudicator),
            SubrequestDescriptor {
                label: "recoverability",
                request_index: 3,
                request_total: 3,
            },
        );
        let branches = usefulness
            .fan_out_named("usefulness", "redundancy", redundancy)
            .fan_out_named("judgment_pair", "recoverability", recoverability)
            .merge(Step::new(AssembleAssessment, MechanizedExecutor));

        Self {
            inner: context.then(branches).named("tool_call_segment_review"),
        }
    }
}

#[async_trait]
impl Procedure for ToolCallSegmentReview {
    type Subject = SegmentReviewSubject;
    type Output = LocalAnalysisAssessment;
    type Artifact = ToolCallSegmentReviewArtifact;
    type Error = ToolCallSegmentReviewError;

    fn name(&self) -> &'static str {
        self.inner.name()
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<crate::core::ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        self.inner.run(subject).await
    }
}

fn branch_evidence_policy(focus: &str) -> EvidencePolicy {
    EvidencePolicy {
        allowed: vec![
            "bounded local analysis packet".to_string(),
            "mechanized local signals".to_string(),
            format!("branch focus: {focus}"),
        ],
        forbidden: vec![
            "future events outside the projected packet".to_string(),
            "external repository knowledge".to_string(),
            "counterfactuals unsupported by the packet".to_string(),
        ],
        hindsight_allowed: false,
        external_context_allowed: false,
    }
}

fn derive_signals(
    packet: &LocalAnalysisPacket,
    coverage: Option<&SegmentationCoverage>,
) -> LocalAnalysisSignals {
    let mut repeated_tool_name_count = 0usize;
    let mut search_calls_in_scope = 0usize;
    let mut read_calls_in_scope = 0usize;
    let mut browse_calls_in_scope = 0usize;
    let mut edit_calls_in_scope = 0usize;
    let mut execute_calls_in_scope = 0usize;
    let mut failed_calls_in_scope = 0usize;
    let mut directory_pivots = 0usize;
    let mut similar_search_neighbors = 0usize;
    let distinct_tool_count = packet
        .calls
        .iter()
        .map(|call| call.tool_name.clone())
        .collect::<BTreeSet<_>>()
        .len();

    let focal_terms = packet
        .calls
        .first()
        .and_then(|call| {
            if packet.target_kind == LocalAnalysisTargetKind::FocalCall
                && Some(call.index) == packet.focal_call_index
            {
                call.search_term.as_deref()
            } else {
                None
            }
        })
        .map(search_terms)
        .unwrap_or_default();
    let mut prior_path_hint: Option<&str> = None;

    for call in &packet.calls {
        if packet.target_kind == LocalAnalysisTargetKind::FocalCall
            && call.tool_name
                == packet
                    .calls
                    .iter()
                    .find(|candidate| Some(candidate.index) == packet.focal_call_index)
                    .map(|candidate| candidate.tool_name.as_str())
                    .unwrap_or_default()
        {
            repeated_tool_name_count += 1;
        }
        match call.tool_kind {
            ToolKind::Search => search_calls_in_scope += 1,
            ToolKind::Read => read_calls_in_scope += 1,
            ToolKind::Browse => browse_calls_in_scope += 1,
            ToolKind::Edit => edit_calls_in_scope += 1,
            ToolKind::Execute => execute_calls_in_scope += 1,
            ToolKind::Other => {}
        }
        if call.failed {
            failed_calls_in_scope += 1;
        }

        if let Some(path_hint) = call.path_hint.as_deref() {
            if prior_path_hint.is_some() && prior_path_hint != Some(path_hint) {
                directory_pivots += 1;
            }
            prior_path_hint = Some(path_hint);
        }
    }

    if !focal_terms.is_empty() {
        for call in &packet.calls {
            if Some(call.index) == packet.focal_call_index {
                continue;
            }
            let Some(search_term) = call.search_term.as_deref() else {
                continue;
            };
            let other_terms = search_terms(search_term);
            if !other_terms.is_empty() && shared_terms(&focal_terms, &other_terms) >= 2 {
                similar_search_neighbors += 1;
            }
        }
    } else {
        let search_terms_seen = packet
            .calls
            .iter()
            .filter_map(|call| call.search_term.as_deref())
            .map(search_terms)
            .collect::<Vec<_>>();
        for pair in search_terms_seen.windows(2) {
            if shared_terms(&pair[0], &pair[1]) >= 2 {
                similar_search_neighbors += 1;
            }
        }
    }

    let mut candidate_concerns = Vec::new();
    if repeated_tool_name_count >= 3 {
        candidate_concerns.push(Concern::RepeatedToolCluster);
    }
    if search_calls_in_scope >= 3
        && (similar_search_neighbors >= 2 || repeated_tool_name_count >= 3)
    {
        candidate_concerns.push(Concern::SearchThrashRisk);
    }
    if directory_pivots > 0 {
        candidate_concerns.push(Concern::FilePivot);
    }
    if failed_calls_in_scope > 0 || browse_calls_in_scope > 0 || read_calls_in_scope == 0 {
        candidate_concerns.push(Concern::RecoveryOpportunity);
    }
    if packet.segment_status == Some(SegmentStatus::Ambiguous) {
        candidate_concerns.push(Concern::AmbiguousScope);
    }
    if coverage
        .map(|value| value.uncovered_calls > 0)
        .unwrap_or(false)
    {
        candidate_concerns.push(Concern::ResidualCoverageGap);
    }

    LocalAnalysisSignals {
        scope_turn_count: packet.turn_span.len(),
        repeated_tool_name_count,
        distinct_tool_count,
        search_calls_in_scope,
        read_calls_in_scope,
        browse_calls_in_scope,
        edit_calls_in_scope,
        execute_calls_in_scope,
        failed_calls_in_scope,
        similar_search_neighbors,
        directory_pivots,
        labeled_segments_in_source: coverage.map(|value| value.labeled_segments),
        ambiguous_segments_in_source: coverage.map(|value| value.ambiguous_segments),
        uncovered_calls_in_source: coverage.map(|value| value.uncovered_calls),
        candidate_concerns,
    }
}

fn derive_overall(
    usefulness: &UsefulnessAssessment,
    redundancy: &RedundancyAssessment,
    recoverability: &RecoverabilityAssessment,
) -> (OverallVerdict, Confidence) {
    let overall = match (
        usefulness.verdict,
        redundancy.verdict,
        recoverability.verdict,
    ) {
        (
            UsefulnessVerdict::KeyProgress | UsefulnessVerdict::HelpfulButNonEssential,
            RedundancyVerdict::Distinct | RedundancyVerdict::Overlapping,
            RecoverabilityVerdict::NoRecoveryNeeded,
        ) => OverallVerdict::FocusedProgress,
        (
            UsefulnessVerdict::HelpfulButNonEssential | UsefulnessVerdict::LowValue,
            RedundancyVerdict::Distinct | RedundancyVerdict::Overlapping,
            RecoverabilityVerdict::ClearNextStep | RecoverabilityVerdict::PartialNextStep,
        ) => OverallVerdict::UsefulExploration,
        (
            UsefulnessVerdict::LowValue | UsefulnessVerdict::NoValue,
            _,
            RecoverabilityVerdict::ClearNextStep | RecoverabilityVerdict::PartialNextStep,
        ) => OverallVerdict::RecoverableDetour,
        (
            UsefulnessVerdict::LowValue | UsefulnessVerdict::NoValue,
            RedundancyVerdict::RedundantRepeat | RedundancyVerdict::SearchThrash,
            _,
        ) => OverallVerdict::RedundantThrash,
        (UsefulnessVerdict::Unclear, _, _)
        | (_, RedundancyVerdict::Unclear, _)
        | (_, _, RecoverabilityVerdict::Unclear) => OverallVerdict::Unclear,
        _ => OverallVerdict::Mixed,
    };

    (
        overall,
        min_confidence(
            usefulness.confidence,
            min_confidence(redundancy.confidence, recoverability.confidence),
        ),
    )
}

fn min_confidence(left: Confidence, right: Confidence) -> Confidence {
    use Confidence::{High, Low, Medium};

    match (left, right) {
        (Low, _) | (_, Low) => Low,
        (Medium, _) | (_, Medium) => Medium,
        (High, High) => High,
    }
}

fn render_review_context(input: &LocalAnalysisContext) -> String {
    format!(
        "subject_id: {}\ntarget_kind: {:?}\ntarget_id: {}\nscope_summary: {}\nturn_span: {}\ntotal_calls_in_scope: {}\ntotal_calls_in_run: {}\nsegment_status: {}\nsegment_label: {}\n\ncalls:\n{}\n\nsignals:\n{}",
        input.packet.subject_id,
        input.packet.target_kind,
        input.packet.target_id,
        input.packet.scope_summary,
        if input.packet.turn_span.is_empty() {
            "-".to_string()
        } else {
            input
                .packet
                .turn_span
                .iter()
                .map(|turn| turn.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        },
        input.packet.total_calls_in_scope,
        input.packet.total_calls_in_run,
        input
            .packet
            .segment_status
            .map(|value| format!("{value:?}"))
            .unwrap_or_else(|| "-".to_string()),
        input
            .packet
            .segment_label
            .map(|value| format!("{value:?}"))
            .unwrap_or_else(|| "-".to_string()),
        render_call_block(&input.packet),
        render_signals(&input.signals),
    )
}

fn render_call_block(packet: &LocalAnalysisPacket) -> String {
    if packet.calls.is_empty() {
        return "  (none)".to_string();
    }

    packet
        .calls
        .iter()
        .map(|call| {
            let marker = if Some(call.index) == packet.focal_call_index {
                "focal"
            } else {
                "scope"
            };
            format!(
                "  ({marker}) [{}] tool={} kind={:?} failed={} latency_ms={} search_term={} path_hint={} summary={} args={} result={}",
                call.index,
                call.tool_name,
                call.tool_kind,
                call.failed,
                call.latency_ms,
                call.search_term.as_deref().unwrap_or("-"),
                call.path_hint.as_deref().unwrap_or("-"),
                call.summary,
                call.args_preview,
                call.result_preview,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_signals(signals: &LocalAnalysisSignals) -> String {
    format!(
        "  scope_turn_count={}\n  repeated_tool_name_count={}\n  distinct_tool_count={}\n  search_calls_in_scope={}\n  read_calls_in_scope={}\n  browse_calls_in_scope={}\n  edit_calls_in_scope={}\n  execute_calls_in_scope={}\n  failed_calls_in_scope={}\n  similar_search_neighbors={}\n  directory_pivots={}\n  labeled_segments_in_source={:?}\n  ambiguous_segments_in_source={:?}\n  uncovered_calls_in_source={:?}\n  candidate_concerns={:?}",
        signals.scope_turn_count,
        signals.repeated_tool_name_count,
        signals.distinct_tool_count,
        signals.search_calls_in_scope,
        signals.read_calls_in_scope,
        signals.browse_calls_in_scope,
        signals.edit_calls_in_scope,
        signals.execute_calls_in_scope,
        signals.failed_calls_in_scope,
        signals.similar_search_neighbors,
        signals.directory_pivots,
        signals.labeled_segments_in_source,
        signals.ambiguous_segments_in_source,
        signals.uncovered_calls_in_source,
        signals.candidate_concerns,
    )
}

fn search_terms(input: &str) -> BTreeSet<String> {
    input
        .split(|ch: char| !ch.is_alphanumeric() && ch != '!' && ch != '_')
        .filter(|term| !term.is_empty())
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

fn shared_terms(left: &BTreeSet<String>, right: &BTreeSet<String>) -> usize {
    left.intersection(right).count()
}
