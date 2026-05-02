#![allow(dead_code)] // REMOVE BY 2026-04-26: typed C4 -> C5 scaffold is not wired into the live controller yet

//! Explicit parent-side observation of child completion after `C4`.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use thiserror::Error;
use tracing::{debug, instrument};

use crate::branch_evaluation::BranchDisposition;
use crate::cli::prototype1_state::cli_facing::Prototype1BranchEvaluationReport;
use crate::intervention::{
    CommitError, CommitPhase, Configuration, Intervention, Outcome, Prototype1NodeRecord,
    Prototype1RunnerDisposition, Prototype1RunnerResult, RecordStore, Surface, load_node_record,
    load_runner_result_at,
};
use crate::spec::PrepareError;

use super::c3::C4;
use super::event::{
    ChildRuntimeLifecycle, ObservedChildTerminal, Paths, RecordedAt, Refs, RuntimeId, TransitionId,
    World,
};
use super::invocation::result_path;
use super::journal::{
    CompletionEntry, JournalEntry, ObservedChildResult, PrototypeJournal, PrototypeJournalError,
};
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

fn load_report(path: &Path) -> Result<Prototype1BranchEvaluationReport, ObserveChildError> {
    let text =
        fs::read_to_string(path).map_err(|source| ObserveChildError::ReadEvaluationReport {
            path: path.to_path_buf(),
            source,
        })?;
    serde_json::from_str(&text).map_err(|source| ObserveChildError::ParseEvaluationReport {
        path: path.to_path_buf(),
        source,
    })
}

/// Typed failure for the `C4 -> C5` child-completion observation transition.
#[derive(Debug, Error)]
pub(crate) enum ObserveChildError {
    #[error("C4 is missing a concrete child runtime id")]
    MissingRuntimeId,
    #[error("failed to read runner result for node '{node_id}'")]
    LoadRunnerResult {
        node_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("failed to reload node record '{node_id}'")]
    LoadNode {
        node_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("runner result for node '{node_id}' did not include an evaluation artifact path")]
    MissingEvaluationArtifactPath { node_id: String },
    #[error("failed to read evaluation report '{path}': {source}")]
    ReadEvaluationReport {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse evaluation report '{path}': {source}")]
    ParseEvaluationReport {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to read child state journal")]
    ReadJournal {
        #[source]
        source: PrototypeJournalError,
    },
}

/// Surface over the runner result path used by child-completion observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RunnerResultSurface;

impl Surface<C4> for RunnerResultSurface {
    type Target = ();
    type ReadView = PathBuf;
    type Error = ObserveChildError;

    fn read_view(&self, config: &C4, _: &Self::Target) -> Result<Self::ReadView, Self::Error> {
        let runtime_id = config
            .binary
            .child_runtime
            .ok_or(ObserveChildError::MissingRuntimeId)?;
        Ok(result_path(&config.node.node_dir, runtime_id))
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

fn child_result_path(
    records: &PrototypeJournal,
    runtime_id: RuntimeId,
) -> Result<Option<PathBuf>, ObserveChildError> {
    records
        .load_entries()
        .map_err(|source| ObserveChildError::ReadJournal { source })
        .map(|entries| {
            entries.into_iter().rev().find_map(|entry| {
                let JournalEntry::Child(child) = entry else {
                    return None;
                };
                child.result_path(runtime_id)
            })
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rejected {}

impl Intervention<C4, C5> for ObserveChild {
    type Surface = RunnerResultSurface;
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
        let runner_result_path = RunnerResultSurface
            .read_view(&from, &())
            .map_err(CommitError::Transition)?;
        let mut wait = Some(observe::Step::start(observe::span!(
            "prototype1.child.observe.wait_for_result",
            transition_id = ?self.transition_id,
            campaign_id = %from.campaign_id,
            node_id = %from.node.node_id,
            generation = from.node.generation,
            runtime_id = %runtime_id,
            expected_runner_result_path = %runner_result_path.display(),
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
            runner_result_path = %runner_result_path.display(),
            "recorded completion before entry"
        );

        loop {
            if let Some(runner_result_path) =
                child_result_path(records, runtime_id).map_err(CommitError::Transition)?
            {
                if let Some(wait) = wait.take() {
                    wait.success();
                }
                let runner_result =
                    load_runner_result_at(&runner_result_path).map_err(|source| {
                        CommitError::Transition(ObserveChildError::LoadRunnerResult {
                            node_id: from.node.node_id.clone(),
                            source,
                        })
                    })?;
                let runner_outcome = observe::Step::start(observe::span!(
                    "prototype1.child.observe.load_runner_result",
                    transition_id = ?self.transition_id,
                    campaign_id = %from.campaign_id,
                    node_id = %from.node.node_id,
                    generation = from.node.generation,
                    runtime_id = %runtime_id,
                    runner_result_path = %runner_result_path.display(),
                    runner_disposition = ?runner_result.disposition,
                ));
                if runner_result.disposition == Prototype1RunnerDisposition::Succeeded {
                    runner_outcome.success();
                } else {
                    runner_outcome.rejected();
                }
                let node = load_node_record(&from.campaign_manifest_path, &from.node.node_id)
                    .map_err(|source| {
                        CommitError::Transition(ObserveChildError::LoadNode {
                            node_id: from.node.node_id.clone(),
                            source,
                        })
                    })?;

                if runner_result.disposition != Prototype1RunnerDisposition::Succeeded {
                    let result = ObservedChildResult::Failed {
                        disposition: runner_result.disposition,
                        detail: runner_result.detail.clone(),
                        exit_code: runner_result.exit_code,
                    };
                    let next = C5 {
                        base: C4 { node, ..from },
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

                let evaluation_artifact_path =
                    runner_result.evaluation_artifact_path.clone().ok_or(
                        CommitError::Transition(ObserveChildError::MissingEvaluationArtifactPath {
                            node_id: from.node.node_id.clone(),
                        }),
                    )?;
                let report =
                    load_report(&evaluation_artifact_path).map_err(CommitError::Transition)?;
                observe::Step::start(observe::span!(
                    "prototype1.child.observe.load_evaluation_report",
                    transition_id = ?self.transition_id,
                    campaign_id = %from.campaign_id,
                    node_id = %from.node.node_id,
                    generation = from.node.generation,
                    runtime_id = %runtime_id,
                    runner_result_path = %runner_result_path.display(),
                    evaluation_artifact_path = %evaluation_artifact_path.display(),
                    branch_disposition = ?report.overall_disposition,
                ))
                .success();
                let next = C5 {
                    base: C4 { node, ..from },
                    report: Report {
                        overall_disposition: report.overall_disposition.clone(),
                    },
                    observed: ObservedChild::Succeeded(SuccessfulObservation {
                        runner_result,
                        evaluation: report.clone(),
                    }),
                };
                let result = ObservedChildResult::Succeeded {
                    evaluation_artifact_path,
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
