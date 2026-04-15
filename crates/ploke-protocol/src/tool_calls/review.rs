use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core::{Confidence, EvidencePolicy, Measurement};
use crate::llm::{JsonAdjudicationSpec, JsonAdjudicator, JsonChatPrompt, ProtocolLlmError};
use crate::procedure::{NamedProcedure, Procedure, ProcedureExt, Sequence, SequenceError};
use crate::step::{MechanizedExecutor, MechanizedSpec, Step, StepSpec};

use super::trace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Appropriate,
    WrongTool,
    BadArguments,
    Redundant,
    RecoverableFailure,
    Unclear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub index: usize,
    pub turn: u32,
    pub tool_name: String,
    pub failed: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judgment {
    pub index: usize,
    pub verdict: Verdict,
    pub confidence: Confidence,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Metric;

impl Measurement for Metric {
    type Subject = trace::Trace;
    type Value = Judgment;

    fn name(&self) -> &'static str {
        "tool_call_review"
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SelectIndexedCall {
    pub index: usize,
}

impl StepSpec for SelectIndexedCall {
    type Input = trace::Trace;
    type Output = Evidence;

    fn step_id(&self) -> &'static str {
        "select_indexed_tool_call"
    }

    fn step_name(&self) -> &'static str {
        "select_indexed_tool_call"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec![
                "subject.calls[index]".to_string(),
                "subject.calls[index].summary".to_string(),
            ],
            forbidden: vec![
                "external context".to_string(),
                "future turns not encoded in the selected call summary".to_string(),
            ],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SelectIndexedCallError {
    #[error("tool-call trace is empty")]
    EmptyTrace,
    #[error("selected tool call index {0} not found")]
    MissingIndex(usize),
}

impl MechanizedSpec for SelectIndexedCall {
    type Error = SelectIndexedCallError;

    fn execute_mechanized(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        if input.calls.is_empty() {
            return Err(SelectIndexedCallError::EmptyTrace);
        }

        let indexed = input
            .calls
            .into_iter()
            .find(|call| call.index == self.index)
            .ok_or(SelectIndexedCallError::MissingIndex(self.index))?;

        Ok(Evidence {
            index: indexed.index,
            turn: indexed.turn,
            tool_name: indexed.tool_name,
            failed: indexed.failed,
            summary: indexed.summary,
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReviewEvidence;

impl StepSpec for ReviewEvidence {
    type Input = Evidence;
    type Output = Judgment;

    fn step_id(&self) -> &'static str {
        "adjudicate_tool_call_review"
    }

    fn step_name(&self) -> &'static str {
        "adjudicate_tool_call_review"
    }

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            allowed: vec![
                "selected tool-call evidence packet".to_string(),
                "explicitly provided summary fields".to_string(),
            ],
            forbidden: vec![
                "unstated run context".to_string(),
                "external repository knowledge".to_string(),
                "counterfactual assumptions not supported by the packet".to_string(),
            ],
            hindsight_allowed: false,
            external_context_allowed: false,
        }
    }
}

impl JsonAdjudicationSpec for ReviewEvidence {
    fn build_prompt(&self, input: &Self::Input) -> JsonChatPrompt {
        JsonChatPrompt {
            system: "You are reviewing one localized tool call from an eval trace. Use only the provided evidence packet. Return exactly one JSON object with keys: index, verdict, confidence, rationale. Valid verdict values: appropriate, wrong_tool, bad_arguments, redundant, recoverable_failure, unclear. Valid confidence values: low, medium, high.".to_string(),
            user: format!(
                "Review this tool call.\n\nindex: {}\nturn: {}\ntool: {}\nfailed: {}\nsummary: {}\n",
                input.index,
                input.turn,
                input.tool_name,
                input.failed,
                input.summary
            ),
        }
    }
}

pub type ToolCallReviewInner =
    Sequence<Step<SelectIndexedCall, MechanizedExecutor>, Step<ReviewEvidence, JsonAdjudicator>>;

pub type ToolCallReviewArtifact = crate::core::ProcedureArtifact<
    crate::core::SequenceArtifact<
        crate::core::StepArtifact<trace::Trace, Evidence, crate::step::MechanizedProvenance>,
        crate::core::StepArtifact<Evidence, Judgment, crate::llm::JsonLlmProvenance>,
    >,
>;

pub type ToolCallReviewError = SequenceError<SelectIndexedCallError, ProtocolLlmError>;

#[derive(Debug, Clone)]
pub struct ToolCallReview {
    inner: NamedProcedure<ToolCallReviewInner>,
}

impl ToolCallReview {
    pub fn new(index: usize, adjudicator: JsonAdjudicator) -> Self {
        let select = Step::new(SelectIndexedCall { index }, MechanizedExecutor);
        let review = Step::new(ReviewEvidence, adjudicator);
        Self {
            inner: select.then(review).named("tool_call_review"),
        }
    }
}

#[async_trait]
impl Procedure for ToolCallReview {
    type Subject = trace::Trace;
    type Output = Judgment;
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
