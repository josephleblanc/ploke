use std::collections::BTreeSet;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core::{Confidence, EvidencePolicy, Measurement};
use crate::llm::{JsonAdjudicationSpec, JsonAdjudicator, JsonChatPrompt, ProtocolLlmError};
use crate::procedure::{
    FanOut, FanOutError, Merge, MergeError, NamedProcedure, ObservedSubrequest, Procedure,
    ProcedureExt, Sequence, SequenceError, SubrequestDescriptor,
};
use crate::step::{MechanizedExecutor, MechanizedSpec, Step, StepSpec};

use super::trace::{ToolCallSequence, ToolKind, TurnContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentLabel {
    LocateTarget,
    InspectCandidate,
    RefineSearch,
    ValidateHypothesis,
    EditAttempt,
    Recovery,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentStatus {
    Labeled,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceSignals {
    pub total_turns: usize,
    pub total_calls: usize,
    pub search_calls: usize,
    pub read_calls: usize,
    pub browse_calls: usize,
    pub edit_calls: usize,
    pub execute_calls: usize,
    pub failed_calls: usize,
    pub repeated_search_runs: usize,
    pub directory_pivots: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search_terms_seen: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceReviewContext {
    pub sequence: ToolCallSequence,
    pub signals: SequenceSignals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentSegmentProposal {
    pub start_index: usize,
    pub end_index: usize,
    pub status: SegmentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<IntentLabel>,
    pub confidence: Confidence,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentationJudgment {
    pub segments: Vec<IntentSegmentProposal>,
    pub overall_rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentSegment {
    pub segment_index: usize,
    pub start_index: usize,
    pub end_index: usize,
    pub status: SegmentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<IntentLabel>,
    pub confidence: Confidence,
    pub rationale: String,
    pub turns: Vec<u32>,
    pub calls: Vec<super::trace::NeighborhoodCall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UncoveredCallSpan {
    pub start_index: usize,
    pub end_index: usize,
    pub call_indices: Vec<usize>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentationCoverage {
    pub total_calls: usize,
    pub labeled_segments: usize,
    pub ambiguous_segments: usize,
    pub labeled_calls: usize,
    pub ambiguous_calls: usize,
    pub uncovered_calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentedToolCallSequence {
    pub sequence: ToolCallSequence,
    pub signals: SequenceSignals,
    pub segments: Vec<IntentSegment>,
    pub coverage: SegmentationCoverage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncovered_spans: Vec<UncoveredCallSpan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncovered_call_indices: Vec<usize>,
    pub overall_rationale: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Metric;

impl Measurement for Metric {
    type Subject = ToolCallSequence;
    type Value = SegmentedToolCallSequence;

    fn name(&self) -> &'static str {
        "tool_call_intent_segmentation"
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ContextualizeSequence;

impl StepSpec for ContextualizeSequence {
    type InputState = ToolCallSequence;
    type OutputState = SequenceReviewContext;

    fn step_id(&self) -> &'static str {
        "contextualize_tool_call_sequence"
    }

    fn step_name(&self) -> &'static str {
        "contextualize_tool_call_sequence"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec![
                "ordered tool call sequence".to_string(),
                "turn-local summaries".to_string(),
                "tool arguments and result previews".to_string(),
            ],
            forbidden: vec![
                "repository context not projected into the sequence packet".to_string(),
                "external benchmark knowledge".to_string(),
            ],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

impl MechanizedSpec for ContextualizeSequence {
    type Error = std::convert::Infallible;

    fn execute_mechanized(
        &self,
        input: Self::InputState,
    ) -> Result<Self::OutputState, Self::Error> {
        Ok(SequenceReviewContext {
            signals: derive_sequence_signals(&input),
            sequence: input,
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SegmentByIntent;

impl StepSpec for SegmentByIntent {
    type InputState = SequenceReviewContext;
    type OutputState = SegmentationJudgment;

    fn step_id(&self) -> &'static str {
        "segment_tool_call_sequence_by_intent"
    }

    fn step_name(&self) -> &'static str {
        "segment_tool_call_sequence_by_intent"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec![
                "ordered tool call sequence".to_string(),
                "turn-local summaries".to_string(),
                "mechanized sequence signals".to_string(),
            ],
            forbidden: vec![
                "external repository context".to_string(),
                "future outcomes not represented in the packet".to_string(),
                "counterfactual intent not grounded in the observed call sequence".to_string(),
            ],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

impl JsonAdjudicationSpec for SegmentByIntent {
    fn build_prompt(&self, input: &Self::InputState) -> JsonChatPrompt {
        JsonChatPrompt {
            system: "You are segmenting an ordered tool-call sequence into contiguous intent episodes. Use only the provided sequence and mechanized signals. Return exactly one JSON object with keys: segments, overall_rationale. Each item in segments must be an object with keys: start_index, end_index, status, label, confidence, rationale. Valid status values: labeled, ambiguous. Valid label values: locate_target, inspect_candidate, refine_search, validate_hypothesis, edit_attempt, recovery, other. Valid confidence values: low, medium, high. If status is labeled, label is required. If status is ambiguous, label must be null. Use label=other only for a coherent segment whose intent does not fit the listed taxonomy; do not use other as a substitute for uncertainty. Segments must use inclusive indices, be non-overlapping, and be ordered. Prefer a small number of meaningful contiguous segments over one label per call. It is acceptable to leave some calls uncovered by omitting them from segments when even an ambiguous segment would overstate coherence.".to_string(),
            user: format!(
                "Task: segment this tool-call sequence by intent.\n\n{}\n\nGroup neighboring calls when they appear to pursue the same immediate objective, such as locating a target, inspecting a candidate file, refining a search, validating a hypothesis, attempting an edit, or recovering from a detour. Use status=ambiguous when the calls appear to form one contiguous episode but the intent label is not well supported. Leave calls uncovered only when the visible evidence is too weak to defend even an ambiguous contiguous episode.",
                render_sequence_context(input)
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PreserveSequenceContext;

impl StepSpec for PreserveSequenceContext {
    type InputState = SequenceReviewContext;
    type OutputState = SequenceReviewContext;

    fn step_id(&self) -> &'static str {
        "preserve_sequence_context"
    }

    fn step_name(&self) -> &'static str {
        "preserve_sequence_context"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec!["contextualized tool call sequence".to_string()],
            forbidden: vec!["new evidence".to_string()],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

impl MechanizedSpec for PreserveSequenceContext {
    type Error = std::convert::Infallible;

    fn execute_mechanized(
        &self,
        input: Self::InputState,
    ) -> Result<Self::OutputState, Self::Error> {
        Ok(input)
    }
}

#[derive(Debug, Clone)]
pub struct NormalizeSegments;

#[derive(Debug, Error)]
pub enum NormalizeSegmentsError {
    #[error("segmentation cannot run on an empty tool-call sequence")]
    EmptySequence,
    #[error(
        "segment {segment_index} has invalid range {start_index}..={end_index} for {call_count} calls"
    )]
    InvalidRange {
        segment_index: usize,
        start_index: usize,
        end_index: usize,
        call_count: usize,
    },
    #[error("segment {segment_index} overlaps previously covered index {overlapping_index}")]
    Overlap {
        segment_index: usize,
        overlapping_index: usize,
    },
    #[error("segment {segment_index} is labeled but missing a label value")]
    MissingLabel { segment_index: usize },
    #[error("segment {segment_index} is ambiguous but still carries label {label:?}")]
    AmbiguousWithLabel {
        segment_index: usize,
        label: IntentLabel,
    },
}

impl StepSpec for NormalizeSegments {
    type InputState =
        crate::core::ForkState<SequenceReviewContext, SequenceReviewContext, SegmentationJudgment>;
    type OutputState = SegmentedToolCallSequence;

    fn step_id(&self) -> &'static str {
        "normalize_tool_call_segments"
    }

    fn step_name(&self) -> &'static str {
        "normalize_tool_call_segments"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec![
                "contextualized tool call sequence".to_string(),
                "adjudicated segment proposals".to_string(),
            ],
            forbidden: vec![
                "new external evidence".to_string(),
                "discarding segment rationale or confidence".to_string(),
            ],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

impl MechanizedSpec for NormalizeSegments {
    type Error = NormalizeSegmentsError;

    fn execute_mechanized(
        &self,
        input: Self::InputState,
    ) -> Result<Self::OutputState, Self::Error> {
        let context = input.left;
        let judgment = input.right;
        if context.sequence.calls.is_empty() {
            return Err(NormalizeSegmentsError::EmptySequence);
        }

        let mut proposals = judgment.segments;
        proposals.sort_by_key(|proposal| (proposal.start_index, proposal.end_index));

        let mut covered = BTreeSet::new();
        let mut segments = Vec::with_capacity(proposals.len());
        let call_count = context.sequence.calls.len();

        for (segment_index, proposal) in proposals.into_iter().enumerate() {
            if proposal.start_index > proposal.end_index || proposal.end_index >= call_count {
                return Err(NormalizeSegmentsError::InvalidRange {
                    segment_index,
                    start_index: proposal.start_index,
                    end_index: proposal.end_index,
                    call_count,
                });
            }

            for idx in proposal.start_index..=proposal.end_index {
                if covered.contains(&idx) {
                    return Err(NormalizeSegmentsError::Overlap {
                        segment_index,
                        overlapping_index: idx,
                    });
                }
            }

            match (proposal.status, proposal.label) {
                (SegmentStatus::Labeled, None) => {
                    return Err(NormalizeSegmentsError::MissingLabel { segment_index });
                }
                (SegmentStatus::Ambiguous, Some(label)) => {
                    return Err(NormalizeSegmentsError::AmbiguousWithLabel {
                        segment_index,
                        label,
                    });
                }
                _ => {}
            }

            let calls = context.sequence.calls[proposal.start_index..=proposal.end_index].to_vec();
            let turns = calls
                .iter()
                .map(|call| call.turn)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();

            for idx in proposal.start_index..=proposal.end_index {
                covered.insert(idx);
            }

            segments.push(IntentSegment {
                segment_index,
                start_index: proposal.start_index,
                end_index: proposal.end_index,
                status: proposal.status,
                label: proposal.label,
                confidence: proposal.confidence,
                rationale: proposal.rationale,
                turns,
                calls,
            });
        }

        let uncovered_call_indices = (0..call_count)
            .filter(|idx| !covered.contains(idx))
            .collect::<Vec<_>>();
        let uncovered_spans = build_uncovered_spans(&uncovered_call_indices);
        let coverage = derive_coverage(call_count, &segments, uncovered_call_indices.len());

        Ok(SegmentedToolCallSequence {
            sequence: context.sequence,
            signals: context.signals,
            segments,
            coverage,
            uncovered_spans,
            uncovered_call_indices,
            overall_rationale: judgment.overall_rationale,
        })
    }
}

pub type ContextBranches = FanOut<
    Step<PreserveSequenceContext, MechanizedExecutor>,
    ObservedSubrequest<Step<SegmentByIntent, JsonAdjudicator>>,
>;

pub type IntentSegmentationInner = Sequence<
    Step<ContextualizeSequence, MechanizedExecutor>,
    Merge<ContextBranches, Step<NormalizeSegments, MechanizedExecutor>>,
>;

pub type IntentSegmentationArtifact = crate::core::ProcedureArtifact<
    crate::core::SequenceArtifact<
        crate::core::StepArtifact<
            ToolCallSequence,
            SequenceReviewContext,
            crate::step::MechanizedProvenance,
        >,
        crate::core::MergeArtifact<
            crate::core::FanOutArtifact<
                SequenceReviewContext,
                crate::core::StepArtifact<
                    SequenceReviewContext,
                    SequenceReviewContext,
                    crate::step::MechanizedProvenance,
                >,
                crate::core::StepArtifact<
                    SequenceReviewContext,
                    SegmentationJudgment,
                    crate::llm::JsonLlmProvenance,
                >,
            >,
            crate::core::StepArtifact<
                crate::core::ForkState<
                    SequenceReviewContext,
                    SequenceReviewContext,
                    SegmentationJudgment,
                >,
                SegmentedToolCallSequence,
                crate::step::MechanizedProvenance,
            >,
        >,
    >,
>;

pub type IntentSegmentationError = SequenceError<
    std::convert::Infallible,
    MergeError<FanOutError<std::convert::Infallible, ProtocolLlmError>, NormalizeSegmentsError>,
>;

#[derive(Debug, Clone)]
pub struct ToolCallIntentSegmentation {
    inner: NamedProcedure<IntentSegmentationInner>,
}

impl ToolCallIntentSegmentation {
    pub fn new(adjudicator: JsonAdjudicator) -> Self {
        let context = Step::new(ContextualizeSequence, MechanizedExecutor);
        let preserve = Step::new(PreserveSequenceContext, MechanizedExecutor);
        let segment = ObservedSubrequest::new(
            Step::new(SegmentByIntent, adjudicator),
            SubrequestDescriptor {
                label: "intent_segmentation",
                request_index: 1,
                request_total: 1,
            },
        );
        let normalize = Step::new(NormalizeSegments, MechanizedExecutor);

        Self {
            inner: context
                .then(
                    preserve
                        .fan_out_named("context", "intent_segmentation", segment)
                        .merge(normalize),
                )
                .named("tool_call_intent_segmentation"),
        }
    }
}

#[async_trait]
impl Procedure for ToolCallIntentSegmentation {
    type Subject = ToolCallSequence;
    type Output = SegmentedToolCallSequence;
    type Artifact = IntentSegmentationArtifact;
    type Error = IntentSegmentationError;

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

fn derive_sequence_signals(sequence: &ToolCallSequence) -> SequenceSignals {
    let mut search_calls = 0usize;
    let mut read_calls = 0usize;
    let mut browse_calls = 0usize;
    let mut edit_calls = 0usize;
    let mut execute_calls = 0usize;
    let mut failed_calls = 0usize;
    let mut repeated_search_runs = 0usize;
    let mut directory_pivots = 0usize;
    let mut search_terms_seen = Vec::new();
    let mut prior_search_term: Option<&str> = None;
    let mut prior_path_hint: Option<&str> = None;

    for call in &sequence.calls {
        match call.tool_kind {
            ToolKind::Search => {
                search_calls += 1;
                if let Some(search_term) = call.search_term.as_deref() {
                    if prior_search_term == Some(search_term) {
                        repeated_search_runs += 1;
                    }
                    if !search_terms_seen.iter().any(|seen| seen == search_term) {
                        search_terms_seen.push(search_term.to_string());
                    }
                    prior_search_term = Some(search_term);
                } else {
                    prior_search_term = None;
                }
            }
            ToolKind::Read => {
                read_calls += 1;
                prior_search_term = None;
            }
            ToolKind::Browse => {
                browse_calls += 1;
                prior_search_term = None;
            }
            ToolKind::Edit => {
                edit_calls += 1;
                prior_search_term = None;
            }
            ToolKind::Execute => {
                execute_calls += 1;
                prior_search_term = None;
            }
            ToolKind::Other => {
                prior_search_term = None;
            }
        }

        if call.failed {
            failed_calls += 1;
        }

        if let Some(path_hint) = call.path_hint.as_deref() {
            if prior_path_hint.is_some() && prior_path_hint != Some(path_hint) {
                directory_pivots += 1;
            }
            prior_path_hint = Some(path_hint);
        }
    }

    SequenceSignals {
        total_turns: sequence.total_turns,
        total_calls: sequence.total_calls_in_run,
        search_calls,
        read_calls,
        browse_calls,
        edit_calls,
        execute_calls,
        failed_calls,
        repeated_search_runs,
        directory_pivots,
        search_terms_seen,
    }
}

fn render_sequence_context(context: &SequenceReviewContext) -> String {
    let mut rendered = String::new();
    rendered.push_str("Sequence summary\n");
    rendered.push_str("----------------\n");
    rendered.push_str(&format!(
        "subject_id: {}\nturns: {}\ncalls: {}\nsearch_calls: {}\nread_calls: {}\nbrowse_calls: {}\nedit_calls: {}\nexecute_calls: {}\nfailed_calls: {}\nrepeated_search_runs: {}\ndirectory_pivots: {}\nsearch_terms_seen: {}\n\n",
        context.sequence.subject_id,
        context.signals.total_turns,
        context.signals.total_calls,
        context.signals.search_calls,
        context.signals.read_calls,
        context.signals.browse_calls,
        context.signals.edit_calls,
        context.signals.execute_calls,
        context.signals.failed_calls,
        context.signals.repeated_search_runs,
        context.signals.directory_pivots,
        if context.signals.search_terms_seen.is_empty() {
            "(none)".to_string()
        } else {
            context.signals.search_terms_seen.join(", ")
        }
    ));

    rendered.push_str("Turn summaries\n");
    rendered.push_str("--------------\n");
    for turn in &context.sequence.turns {
        rendered.push_str(&render_turn_context(turn));
        rendered.push('\n');
    }

    rendered.push('\n');
    rendered.push_str("Ordered tool calls\n");
    rendered.push_str("------------------\n");
    for call in &context.sequence.calls {
        rendered.push_str(&format!(
            "[{}] turn={} tool={} kind={:?} failed={} summary={}\n",
            call.index, call.turn, call.tool_name, call.tool_kind, call.failed, call.summary
        ));
        if let Some(search_term) = call.search_term.as_deref() {
            rendered.push_str(&format!("  search_term: {}\n", search_term));
        }
        if let Some(path_hint) = call.path_hint.as_deref() {
            rendered.push_str(&format!("  path_hint: {}\n", path_hint));
        }
        rendered.push_str(&format!("  args: {}\n", call.args_preview));
        rendered.push_str(&format!("  result: {}\n", call.result_preview));
    }

    rendered
}

fn build_uncovered_spans(indices: &[usize]) -> Vec<UncoveredCallSpan> {
    let mut spans = Vec::new();
    let mut start = None;
    let mut previous = None;

    for &idx in indices {
        match (start, previous) {
            (None, _) => {
                start = Some(idx);
                previous = Some(idx);
            }
            (Some(span_start), Some(prev)) if idx == prev + 1 => {
                start = Some(span_start);
                previous = Some(idx);
            }
            (Some(span_start), Some(prev)) => {
                spans.push(UncoveredCallSpan {
                    start_index: span_start,
                    end_index: prev,
                    call_indices: (span_start..=prev).collect(),
                    rationale: "no adjudicated segment covered this contiguous span".to_string(),
                });
                start = Some(idx);
                previous = Some(idx);
            }
            _ => {}
        }
    }

    if let (Some(span_start), Some(prev)) = (start, previous) {
        spans.push(UncoveredCallSpan {
            start_index: span_start,
            end_index: prev,
            call_indices: (span_start..=prev).collect(),
            rationale: "no adjudicated segment covered this contiguous span".to_string(),
        });
    }

    spans
}

fn derive_coverage(
    total_calls: usize,
    segments: &[IntentSegment],
    uncovered_calls: usize,
) -> SegmentationCoverage {
    let labeled_segments = segments
        .iter()
        .filter(|segment| segment.status == SegmentStatus::Labeled)
        .count();
    let ambiguous_segments = segments.len() - labeled_segments;
    let labeled_calls = segments
        .iter()
        .filter(|segment| segment.status == SegmentStatus::Labeled)
        .map(|segment| segment.end_index - segment.start_index + 1)
        .sum();
    let ambiguous_calls = segments
        .iter()
        .filter(|segment| segment.status == SegmentStatus::Ambiguous)
        .map(|segment| segment.end_index - segment.start_index + 1)
        .sum();

    SegmentationCoverage {
        total_calls,
        labeled_segments,
        ambiguous_segments,
        labeled_calls,
        ambiguous_calls,
        uncovered_calls,
    }
}

fn render_turn_context(turn: &TurnContext) -> String {
    format!(
        "turn={} tool_count={} failed_tool_count={} patch_proposed={} patch_applied={}",
        turn.turn, turn.tool_count, turn.failed_tool_count, turn.patch_proposed, turn.patch_applied
    )
}
