#![allow(dead_code)] // REMOVE BY 2026-04-26: typed C3 -> C4 scaffold is not wired into the live controller yet

//! Explicit `C3 -> C4` prototype configuration transition.
//!
//! Temporary note:
//! This file models the runtime handoff seam that the current prototype still
//! handles inside the old parent/child process helper. It introduces a
//! journal-backed handshake for child acknowledgement, but it does not yet
//! replace the live runner path. The journal now has replay classification for
//! these entries as well, even though the live runner is not yet wired to
//! append `ChildReady`.
//!
//! The current scaffold assumes:
//! - `C3` means the child binary exists but has not yet acknowledged itself
//! - `C4` means the parent has observed a matching child-ready witness
//! - the handshake is mediated through the shared transition journal
//! - the fresh child process bootstraps from one persisted invocation record
//!   written before spawn

use std::fs;
use std::path::PathBuf;
use std::process::{Child as ProcessChild, Command as ProcessCommand};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;
use tracing::{debug, instrument};

use crate::intervention::{
    CommitError, Intervention, Outcome, Prototype1NodeStatus, RecordStore, Surface,
    clear_runner_result, load_node_record, load_runner_request, resolve_treatment_branch,
    update_node_status,
};
use crate::spec::PrepareError;

use super::c1::{Acknowledged, Artifact, Binary, Child, ChildAckState, Parent, Present, Prototype};
use super::event::{ChildRuntimeLifecycle, ContentHash, Paths, RecordedAt, Refs, RuntimeId};
use super::invocation::{ChildInvocation, invocation_path, write_child_invocation};
use super::journal::{
    JournalEntry, PrototypeJournal, PrototypeJournalError, ReadyEntry, SpawnEntry,
    SpawnObservation, SpawnPhase,
};

/// Environment key used to tell the child which runtime instance it is.
pub(crate) const RUNTIME_ID_ENV: &str = "PLOKE_PROTOTYPE1_RUNTIME_ID";

/// Environment key used to tell the child which journal to append to.
pub(crate) const JOURNAL_PATH_ENV: &str = "PLOKE_PROTOTYPE1_TRANSITION_JOURNAL";

const READY_TIMEOUT: Duration = Duration::from_secs(10);
const READY_POLL: Duration = Duration::from_millis(50);

/// `C4`: parent binary over child artifact world with a concrete child runtime
/// acknowledged through the shared journal.
pub(crate) type C4 = Prototype<Parent, Child, Present, Acknowledged>;

/// `C3`: parent binary over child artifact world with a child binary present
/// but not yet acknowledged.
pub(crate) type C3 = super::c2::C3;

fn spawn_entry<AckState>(
    config: &Prototype<Parent, Child, Present, AckState>,
    runtime_id: RuntimeId,
    phase: SpawnPhase,
    argv: Vec<String>,
    parent_pid: u32,
    child_pid: Option<u32>,
    result: Option<SpawnObservation>,
) -> SpawnEntry
where
    AckState: ChildAckState,
{
    let child_lifecycle = match result {
        Some(SpawnObservation::Acknowledged) => ChildRuntimeLifecycle::Acknowledged,
        Some(SpawnObservation::TerminatedBeforeAcknowledged { .. }) => {
            ChildRuntimeLifecycle::Terminated
        }
        None => ChildRuntimeLifecycle::Spawned,
    };
    SpawnEntry {
        runtime_id,
        phase,
        recorded_at: RecordedAt::now(),
        generation: config.node.generation,
        refs: Refs {
            campaign_id: config.campaign_id.clone(),
            node_id: config.node.node_id.clone(),
            instance_id: config.node.instance_id.clone(),
            source_state_id: config.node.source_state_id.clone(),
            branch_id: config.node.branch_id.clone(),
            candidate_id: config.node.candidate_id.clone(),
            branch_label: config.resolved.branch.branch_label.clone(),
            spec_id: config.resolved.branch.synthesized_spec_id.clone(),
        },
        paths: Paths {
            repo_root: config.artifact.repo_root.clone(),
            workspace_root: config.artifact.repo_root.clone(),
            binary_path: config.binary.child_path.clone(),
            target_relpath: config.artifact.target_relpath.clone(),
            absolute_path: config
                .artifact
                .repo_root
                .join(&config.artifact.target_relpath),
        },
        world: crate::cli::prototype1_state::event::World {
            node_status: config.node.status,
            running_binary: config.binary.parent_running,
            running_lineage: crate::cli::prototype1_state::event::LineageMark::Parent,
            artifact_lineage: crate::cli::prototype1_state::event::LineageMark::Child,
            child_lifecycle: Some(child_lifecycle),
        },
        child_lifecycle,
        parent_pid,
        child_pid,
        argv,
        result,
    }
}

/// Shared journal-backed handoff view.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Handoff {
    path: PathBuf,
}

impl Handoff {
    fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn with_txn<R>(
        &self,
        f: impl FnOnce(&mut HandoffTxn) -> Result<R, PrototypeJournalError>,
    ) -> Result<R, PrototypeJournalError> {
        let mut txn = HandoffTxn {
            journal: PrototypeJournal::new(self.path.clone()),
        };
        let result = f(&mut txn)?;
        drop(txn);
        Ok(result)
    }

    fn with_read<R>(
        &self,
        f: impl FnOnce(&[JournalEntry]) -> R,
    ) -> Result<R, PrototypeJournalError> {
        let journal = PrototypeJournal::new(self.path.clone());
        let entries = journal.load_entries()?;
        Ok(f(&entries))
    }

    fn find_ready(
        &self,
        runtime_id: RuntimeId,
    ) -> Result<Option<ReadyEntry>, PrototypeJournalError> {
        self.with_read(|entries| {
            entries.iter().find_map(|entry| match entry {
                JournalEntry::ChildReady(ready) if ready.runtime_id == runtime_id => {
                    Some(ready.clone())
                }
                _ => None,
            })
        })
    }
}

struct HandoffTxn {
    journal: PrototypeJournal,
}

impl HandoffTxn {
    fn record_spawned(&mut self, entry: SpawnEntry) -> Result<(), PrototypeJournalError> {
        self.journal.append(JournalEntry::SpawnChild(entry))
    }

    fn record_ready(&mut self, entry: ReadyEntry) -> Result<(), PrototypeJournalError> {
        self.journal.append(JournalEntry::ChildReady(entry))
    }

    fn record_observed(&mut self, entry: SpawnEntry) -> Result<(), PrototypeJournalError> {
        self.journal.append(JournalEntry::SpawnChild(entry))
    }
}

/// Child-side helper for later wiring. This writes the ready witness without
/// leaving any journal handle alive after the closure returns.
#[instrument(
    target = "ploke_exec",
    level = "debug",
    skip(journal_path, entry),
    fields(runtime_id = %entry.runtime_id)
)]
pub(crate) fn record_child_ready(
    journal_path: impl Into<PathBuf>,
    entry: ReadyEntry,
) -> Result<(), PrototypeJournalError> {
    let journal_path = journal_path.into();
    debug!(
        target: ploke_core::EXECUTION_DEBUG_TARGET,
        runtime_id = %entry.runtime_id,
        journal_path = %journal_path.display(),
        "appending child ready entry"
    );
    Handoff::new(journal_path).with_txn(|txn| txn.record_ready(entry))
}

/// Typed failure for the `C3 -> C4` spawn transition.
#[derive(Debug, Error)]
pub(crate) enum SpawnChildError {
    #[error("failed to load node record '{node_id}'")]
    LoadNode {
        node_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("failed to load runner request for node '{node_id}'")]
    LoadRequest {
        node_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("failed to clear prior runner result for node '{node_id}'")]
    ClearRunnerResult {
        node_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("failed to resolve treatment branch '{branch_id}' for node '{node_id}'")]
    ResolveBranch {
        node_id: String,
        branch_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("node '{node_id}' is not binary_built: observed '{observed:?}'")]
    UnexpectedNodeStatus {
        node_id: String,
        observed: Prototype1NodeStatus,
    },
    #[error("failed to read target artifact '{path}': {source}")]
    ReadTarget {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("expected built artifact at '{path}' to match proposed branch content")]
    ArtifactNotBuilt {
        path: PathBuf,
        expected_proposed_hash: ContentHash,
        observed_hash: ContentHash,
    },
    #[error("expected promoted child binary at '{path}' before spawn")]
    MissingChildBinary { path: PathBuf },
    #[error("runner request binary path '{request}' does not match config child path '{config}'")]
    BinaryPathMismatch { request: PathBuf, config: PathBuf },
    #[error("failed to spawn child binary '{path}': {source}")]
    SpawnInvoke {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to persist invocation for node '{node_id}'")]
    WriteInvocation {
        node_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("failed to query child runtime status for runtime '{runtime_id}': {source}")]
    PollChildStatus {
        runtime_id: RuntimeId,
        source: std::io::Error,
    },
    #[error("failed to update node '{node_id}' status")]
    UpdateNodeStatus {
        node_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("failed to read handoff journal")]
    ReadJournal {
        #[source]
        source: PrototypeJournalError,
    },
}

/// Committed non-success result for the `C3 -> C4` handoff transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rejected {
    ExitedBeforeReady {
        runtime_id: RuntimeId,
        child_pid: u32,
        exit_code: Option<i32>,
    },
    ReadyTimedOut {
        runtime_id: RuntimeId,
        child_pid: u32,
        waited_ms: u64,
    },
}

impl Rejected {
    fn exited_before_ready_result(&self) -> Option<SpawnObservation> {
        match self {
            Self::ExitedBeforeReady { exit_code, .. } => {
                Some(SpawnObservation::TerminatedBeforeAcknowledged {
                    exit_code: *exit_code,
                })
            }
            Self::ReadyTimedOut { .. } => None,
        }
    }
}

/// Surface over the built child binary path used by the handoff transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ChildBinarySurface;

impl Surface<C3> for ChildBinarySurface {
    type Target = ();
    type ReadView = PathBuf;
    type Error = SpawnChildError;

    fn read_view(&self, config: &C3, _: &Self::Target) -> Result<Self::ReadView, Self::Error> {
        Ok(config.binary.child_path.clone())
    }
}

impl C3 {
    /// Load and validate a built `C3` state for one node.
    pub(crate) fn load(
        campaign_id: impl Into<String>,
        campaign_manifest_path: impl Into<PathBuf>,
        node_id: &str,
        repo_root: impl Into<PathBuf>,
    ) -> Result<Self, SpawnChildError> {
        let campaign_id = campaign_id.into();
        let campaign_manifest_path = campaign_manifest_path.into();
        let repo_root = repo_root.into();

        let node = load_node_record(&campaign_manifest_path, node_id).map_err(|source| {
            SpawnChildError::LoadNode {
                node_id: node_id.to_string(),
                source,
            }
        })?;
        if node.status != Prototype1NodeStatus::BinaryBuilt {
            return Err(SpawnChildError::UnexpectedNodeStatus {
                node_id: node_id.to_string(),
                observed: node.status,
            });
        }
        let resolved =
            resolve_treatment_branch(&campaign_id, &campaign_manifest_path, &node.branch_id)
                .map_err(|source| SpawnChildError::ResolveBranch {
                    node_id: node_id.to_string(),
                    branch_id: node.branch_id.clone(),
                    source,
                })?;

        let absolute_path = repo_root.join(&resolved.target_relpath);
        let current =
            fs::read_to_string(&absolute_path).map_err(|source| SpawnChildError::ReadTarget {
                path: absolute_path.clone(),
                source,
            })?;
        let observed_hash = ContentHash::of(&current);
        if current != resolved.branch.proposed_content {
            return Err(SpawnChildError::ArtifactNotBuilt {
                path: absolute_path,
                expected_proposed_hash: ContentHash(resolved.branch.proposed_content_hash.clone()),
                observed_hash,
            });
        }
        if !node.binary_path.exists() {
            return Err(SpawnChildError::MissingChildBinary {
                path: node.binary_path.clone(),
            });
        }

        let child_path = node.binary_path.clone();

        Ok(Self {
            campaign_id,
            campaign_manifest_path,
            node,
            resolved: resolved.clone(),
            artifact: Artifact {
                repo_root,
                target_relpath: resolved.target_relpath.clone(),
                source_content_hash: ContentHash(resolved.source_content_hash.clone()),
                current_content_hash: ContentHash(resolved.branch.proposed_content_hash.clone()),
                proposed_content_hash: ContentHash(resolved.branch.proposed_content_hash.clone()),
                _lineage: std::marker::PhantomData,
            },
            binary: Binary {
                parent_running: true,
                child_path,
                child_runtime: None,
                _lineage: std::marker::PhantomData,
                _child: std::marker::PhantomData,
                _ack: std::marker::PhantomData,
            },
        })
    }
}

/// Concrete intervention mediating `C3 -> C4`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpawnChild {
    runtime_id: RuntimeId,
}

impl SpawnChild {
    pub(crate) fn new() -> Self {
        Self {
            runtime_id: RuntimeId::new(),
        }
    }
}

impl Intervention<C3, C4> for SpawnChild {
    type Surface = ChildBinarySurface;
    type Journal = PrototypeJournal;
    type Error = SpawnChildError;
    type Rejected = Rejected;

    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self, records),
        fields(node_id = %from.node.node_id, branch_id = %from.resolved.branch.branch_id, runtime_id = %self.runtime_id)
    )]
    fn transition(
        &self,
        from: C3,
        records: &mut Self::Journal,
    ) -> Result<
        Outcome<C4, Self::Rejected>,
        CommitError<Self::Error, <Self::Journal as RecordStore>::Error>,
    > {
        let binary_path = ChildBinarySurface
            .read_view(&from, &())
            .map_err(CommitError::Transition)?;
        let request = load_runner_request(&from.campaign_manifest_path, &from.node.node_id)
            .map_err(|source| {
                CommitError::Transition(SpawnChildError::LoadRequest {
                    node_id: from.node.node_id.clone(),
                    source,
                })
            })?;
        if request.binary_path != from.binary.child_path {
            return Err(CommitError::Transition(
                SpawnChildError::BinaryPathMismatch {
                    request: request.binary_path,
                    config: from.binary.child_path.clone(),
                },
            ));
        }
        let _ = clear_runner_result(&from.campaign_manifest_path, &from.node.node_id).map_err(
            |source| {
                CommitError::Transition(SpawnChildError::ClearRunnerResult {
                    node_id: from.node.node_id.clone(),
                    source,
                })
            },
        )?;
        let invocation_path = invocation_path(&from.node.node_dir, self.runtime_id);
        let invocation = ChildInvocation::new(
            from.campaign_id.clone(),
            from.node.node_id.clone(),
            self.runtime_id,
            records.path().to_path_buf(),
        );
        write_child_invocation(&invocation_path, &invocation).map_err(|source| {
            CommitError::Transition(SpawnChildError::WriteInvocation {
                node_id: from.node.node_id.clone(),
                source,
            })
        })?;
        let child_argv = invocation.launch_args(&invocation_path);

        let handoff = Handoff::new(records.path().to_path_buf());
        let parent_pid = std::process::id();
        let mut child = ProcessCommand::new(&binary_path)
            .args(&child_argv)
            .current_dir(&from.artifact.repo_root)
            .spawn()
            .map_err(|source| {
                CommitError::Transition(SpawnChildError::SpawnInvoke {
                    path: binary_path.clone(),
                    source,
                })
            })?;
        let child_pid = child.id();
        debug!(
            target: ploke_core::EXECUTION_DEBUG_TARGET,
            node_id = %from.node.node_id,
            branch_id = %from.resolved.branch.branch_id,
            runtime_id = %self.runtime_id,
            child_pid,
            journal_path = %handoff.path.display(),
            "spawned child runtime"
        );

        handoff
            .with_txn(|txn| {
                txn.record_spawned(spawn_entry(
                    &from,
                    self.runtime_id,
                    SpawnPhase::Spawned,
                    child_argv.clone(),
                    parent_pid,
                    Some(child_pid),
                    None,
                ))
            })
            .map_err(|source| CommitError::Record {
                phase: crate::intervention::CommitPhase::Before,
                source,
            })?;

        let outcome = wait_for_ready(&handoff, &mut child, self.runtime_id)
            .map_err(CommitError::Transition)?;

        match outcome {
            WaitOutcome::Ready(_ready) => {
                let (_, node) = update_node_status(
                    &from.campaign_id,
                    &from.campaign_manifest_path,
                    &from.node.node_id,
                    Prototype1NodeStatus::Running,
                )
                .map_err(|source| {
                    CommitError::Transition(SpawnChildError::UpdateNodeStatus {
                        node_id: from.node.node_id.clone(),
                        source,
                    })
                })?;

                let next = Prototype {
                    campaign_id: from.campaign_id,
                    campaign_manifest_path: from.campaign_manifest_path,
                    node,
                    resolved: from.resolved,
                    artifact: from.artifact,
                    binary: Binary {
                        parent_running: true,
                        child_path: from.binary.child_path,
                        child_runtime: Some(self.runtime_id),
                        _lineage: std::marker::PhantomData,
                        _child: std::marker::PhantomData,
                        _ack: std::marker::PhantomData,
                    },
                };

                handoff
                    .with_txn(|txn| {
                        txn.record_observed(spawn_entry(
                            &next,
                            self.runtime_id,
                            SpawnPhase::Observed,
                            child_argv.clone(),
                            parent_pid,
                            Some(child_pid),
                            Some(SpawnObservation::Acknowledged),
                        ))
                    })
                    .map_err(|source| CommitError::Record {
                        phase: crate::intervention::CommitPhase::After,
                        source,
                    })?;
                debug!(
                    target: ploke_core::EXECUTION_DEBUG_TARGET,
                    node_id = %next.node.node_id,
                    branch_id = %next.resolved.branch.branch_id,
                    runtime_id = %self.runtime_id,
                    child_pid,
                    "recorded spawn observed entry"
                );

                Ok(Outcome::Advanced(next))
            }
            WaitOutcome::Rejected(rejected) => {
                debug!(
                    target: ploke_core::EXECUTION_DEBUG_TARGET,
                    node_id = %from.node.node_id,
                    branch_id = %from.resolved.branch.branch_id,
                    runtime_id = %self.runtime_id,
                    rejected = ?rejected,
                    "spawn handshake rejected"
                );
                if let Some(result) = rejected.exited_before_ready_result() {
                    let (_, failed_node) = update_node_status(
                        &from.campaign_id,
                        &from.campaign_manifest_path,
                        &from.node.node_id,
                        Prototype1NodeStatus::Failed,
                    )
                    .map_err(|source| {
                        CommitError::Transition(SpawnChildError::UpdateNodeStatus {
                            node_id: from.node.node_id.clone(),
                            source,
                        })
                    })?;
                    let failed = Prototype {
                        campaign_id: from.campaign_id,
                        campaign_manifest_path: from.campaign_manifest_path,
                        node: failed_node,
                        resolved: from.resolved,
                        artifact: from.artifact,
                        binary: from.binary,
                    };

                    handoff
                        .with_txn(|txn| {
                            txn.record_observed(spawn_entry(
                                &failed,
                                self.runtime_id,
                                SpawnPhase::Observed,
                                child_argv.clone(),
                                parent_pid,
                                Some(child_pid),
                                Some(result),
                            ))
                        })
                        .map_err(|source| CommitError::Record {
                            phase: crate::intervention::CommitPhase::After,
                            source,
                        })?;
                }

                Ok(Outcome::Rejected(rejected))
            }
        }
    }
}

enum WaitOutcome {
    Ready(Box<ReadyEntry>),
    Rejected(Rejected),
}

fn wait_for_ready(
    handoff: &Handoff,
    child: &mut ProcessChild,
    runtime_id: RuntimeId,
) -> Result<WaitOutcome, SpawnChildError> {
    let start = Instant::now();
    loop {
        if let Some(ready) = handoff
            .find_ready(runtime_id)
            .map_err(|source| SpawnChildError::ReadJournal { source })?
        {
            debug!(
                target: ploke_core::EXECUTION_DEBUG_TARGET,
                runtime_id = %runtime_id,
                pid = ready.pid,
                "observed child ready handshake"
            );
            return Ok(WaitOutcome::Ready(Box::new(ready)));
        }

        if let Some(status) = child
            .try_wait()
            .map_err(|source| SpawnChildError::PollChildStatus { runtime_id, source })?
        {
            return Ok(WaitOutcome::Rejected(Rejected::ExitedBeforeReady {
                runtime_id,
                child_pid: child.id(),
                exit_code: status.code(),
            }));
        }

        let waited = start.elapsed();
        if waited >= READY_TIMEOUT {
            return Ok(WaitOutcome::Rejected(Rejected::ReadyTimedOut {
                runtime_id,
                child_pid: child.id(),
                waited_ms: waited.as_millis() as u64,
            }));
        }

        thread::sleep(READY_POLL);
    }
}
