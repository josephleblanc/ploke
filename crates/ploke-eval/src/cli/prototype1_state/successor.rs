//! Typed records for the successor handoff path.
//!
//! The successor is not a separate controller role or a live `Successor<State>`
//! authority carrier. It is the incoming Parent before handoff acknowledgement.
//! These records project that handoff path into the append-only transition
//! journal.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::intervention::CommitPhase;
use crate::intervention::Prototype1ContinuationDecision;

use super::event::{RecordedAt, RuntimeId};
use super::invocation::{SuccessorCompletionStatus, SuccessorInvocation};
use super::journal::Streams;

/// State projected by a recorded successor transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    /// Parent selected this child artifact as the next parent candidate.
    Selected {
        decision: Prototype1ContinuationDecision,
    },
    /// Parent spawned the successor process.
    Spawned {
        pid: u32,
        active_parent_root: PathBuf,
        binary_path: PathBuf,
        invocation_path: PathBuf,
        ready_path: PathBuf,
        streams: Streams,
    },
    /// Parent is installing the selected artifact into the active checkout.
    Checkout {
        phase: CommitPhase,
        active_parent_root: PathBuf,
        selected_branch: String,
        installed_commit: Option<String>,
    },
    /// Parent observed the successor acknowledgement.
    Ready { pid: u32, ready_path: PathBuf },
    /// Parent stopped waiting before acknowledgement.
    TimedOut { waited_ms: u64, ready_path: PathBuf },
    /// Successor process exited before acknowledgement.
    ExitedBeforeReady { exit_code: Option<i32> },
    /// Successor wrote its bounded-turn completion record.
    Completed {
        status: SuccessorCompletionStatus,
        completion_path: PathBuf,
        trace_path: Option<PathBuf>,
        detail: Option<String>,
    },
}

/// Durable record written by a typed successor transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Record {
    pub runtime_id: Option<RuntimeId>,
    pub recorded_at: RecordedAt,
    pub campaign_id: String,
    pub node_id: String,
    pub state: State,
}

impl Record {
    pub(crate) fn selected(
        campaign_id: String,
        node_id: String,
        decision: Prototype1ContinuationDecision,
    ) -> Self {
        Self {
            runtime_id: None,
            recorded_at: RecordedAt::now(),
            campaign_id,
            node_id,
            state: State::Selected { decision },
        }
    }

    pub(crate) fn checkout(
        campaign_id: String,
        node_id: String,
        phase: CommitPhase,
        active_parent_root: PathBuf,
        selected_branch: String,
        installed_commit: Option<String>,
    ) -> Self {
        Self {
            runtime_id: None,
            recorded_at: RecordedAt::now(),
            campaign_id,
            node_id,
            state: State::Checkout {
                phase,
                active_parent_root,
                selected_branch,
                installed_commit,
            },
        }
    }

    pub(crate) fn spawned(
        invocation: &SuccessorInvocation,
        pid: u32,
        active_parent_root: PathBuf,
        binary_path: PathBuf,
        invocation_path: PathBuf,
        ready_path: PathBuf,
        streams: Streams,
    ) -> Self {
        Self {
            runtime_id: Some(invocation.runtime_id()),
            recorded_at: RecordedAt::now(),
            campaign_id: invocation.campaign_id().to_string(),
            node_id: invocation.node_id().to_string(),
            state: State::Spawned {
                pid,
                active_parent_root,
                binary_path,
                invocation_path,
                ready_path,
                streams,
            },
        }
    }

    pub(crate) fn ready(invocation: &SuccessorInvocation, pid: u32, ready_path: PathBuf) -> Self {
        Self {
            runtime_id: Some(invocation.runtime_id()),
            recorded_at: RecordedAt::now(),
            campaign_id: invocation.campaign_id().to_string(),
            node_id: invocation.node_id().to_string(),
            state: State::Ready { pid, ready_path },
        }
    }

    pub(crate) fn timed_out(
        invocation: &SuccessorInvocation,
        waited_ms: u64,
        ready_path: PathBuf,
    ) -> Self {
        Self {
            runtime_id: Some(invocation.runtime_id()),
            recorded_at: RecordedAt::now(),
            campaign_id: invocation.campaign_id().to_string(),
            node_id: invocation.node_id().to_string(),
            state: State::TimedOut {
                waited_ms,
                ready_path,
            },
        }
    }

    pub(crate) fn exited_before_ready(
        invocation: &SuccessorInvocation,
        exit_code: Option<i32>,
    ) -> Self {
        Self {
            runtime_id: Some(invocation.runtime_id()),
            recorded_at: RecordedAt::now(),
            campaign_id: invocation.campaign_id().to_string(),
            node_id: invocation.node_id().to_string(),
            state: State::ExitedBeforeReady { exit_code },
        }
    }

    pub(crate) fn completed(
        invocation: &SuccessorInvocation,
        status: SuccessorCompletionStatus,
        completion_path: PathBuf,
        trace_path: Option<PathBuf>,
        detail: Option<String>,
    ) -> Self {
        Self {
            runtime_id: Some(invocation.runtime_id()),
            recorded_at: RecordedAt::now(),
            campaign_id: invocation.campaign_id().to_string(),
            node_id: invocation.node_id().to_string(),
            state: State::Completed {
                status,
                completion_path,
                trace_path,
                detail,
            },
        }
    }

    /// Stable display label for monitor summaries.
    pub(crate) fn entry_kind(&self) -> &'static str {
        match self.state {
            State::Selected { .. } => "successor:selected",
            State::Checkout { phase, .. } => match phase {
                CommitPhase::Before => "successor:checkout:before",
                CommitPhase::After => "successor:checkout:after",
            },
            State::Spawned { .. } => "successor:spawned",
            State::Ready { .. } => "successor:ready",
            State::TimedOut { .. } => "successor:timed_out",
            State::ExitedBeforeReady { .. } => "successor:exited_before_ready",
            State::Completed { .. } => "successor:completed",
        }
    }
}
