//! Typed records for the successor handoff path.
//!
//! The successor is not a separate controller role or a live `Successor<State>`
//! authority carrier. It is the incoming Parent before handoff acknowledgement.
//! These records project that handoff path into the append-only transition
//! journal.

use crate::prelude::*;

use crate::intervention::CommitPhase;
use crate::intervention::Prototype1ContinuationDecision;
use crate::successor_selection::SuccessorDecision;

use super::event::{RecordedAt, RuntimeId, TransitionId};
use super::history::HistoryHash;
use super::invocation::{
    ProcessIncarnation, SUCCESSOR_READY_SCHEMA_VERSION, SUCCESSOR_READY_SCHEMA_VERSION_V1,
    SuccessorCompletionStatus, SuccessorInvocation, SuccessorReadyRecord, process_incarnation,
};
use super::journal::Streams;
use super::profile::RunMode;
use super::session::{Cursor as SessionCursor, Fence, SessionId};
use super::walk::{endpoint::ServerEndpoint, phase::WalkPhase};

const READY_SCHEMA: &str = "prototype1-controller-ready.v1";

/// Exact predecessor controller attempt that observed successor readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PredecessorAttempt {
    session: SessionId,
    transition: TransitionId,
    fence: Fence,
    #[serde(default)]
    allow_live_api: bool,
    #[serde(default)]
    allow_git_changes: bool,
}

impl PredecessorAttempt {
    pub(crate) const fn new(
        session: SessionId,
        transition: TransitionId,
        fence: Fence,
        allow_live_api: bool,
        allow_git_changes: bool,
    ) -> Self {
        Self {
            session,
            transition,
            fence,
            allow_live_api,
            allow_git_changes,
        }
    }

    pub(crate) const fn session(&self) -> SessionId {
        self.session
    }

    pub(crate) const fn transition(&self) -> TransitionId {
        self.transition
    }

    pub(crate) const fn fence(&self) -> Fence {
        self.fence
    }

    pub(crate) const fn allow_live_api(&self) -> bool {
        self.allow_live_api
    }

    pub(crate) const fn allow_git_changes(&self) -> bool {
        self.allow_git_changes
    }
}

/// Exact committed R4c session edge from which Ready may be published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadyCommit {
    session_id: SessionId,
    transition_id: TransitionId,
    fence: Fence,
    cursor: SessionCursor,
    mode: RunMode,
}

impl ReadyCommit {
    pub(crate) fn new(
        session_id: SessionId,
        transition_id: TransitionId,
        fence: Fence,
        cursor: SessionCursor,
        mode: RunMode,
    ) -> Result<Self, String> {
        if cursor.phase != WalkPhase::R4c {
            return Err(format!(
                "successor Ready requires a committed R4c cursor, found {}",
                cursor.phase
            ));
        }
        Ok(Self {
            session_id,
            transition_id,
            fence,
            cursor,
            mode,
        })
    }

    pub(crate) fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub(crate) fn transition_id(&self) -> TransitionId {
        self.transition_id
    }

    pub(crate) fn fence(&self) -> Fence {
        self.fence
    }

    pub(crate) fn cursor(&self) -> &SessionCursor {
        &self.cursor
    }

    pub(crate) fn mode(&self) -> RunMode {
        self.mode
    }
}

/// Parent-visible proof that the successor's R4c session is durable and, in
/// Step mode, its isolated inspection endpoint is already bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadyReceipt {
    schema_version: String,
    record: SuccessorReadyRecord,
    commit: ReadyCommit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    endpoint: Option<ServerEndpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    predecessor: Option<ServerEndpoint>,
}

impl ReadyReceipt {
    pub(crate) fn new(
        record: SuccessorReadyRecord,
        commit: ReadyCommit,
        endpoint: Option<ServerEndpoint>,
        predecessor: Option<ServerEndpoint>,
    ) -> Result<Self, String> {
        let receipt = Self {
            schema_version: READY_SCHEMA.to_string(),
            record,
            commit,
            endpoint,
            predecessor,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub(crate) fn record(&self) -> &SuccessorReadyRecord {
        &self.record
    }

    pub(crate) fn commit(&self) -> &ReadyCommit {
        &self.commit
    }

    pub(crate) fn endpoint(&self) -> Option<&ServerEndpoint> {
        self.endpoint.as_ref()
    }

    pub(crate) fn predecessor(&self) -> Option<&ServerEndpoint> {
        self.predecessor.as_ref()
    }

    /// Determine whether this process is the exact Ready publisher, including
    /// its Linux boot and process start identity rather than PID alone.
    pub(crate) fn owned_by_current(&self) -> Result<bool, String> {
        let expected = self.record.incarnation.as_ref().ok_or_else(|| {
            "legacy successor Ready has no process incarnation; automatic ownership recovery is unavailable"
                .to_string()
        })?;
        if self.record.pid != std::process::id() {
            return Ok(false);
        }
        let actual = process_incarnation(self.record.pid)
            .map_err(|source| format!("cannot inspect Ready process incarnation: {source}"))?;
        Ok(actual.as_ref() == Some(expected))
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        self.validate_persisted()?;
        if let Some(endpoint) = self.endpoint.as_ref() {
            endpoint.validate().map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Validate durable receipt structure without requiring its historical
    /// socket inode to remain live during later journal replay.
    pub(crate) fn validate_persisted(&self) -> Result<(), String> {
        if self.schema_version != READY_SCHEMA {
            return Err(format!(
                "unsupported controller Ready schema '{}'",
                self.schema_version
            ));
        }
        match self.record.schema_version.as_str() {
            SUCCESSOR_READY_SCHEMA_VERSION => {
                let incarnation = self.record.incarnation.as_ref().ok_or_else(|| {
                    "current successor Ready record has no process incarnation".to_string()
                })?;
                if incarnation.boot_id.is_nil() || incarnation.start_ticks == 0 {
                    return Err("successor Ready process incarnation is incomplete".to_string());
                }
            }
            SUCCESSOR_READY_SCHEMA_VERSION_V1 if self.record.incarnation.is_none() => {}
            schema => {
                return Err(format!(
                    "unsupported successor Ready record schema '{schema}'"
                ));
            }
        }
        if self.commit.cursor.phase != WalkPhase::R4c {
            return Err("controller Ready cursor is not R4c".to_string());
        }
        if let Some(endpoint) = self.endpoint.as_ref() {
            endpoint
                .validate_persisted()
                .map_err(|error| error.to_string())?;
        }
        if let Some(predecessor) = self.predecessor.as_ref() {
            predecessor
                .validate_persisted()
                .map_err(|error| error.to_string())?;
        }
        match (
            self.commit.mode,
            self.endpoint.as_ref(),
            self.predecessor.as_ref(),
        ) {
            (RunMode::Step, Some(endpoint), Some(predecessor))
                if endpoint.pid() == self.record.pid
                    && predecessor.repo_root() == endpoint.repo_root()
                    && predecessor.socket() != endpoint.socket() =>
            {
                let _ = endpoint;
            }
            (RunMode::Step, _, _) => {
                return Err(
                    "Step-mode controller Ready requires a bound successor endpoint and a distinct same-repository predecessor"
                        .to_string(),
                );
            }
            (RunMode::Continuous, None, None) => {}
            (RunMode::Continuous, _, _) => {
                return Err(
                    "Continuous-mode controller Ready cannot claim Step endpoint authority"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

/// Durable predecessor acknowledgement of one exact Ready receipt under one
/// pending R12 controller attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HandoffAcceptance {
    ready: ReadyReceipt,
    attempt: PredecessorAttempt,
}

impl HandoffAcceptance {
    pub(crate) fn new(ready: ReadyReceipt, attempt: PredecessorAttempt) -> Result<Self, String> {
        ready.validate()?;
        Ok(Self { ready, attempt })
    }

    /// Rebuild an acceptance after the original Ready owner exited.
    ///
    /// Recovery validates the persisted Ready authority and its exact
    /// predecessor attempt, but cannot require the historical socket inode to
    /// remain owned by the exited process.
    pub(crate) fn from_persisted(
        ready: ReadyReceipt,
        attempt: PredecessorAttempt,
    ) -> Result<Self, String> {
        ready.validate_persisted()?;
        Ok(Self { ready, attempt })
    }

    pub(crate) fn ready(&self) -> &ReadyReceipt {
        &self.ready
    }

    pub(crate) fn attempt(&self) -> &PredecessorAttempt {
        &self.attempt
    }
}

/// State projected by a recorded successor transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    /// Parent selected this child artifact as the next parent candidate.
    Selected {
        decision: Prototype1ContinuationDecision,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selection_decision: Option<SuccessorDecision>,
    },
    /// Parent consumed the handoff phase without spawning a successor.
    Stopped {
        decision: Prototype1ContinuationDecision,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selection_decision: Option<SuccessorDecision>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selection_receipt: Option<SelectionReceipt>,
    },
    /// Parent spawned the successor process.
    Spawned {
        pid: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        incarnation: Option<ProcessIncarnation>,
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
    Ready {
        pid: u32,
        ready_path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        controller: Option<ReadyReceipt>,
    },
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SelectionReceipt {
    Completed { hash: HistoryHash },
    NotRun,
}

impl State {
    pub(crate) fn allows_successor_handoff(&self) -> bool {
        matches!(
            self,
            Self::Selected { decision, .. } if decision.disposition.allows_successor()
        )
    }
}

/// Durable record written by a typed successor transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Record {
    pub runtime_id: Option<RuntimeId>,
    pub recorded_at: RecordedAt,
    pub campaign_id: CampaignId,
    pub node_id: String,
    pub state: State,
}

impl Record {
    #[cfg(test)]
    pub(crate) fn selected(
        campaign_id: CampaignId,
        node_id: String,
        decision: Prototype1ContinuationDecision,
    ) -> Self {
        Self {
            runtime_id: None,
            recorded_at: RecordedAt::now(),
            campaign_id,
            node_id,
            state: State::Selected {
                decision,
                selection_decision: None,
            },
        }
    }

    pub(crate) fn selected_with_decision(
        campaign_id: CampaignId,
        node_id: String,
        decision: Prototype1ContinuationDecision,
        selection_decision: SuccessorDecision,
    ) -> Self {
        Self {
            runtime_id: None,
            recorded_at: RecordedAt::now(),
            campaign_id,
            node_id,
            state: State::Selected {
                decision,
                selection_decision: Some(selection_decision),
            },
        }
    }

    pub(crate) fn stopped(
        campaign_id: CampaignId,
        node_id: String,
        decision: Prototype1ContinuationDecision,
        selection_decision: SuccessorDecision,
    ) -> Self {
        Self {
            runtime_id: None,
            recorded_at: RecordedAt::now(),
            campaign_id,
            node_id,
            state: State::Stopped {
                decision,
                selection_decision: Some(selection_decision),
                selection_receipt: None,
            },
        }
    }

    pub(crate) fn stopped_without_selection(
        campaign_id: CampaignId,
        node_id: String,
        decision: Prototype1ContinuationDecision,
        selection_receipt_hash: HistoryHash,
    ) -> Self {
        Self {
            runtime_id: None,
            recorded_at: RecordedAt::now(),
            campaign_id,
            node_id,
            state: State::Stopped {
                decision,
                selection_decision: None,
                selection_receipt: Some(SelectionReceipt::Completed {
                    hash: selection_receipt_hash,
                }),
            },
        }
    }

    pub(crate) fn stopped_without_attempt(
        campaign_id: CampaignId,
        node_id: String,
        decision: Prototype1ContinuationDecision,
    ) -> Self {
        Self {
            runtime_id: None,
            recorded_at: RecordedAt::now(),
            campaign_id,
            node_id,
            state: State::Stopped {
                decision,
                selection_decision: None,
                selection_receipt: Some(SelectionReceipt::NotRun),
            },
        }
    }

    pub(crate) fn checkout(
        campaign_id: CampaignId,
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
        incarnation: ProcessIncarnation,
        active_parent_root: PathBuf,
        binary_path: PathBuf,
        invocation_path: PathBuf,
        ready_path: PathBuf,
        streams: Streams,
    ) -> Self {
        Self {
            runtime_id: Some(invocation.runtime_id()),
            recorded_at: RecordedAt::now(),
            campaign_id: invocation.campaign_id().clone(),
            node_id: invocation.node_id().to_string(),
            state: State::Spawned {
                pid,
                incarnation: Some(incarnation),
                active_parent_root,
                binary_path,
                invocation_path,
                ready_path,
                streams,
            },
        }
    }

    pub(crate) fn ready(
        invocation: &SuccessorInvocation,
        pid: u32,
        ready_path: PathBuf,
        controller: ReadyReceipt,
    ) -> Self {
        Self {
            runtime_id: Some(invocation.runtime_id()),
            recorded_at: RecordedAt::now(),
            campaign_id: invocation.campaign_id().clone(),
            node_id: invocation.node_id().to_string(),
            state: State::Ready {
                pid,
                ready_path,
                controller: Some(controller),
            },
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
            campaign_id: invocation.campaign_id().clone(),
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
            campaign_id: invocation.campaign_id().clone(),
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
            campaign_id: invocation.campaign_id().clone(),
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
            State::Stopped { .. } => "successor:stopped",
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

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::net::UnixListener;

    use tempfile::tempdir;

    use super::*;
    use crate::cli::prototype1_state::{event::ContentHash, invocation::record_runtime_id};

    #[test]
    fn legacy_spawned_record_decodes_without_incarnation() {
        let value = serde_json::json!({
            "runtime_id": null,
            "recorded_at": 1,
            "campaign_id": "campaign",
            "node_id": "node",
            "state": {
                "spawned": {
                    "pid": 4242,
                    "active_parent_root": "/tmp/parent",
                    "binary_path": "/tmp/ploke-eval",
                    "invocation_path": "/tmp/invocation.json",
                    "ready_path": "/tmp/ready.json",
                    "streams": {
                        "stdout": "/tmp/stdout.log",
                        "stderr": "/tmp/stderr.log"
                    }
                }
            }
        });
        let record: Record = serde_json::from_value(value).expect("decode legacy Spawned record");
        assert!(matches!(
            record.state,
            State::Spawned {
                incarnation: None,
                ..
            }
        ));
    }

    #[test]
    fn ready_owner_uses_process_incarnation_not_pid_alone() {
        let runtime = RuntimeId::new();
        let commit = ReadyCommit::new(
            SessionId::for_test(1),
            TransitionId::new(),
            Fence::for_test(1),
            SessionCursor::new(WalkPhase::R4c, ContentHash::of("ready")).expect("Ready cursor"),
            RunMode::Continuous,
        )
        .expect("Ready commit");
        let pid = std::process::id();
        let incarnation = process_incarnation(pid)
            .expect("capture test process")
            .expect("test process exists");
        let mut receipt = ReadyReceipt::new(
            SuccessorReadyRecord {
                schema_version: SUCCESSOR_READY_SCHEMA_VERSION.to_string(),
                campaign_id: CampaignId::from("campaign"),
                node_id: "node".to_string(),
                runtime_id: record_runtime_id(runtime),
                pid,
                incarnation: Some(incarnation),
                recorded_at: "2026-07-13T00:00:00Z".to_string(),
            },
            commit.clone(),
            None,
            None,
        )
        .expect("current Ready receipt");
        assert!(receipt.owned_by_current().expect("inspect current owner"));

        receipt
            .record
            .incarnation
            .as_mut()
            .expect("recorded incarnation")
            .start_ticks += 1;
        assert!(
            !receipt
                .owned_by_current()
                .expect("inspect reused numeric pid")
        );

        let legacy = ReadyReceipt::new(
            SuccessorReadyRecord {
                schema_version: SUCCESSOR_READY_SCHEMA_VERSION_V1.to_string(),
                campaign_id: CampaignId::from("campaign"),
                node_id: "node".to_string(),
                runtime_id: record_runtime_id(runtime),
                pid,
                incarnation: None,
                recorded_at: "2026-07-13T00:00:00Z".to_string(),
            },
            commit,
            None,
            None,
        )
        .expect("legacy Ready remains replayable");
        assert!(legacy.owned_by_current().is_err());
    }

    #[test]
    fn ready_receipt_persists_exact_predecessor_endpoint() {
        let temp = tempdir().expect("tempdir");
        let predecessor_socket = temp.path().join("predecessor.sock");
        let successor_socket = temp.path().join("successor.sock");
        let predecessor_listener =
            UnixListener::bind(&predecessor_socket).expect("bind predecessor endpoint");
        let successor_listener =
            UnixListener::bind(&successor_socket).expect("bind successor endpoint");
        let predecessor = ServerEndpoint::from_bound(temp.path().to_path_buf(), predecessor_socket)
            .expect("capture predecessor endpoint");
        let successor = ServerEndpoint::from_bound(temp.path().to_path_buf(), successor_socket)
            .expect("capture successor endpoint");
        let commit = ReadyCommit::new(
            SessionId::for_test(1),
            TransitionId::new(),
            Fence::for_test(1),
            SessionCursor::new(WalkPhase::R4c, ContentHash::of("ready")).expect("Ready cursor"),
            RunMode::Step,
        )
        .expect("Ready commit");
        let record = SuccessorReadyRecord {
            schema_version: SUCCESSOR_READY_SCHEMA_VERSION.to_string(),
            campaign_id: CampaignId::from("campaign"),
            node_id: "node".to_string(),
            runtime_id: record_runtime_id(RuntimeId::new()),
            pid: std::process::id(),
            incarnation: Some(
                crate::cli::prototype1_state::invocation::process_incarnation(std::process::id())
                    .expect("capture test process")
                    .expect("test process exists"),
            ),
            recorded_at: "2026-07-13T00:00:00Z".to_string(),
        };
        let missing = ReadyReceipt::new(
            record.clone(),
            commit.clone(),
            Some(successor.clone()),
            None,
        )
        .expect_err("Step Ready without predecessor must fail closed");
        assert!(missing.contains("distinct same-repository predecessor"));
        let receipt = ReadyReceipt::new(
            record,
            commit,
            Some(successor.clone()),
            Some(predecessor.clone()),
        )
        .expect("Ready receipt");

        let json = serde_json::to_vec(&receipt).expect("serialize Ready receipt");
        let replayed: ReadyReceipt = serde_json::from_slice(&json).expect("replay Ready receipt");
        assert_eq!(replayed.predecessor(), Some(&predecessor));
        assert_eq!(replayed.endpoint(), Some(&successor));

        drop((predecessor_listener, successor_listener));
    }
}
