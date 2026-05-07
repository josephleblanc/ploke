#![allow(dead_code)] // REMOVE BY 2026-04-26: typed C4 -> C5 scaffold is not wired into the live controller yet

//! Explicit parent-side observation of child completion after `C4`.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use thiserror::Error;
use tracing::{debug, instrument};

use crate::branch_evaluation::BranchDisposition;
use crate::cli::prototype1_state::cli_facing::Prototype1BranchEvaluationReport;
use crate::intervention::{
    CommitError, CommitPhase, Configuration, Intervention, Outcome, Prototype1NodeRecord,
    Prototype1RunnerDisposition, Prototype1RunnerResult, RecordStore, Surface,
};

use super::c3::C4;
use super::channel::{Channel, Cursor, FileTransport, ToParent};
use super::event::{
    ChildRuntimeLifecycle, ObservedChildTerminal, Paths, RecordedAt, Refs, TransitionId, World,
};
use super::invocation::{channel_root, result_path};
use super::journal::{CompletionEntry, JournalEntry, ObservedChildResult, PrototypeJournal};
use super::observe;

const RESULT_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Report {
    pub overall_disposition: BranchDisposition,
}

#[derive(Debug)]
pub(crate) struct SuccessfulObservation {
    pub runner_result: Prototype1RunnerResult,
    pub evaluation: Prototype1BranchEvaluationReport,
}

#[derive(Debug)]
pub(crate) struct FailedObservation {
    pub runner_result: Prototype1RunnerResult,
}

/// Explicit observed child family after `C4`.
#[derive(Debug)]
pub(crate) enum ObservedChild {
    Succeeded(SuccessfulObservation),
    Failed(FailedObservation),
}

/// `C5`: parent has observed one terminal child state and reduced it to a
/// policy-facing report without selecting or promoting anything yet.
#[derive(Debug)]
pub(crate) struct C5 {
    pub base: C4,
    pub report: Report,
    pub observed: ObservedChild,
}

impl Configuration for C5 {
    type ArtifactState = <C4 as Configuration>::ArtifactState;
    type BinaryState = <C4 as Configuration>::BinaryState;

    fn artifact_state(&self) -> &Self::ArtifactState {
        self.base.artifact_state()
    }

    fn binary_state(&self) -> &Self::BinaryState {
        self.base.binary_state()
    }
}

fn completion_entry(
    config: &C4,
    node: &Prototype1NodeRecord,
    transition_id: TransitionId,
    phase: CommitPhase,
    result: Option<ObservedChildResult>,
) -> CompletionEntry {
    let runtime_id = config
        .binary
        .child_runtime
        .expect("C4 must carry a concrete child runtime");
    let child_lifecycle = if result.is_some() {
        ChildRuntimeLifecycle::Terminated
    } else {
        ChildRuntimeLifecycle::Acknowledged
    };
    CompletionEntry {
        transition_id,
        runtime_id,
        phase,
        recorded_at: RecordedAt::now(),
        generation: node.generation,
        refs: Refs {
            campaign_id: config.campaign_id.clone(),
            node_id: node.node_id.clone(),
            instance_id: node.instance_id.clone(),
            source_state_id: node.source_state_id.clone(),
            branch_id: node.branch_id.clone(),
            candidate_id: node.candidate_id.clone(),
            branch_label: config.resolved.branch.branch_label.clone(),
            spec_id: config.resolved.branch.synthesized_spec_id.clone(),
        },
        paths: Paths {
            repo_root: config.artifact.repo_root.clone(),
            workspace_root: node.workspace_root.clone(),
            binary_path: node.binary_path.clone(),
            target_relpath: node.target_relpath.clone(),
            absolute_path: config.artifact.repo_root.join(&node.target_relpath),
        },
        world: World {
            node_status: node.status,
            running_binary: config.binary.parent_running,
            running_lineage: super::event::LineageMark::Parent,
            artifact_lineage: super::event::LineageMark::Child,
            child_lifecycle: Some(child_lifecycle),
        },
        child_lifecycle,
        runner_result_path: result_path(&node.node_dir, runtime_id),
        result,
    }
}

/// Typed failure for the `C4 -> C5` child-completion observation transition.
#[derive(Debug, Error)]
pub(crate) enum ObserveChildError {
    #[error("C4 is missing a concrete child runtime id")]
    MissingRuntimeId,
    #[error(
        "succeeded runner result for node '{node_id}' did not include an evaluation report payload"
    )]
    MissingEvaluationReport { node_id: String },
    #[error("failed to read child channel: {detail}")]
    ReadChannel { detail: String },
}

/// Surface over the child-to-parent channel used by child-completion observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ChildChannelSurface;

impl Surface<C4> for ChildChannelSurface {
    type Target = ();
    type ReadView = PathBuf;
    type Error = ObserveChildError;

    fn read_view(&self, config: &C4, _: &Self::Target) -> Result<Self::ReadView, Self::Error> {
        let runtime_id = config
            .binary
            .child_runtime
            .ok_or(ObserveChildError::MissingRuntimeId)?;
        Ok(channel_root(&config.node.node_dir, runtime_id))
    }
}

/// Concrete intervention mediating `C4 -> C5`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObserveChild {
    transition_id: TransitionId,
}

impl ObserveChild {
    pub(crate) fn new() -> Self {
        Self {
            transition_id: TransitionId::new(),
        }
    }
}

impl Report {
    fn reject() -> Self {
        Self {
            overall_disposition: BranchDisposition::Reject,
        }
    }
}

fn terminal_from_result(result: &ObservedChildResult) -> ObservedChildTerminal {
    match result {
        ObservedChildResult::Succeeded { .. } => ObservedChildTerminal::Succeeded,
        ObservedChildResult::Failed { .. } => ObservedChildTerminal::Failed,
    }
}

fn base_with_result_status(mut base: C4, runner_result: &Prototype1RunnerResult) -> C4 {
    base.node.status = runner_result.status;
    base
}

#[derive(Debug)]
struct ChildResultPayload {
    runner_result: Prototype1RunnerResult,
    evaluation: Option<Prototype1BranchEvaluationReport>,
}

fn child_result_from_channel(
    channel: &Channel<C4, FileTransport>,
    cursor: Cursor,
) -> Result<(Cursor, Option<ChildResultPayload>), ObserveChildError> {
    let (cursor, messages) =
        channel
            .recv_from_child(cursor)
            .map_err(|source| ObserveChildError::ReadChannel {
                detail: format!("{source:?}"),
            })?;
    let result = messages
        .into_iter()
        .find_map(|message| match message.body() {
            ToParent::Result {
                runner_result,
                evaluation,
            } => Some(ChildResultPayload {
                runner_result: runner_result.clone(),
                evaluation: evaluation.clone(),
            }),
            _ => None,
        });
    Ok((cursor, result))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rejected {}

impl Intervention<C4, C5> for ObserveChild {
    type Surface = ChildChannelSurface;
    type Journal = PrototypeJournal;
    type Error = ObserveChildError;
    type Rejected = Rejected;

    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self, records),
        fields(node_id = %from.node.node_id, branch_id = %from.resolved.branch.branch_id, runtime_id = ?from.binary.child_runtime)
    )]
    fn transition(
        &self,
        from: C4,
        records: &mut Self::Journal,
    ) -> Result<
        Outcome<C5, Self::Rejected>,
        CommitError<Self::Error, <Self::Journal as RecordStore>::Error>,
    > {
        let runtime_id = from
            .binary
            .child_runtime
            .ok_or(CommitError::Transition(ObserveChildError::MissingRuntimeId))?;
        let channel_dir = ChildChannelSurface
            .read_view(&from, &())
            .map_err(CommitError::Transition)?;
        let mut wait = Some(observe::Step::start(observe::span!(
            "prototype1.child.observe.wait_for_result",
            transition_id = ?self.transition_id,
            campaign_id = %from.campaign_id,
            node_id = %from.node.node_id,
            generation = from.node.generation,
            runtime_id = %runtime_id,
            channel_dir = %channel_dir.display(),
        )));

        records
            .append(JournalEntry::ObserveChild(completion_entry(
                &from,
                &from.node,
                self.transition_id,
                CommitPhase::Before,
                None,
            )))
            .map_err(|source| CommitError::Record {
                phase: CommitPhase::Before,
                source,
            })?;
        debug!(
            target: ploke_core::EXECUTION_DEBUG_TARGET,
            node_id = %from.node.node_id,
            runtime_id = %runtime_id,
            channel_dir = %channel_dir.display(),
            "recorded completion before entry"
        );

        let parent_channel = Channel::for_role(
            &from,
            super::channel::Endpoints::new(
                channel_dir,
                from.campaign_id.clone(),
                from.node.node_id.clone(),
                runtime_id,
            ),
            FileTransport,
        );
        let mut channel_cursor = Cursor::start();
        loop {
            let observed = child_result_from_channel(&parent_channel, channel_cursor)
                .map_err(CommitError::Transition)?;
            channel_cursor = observed.0;
            if let Some(observed) = observed.1 {
                if let Some(wait) = wait.take() {
                    wait.success();
                }
                let ChildResultPayload {
                    runner_result,
                    evaluation,
                } = observed;
                let runner_outcome = observe::Step::start(observe::span!(
                    "prototype1.child.observe.channel_result",
                    transition_id = ?self.transition_id,
                    campaign_id = %from.campaign_id,
                    node_id = %from.node.node_id,
                    generation = from.node.generation,
                    runtime_id = %runtime_id,
                    runner_disposition = ?runner_result.disposition,
                    evaluation_included = evaluation.is_some(),
                ));
                if runner_result.disposition == Prototype1RunnerDisposition::Succeeded {
                    runner_outcome.success();
                } else {
                    runner_outcome.rejected();
                }

                if runner_result.disposition != Prototype1RunnerDisposition::Succeeded {
                    let result = ObservedChildResult::Failed {
                        disposition: runner_result.disposition,
                        detail: runner_result.detail.clone(),
                        exit_code: runner_result.exit_code,
                    };
                    let base = base_with_result_status(from, &runner_result);
                    let next = C5 {
                        base,
                        report: Report::reject(),
                        observed: ObservedChild::Failed(FailedObservation { runner_result }),
                    };
                    records
                        .append(JournalEntry::ObserveChild(completion_entry(
                            &next.base,
                            &next.base.node,
                            self.transition_id,
                            CommitPhase::After,
                            Some(result.clone()),
                        )))
                        .map_err(|source| CommitError::Record {
                            phase: CommitPhase::After,
                            source,
                        })?;
                    debug!(
                        target: ploke_core::EXECUTION_DEBUG_TARGET,
                        node_id = %next.base.node.node_id,
                        runtime_id = %runtime_id,
                        terminal = ?terminal_from_result(&result),
                        "observed failed child runner result"
                    );
                    return Ok(Outcome::Advanced(next));
                }

                let report = evaluation.ok_or_else(|| {
                    CommitError::Transition(ObserveChildError::MissingEvaluationReport {
                        node_id: from.node.node_id.clone(),
                    })
                })?;
                observe::Step::start(observe::span!(
                    "prototype1.child.observe.evaluation_payload",
                    transition_id = ?self.transition_id,
                    campaign_id = %from.campaign_id,
                    node_id = %from.node.node_id,
                    generation = from.node.generation,
                    runtime_id = %runtime_id,
                    evaluation_artifact_path = %report.evaluation_artifact_path.display(),
                    branch_disposition = ?report.overall_disposition,
                ))
                .success();
                let base = base_with_result_status(from, &runner_result);
                let next = C5 {
                    base,
                    report: Report {
                        overall_disposition: report.overall_disposition.clone(),
                    },
                    observed: ObservedChild::Succeeded(SuccessfulObservation {
                        runner_result,
                        evaluation: report.clone(),
                    }),
                };
                let result = ObservedChildResult::Succeeded {
                    evaluation_artifact_path: report.evaluation_artifact_path.clone(),
                    overall_disposition: report.overall_disposition.clone(),
                };

                records
                    .append(JournalEntry::ObserveChild(completion_entry(
                        &next.base,
                        &next.base.node,
                        self.transition_id,
                        CommitPhase::After,
                        Some(result.clone()),
                    )))
                    .map_err(|source| CommitError::Record {
                        phase: CommitPhase::After,
                        source,
                    })?;
                debug!(
                    target: ploke_core::EXECUTION_DEBUG_TARGET,
                    node_id = %next.base.node.node_id,
                    runtime_id = %runtime_id,
                    disposition = ?next.report.overall_disposition,
                    "observed successful child evaluation"
                );
                return Ok(Outcome::Advanced(next));
            }

            thread::sleep(RESULT_POLL);
        }
    }
}
