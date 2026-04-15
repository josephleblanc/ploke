use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core::{Confidence, Measurement, Protocol as ProtocolTrait};
use crate::llm::JsonChatPrompt;
use crate::tool_calls::trace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    FailedCall,
    WrongTool,
    RedundantRepeat,
    LikelyBadArguments,
    RecoveryOpportunity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub index: usize,
    pub kind: CandidateKind,
    pub confidence: Confidence,
    pub reason: String,
}

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
pub struct Judgment {
    pub index: usize,
    pub verdict: Verdict,
    pub confidence: Confidence,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub index: usize,
    pub turn: u32,
    pub tool_name: String,
    pub failed: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Metric;

impl Measurement for Metric {
    type Subject = Evidence;
    type Value = Judgment;

    fn name(&self) -> &'static str {
        "tool_call_review"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub trace: trace::Trace,
    pub candidate: Candidate,
}

/// Bootstrap specification for a bounded localized tool-call review.
#[derive(Debug, Clone, Copy, Default)]
pub struct Spec;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("tool-call trace is empty")]
    EmptyTrace,
    #[error("selected tool call index {0} not found")]
    MissingIndex(usize),
}

impl Spec {
    /// Cheap heuristic seed for later adjudication: pick the first failed call,
    /// otherwise the first call in the trace.
    pub fn select_candidate(&self, subject: &trace::Trace) -> Result<Candidate, Error> {
        let first = subject.calls.first().ok_or(Error::EmptyTrace)?;

        let candidate = subject
            .calls
            .iter()
            .find(|call| call.failed)
            .unwrap_or(first);

        let kind = if candidate.failed {
            CandidateKind::FailedCall
        } else {
            CandidateKind::RecoveryOpportunity
        };

        Ok(Candidate {
            index: candidate.index,
            kind,
            confidence: Confidence::Low,
            reason: "heuristic seed selection".to_string(),
        })
    }

    pub fn build_state(&self, trace: trace::Trace) -> Result<State, Error> {
        let candidate = self.select_candidate(&trace)?;
        Ok(State { trace, candidate })
    }

    pub fn build_evidence(&self, state: &State) -> Result<Evidence, Error> {
        let call = state
            .trace
            .calls
            .iter()
            .find(|call| call.index == state.candidate.index)
            .ok_or(Error::MissingIndex(state.candidate.index))?;

        Ok(Evidence {
            index: call.index,
            turn: call.turn,
            tool_name: call.tool_name.clone(),
            failed: call.failed,
            summary: call.summary.clone(),
        })
    }

    pub fn build_review_prompt(&self, evidence: &Evidence) -> JsonChatPrompt {
        let system = "You are reviewing one tool call from an eval trace. Return only a JSON object with fields verdict, confidence, and rationale. Allowed verdict values: appropriate, wrong_tool, bad_arguments, redundant, recoverable_failure, unclear. Allowed confidence values: low, medium, high.".to_string();

        let evidence_json = serde_json::to_string_pretty(evidence)
            .expect("tool-call review evidence should serialize");
        let user = format!(
            "Review the following tool call evidence and classify whether the call was appropriate.\n\nTool call evidence JSON:\n{evidence_json}\n"
        );

        JsonChatPrompt { system, user }
    }
}

impl ProtocolTrait for Spec {
    type Subject = trace::Trace;
    type State = State;
    type Output = Evidence;
    type Error = Error;

    fn name(&self) -> &'static str {
        "tool_call_review"
    }

    fn run(&self, subject: Self::Subject) -> Result<Self::Output, Self::Error> {
        let state = self.build_state(subject)?;
        self.build_evidence(&state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_prefers_failed_call_when_present() {
        let protocol = Spec;
        let subject = trace::Trace {
            subject_id: "run-1".to_string(),
            calls: vec![
                trace::Call {
                    index: 0,
                    turn: 1,
                    tool_name: "search".to_string(),
                    summary: "searched".to_string(),
                    failed: false,
                },
                trace::Call {
                    index: 1,
                    turn: 2,
                    tool_name: "read_file".to_string(),
                    summary: "bad path".to_string(),
                    failed: true,
                },
            ],
        };

        let evidence = protocol.run(subject).expect("evidence");
        assert_eq!(evidence.index, 1);
        assert!(evidence.failed);
    }

    #[test]
    fn protocol_falls_back_to_first_call() {
        let protocol = Spec;
        let subject = trace::Trace {
            subject_id: "run-2".to_string(),
            calls: vec![trace::Call {
                index: 0,
                turn: 1,
                tool_name: "search".to_string(),
                summary: "searched".to_string(),
                failed: false,
            }],
        };

        let evidence = protocol.run(subject).expect("evidence");
        assert_eq!(evidence.index, 0);
        assert!(!evidence.failed);
    }
}
