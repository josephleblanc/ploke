#![allow(dead_code)] // REMOVE BY 2026-04-26: typed C4 -> C5 scaffold is not wired into the live controller yet

//! Explicit parent-side observation of child completion after `C4`.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;
use tracing::{debug, instrument};

use crate::cli::prototype1_state::cli_facing::Prototype1TreatmentEvidence;
use crate::intervention::{
    CommitError, CommitPhase, Configuration, Intervention, Outcome, Prototype1NodeRecord,
    Prototype1RunnerDisposition, Prototype1RunnerResult, RecordStore, Surface,
};

use super::c3::C4;
use super::channel::{Channel, Cursor, FileTransport, ToParent};
use super::event::{
    ChildRuntimeLifecycle, ObservedChildTerminal, Paths, RecordedAt, Refs, RuntimeId, TransitionId,
    World,
};
use super::invocation::{channel_root, result_path};
use super::journal::{CompletionEntry, JournalEntry, ObservedChildResult, PrototypeJournal};
use super::observe;

const RESULT_POLL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(crate) struct SuccessfulObservation {
    pub runner_result: Prototype1RunnerResult,
    pub treatment: Prototype1TreatmentEvidence,
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

/// `C5`: parent has observed one terminal child state. Successful children
/// carry treatment evidence; parent-side comparison happens after this state.
#[derive(Debug)]
pub(crate) struct C5 {
    pub base: C4,
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
    #[error("succeeded runner result for node '{node_id}' did not include treatment evidence")]
    MissingTreatmentEvidence { node_id: String },
    #[error("failed to read child channel: {detail}")]
    ReadChannel { detail: String },
    #[error(
        "timed out after {waited_ms}ms waiting for child result for node '{node_id}' runtime '{runtime_id}' at '{runner_result_path}'"
    )]
    TimedOutWaitingForResult {
        node_id: String,
        runtime_id: RuntimeId,
        waited_ms: u64,
        runner_result_path: PathBuf,
    },
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
    stale_after: Duration,
}

impl ObserveChild {
    pub(crate) fn new(stale_after: Duration) -> Self {
        Self {
            transition_id: TransitionId::new(),
            stale_after,
        }
    }
}

fn terminal_from_result(result: &ObservedChildResult) -> ObservedChildTerminal {
    match result {
        ObservedChildResult::TreatmentComplete { .. } => ObservedChildTerminal::Succeeded,
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
    treatment: Option<Prototype1TreatmentEvidence>,
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
                treatment,
            } => Some(ChildResultPayload {
                runner_result: runner_result.clone(),
                treatment: treatment.clone(),
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
        skip(self, from, records),
        fields(
            phase = "observe_child_result",
            transition = "C4->C5",
            node_id = %from.node.node_id,
            branch_id = %from.resolved.branch.branch_id,
            generation = from.node.generation,
            runtime_id = ?from.binary.child_runtime,
        )
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
        let started = Instant::now();
        let runner_result_path = result_path(&from.node.node_dir, runtime_id);
        loop {
            let observed = child_result_from_channel(&parent_channel, channel_cursor)
                .map_err(CommitError::Transition)?;
            channel_cursor = observed.0;
            let observed = observed.1;
            if let Some(observed) = observed {
                if let Some(wait) = wait.take() {
                    wait.success();
                }
                let ChildResultPayload {
                    runner_result,
                    treatment,
                } = observed;
                let runner_outcome = observe::Step::start(observe::span!(
                    "prototype1.child.observe.channel_result",
                    transition_id = ?self.transition_id,
                    campaign_id = %from.campaign_id,
                    node_id = %from.node.node_id,
                    generation = from.node.generation,
                    runtime_id = %runtime_id,
                    runner_disposition = ?runner_result.disposition,
                    treatment_included = treatment.is_some(),
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

                let treatment = treatment.ok_or_else(|| {
                    CommitError::Transition(ObserveChildError::MissingTreatmentEvidence {
                        node_id: from.node.node_id.clone(),
                    })
                })?;
                observe::Step::start(observe::span!(
                    "prototype1.child.observe.treatment_payload",
                    transition_id = ?self.transition_id,
                    campaign_id = %from.campaign_id,
                    node_id = %from.node.node_id,
                    generation = from.node.generation,
                    runtime_id = %runtime_id,
                    treatment_campaign_id = %treatment.treatment_campaign_id,
                ))
                .success();
                let base = base_with_result_status(from, &runner_result);
                let next = C5 {
                    base,
                    observed: ObservedChild::Succeeded(SuccessfulObservation {
                        runner_result,
                        treatment: treatment.clone(),
                    }),
                };
                let result = ObservedChildResult::TreatmentComplete {
                    treatment_campaign_id: treatment.treatment_campaign_id.clone(),
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
                    treatment_campaign_id = %treatment.treatment_campaign_id,
                    "observed successful child treatment evidence"
                );
                return Ok(Outcome::Advanced(next));
            }

            if started.elapsed() >= self.stale_after {
                if let Some(wait) = wait.take() {
                    wait.timed_out();
                }
                return Err(CommitError::Transition(
                    ObserveChildError::TimedOutWaitingForResult {
                        node_id: from.node.node_id.clone(),
                        runtime_id,
                        waited_ms: started.elapsed().as_millis() as u64,
                        runner_result_path: runner_result_path.clone(),
                    },
                ));
            }

            thread::sleep(RESULT_POLL);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;
    use std::thread;
    use std::time::Duration;

    use tempfile::tempdir;

    use crate::campaign::EvalCampaignPolicy;
    use crate::cli::prototype1_state::c1::{
        Acknowledged, Artifact, Binary, Child as ChildLineage, Parent as ParentLineage, Present,
        Prototype,
    };
    use crate::cli::prototype1_state::child::Child as RuntimeChild;
    use crate::cli::prototype1_state::event::{ContentHash, Paths, Refs, RuntimeId};
    use crate::intervention::{
        Prototype1NodeRecord, Prototype1NodeStatus, Prototype1RunnerDisposition,
        Prototype1RunnerRequest, Prototype1RunnerResult, ResolvedTreatmentBranch,
        TreatmentBranchNode, TreatmentBranchStatus,
    };
    use crate::target_registry::BenchmarkFamily;

    use super::super::channel::{Endpoints, FileTransport};
    use super::super::invocation::{channel_root, result_path};
    use super::super::journal::PrototypeJournal;
    use super::*;

    fn runner_result(runtime: &C4) -> Prototype1RunnerResult {
        Prototype1RunnerResult {
            schema_version: "prototype1-treatment-node.v1".to_string(),
            campaign_id: runtime.campaign_id.clone(),
            node_id: runtime.node.node_id.clone(),
            generation: runtime.node.generation,
            branch_id: runtime.node.branch_id.clone(),
            status: Prototype1NodeStatus::Succeeded,
            disposition: Prototype1RunnerDisposition::Succeeded,
            treatment_campaign_id: Some("treatment-campaign".to_string()),
            evaluation_artifact_path: None,
            detail: None,
            exit_code: Some(0),
            stdout_excerpt: None,
            stderr_excerpt: None,
            recorded_at: "2026-05-25T00:00:00Z".to_string(),
        }
    }

    fn treatment_evidence() -> Prototype1TreatmentEvidence {
        Prototype1TreatmentEvidence {
            baseline_campaign_id: "campaign".to_string(),
            branch_id: "branch".to_string(),
            treatment_campaign_id: "treatment-campaign".to_string(),
            treatment_campaign_manifest: "treatment/campaign.json".into(),
            treatment_closure_state_path: "treatment/closure-state.json".into(),
            eval_policy: EvalCampaignPolicy::default(),
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            dataset_sources: Vec::new(),
            instances: Vec::new(),
        }
    }

    fn failed_runner_result(runtime: &C4) -> Prototype1RunnerResult {
        let mut result = runner_result(runtime);
        result.status = Prototype1NodeStatus::Failed;
        result.disposition = Prototype1RunnerDisposition::TreatmentFailed;
        result.treatment_campaign_id = None;
        result.detail = Some("synthetic treatment failure".to_string());
        result.exit_code = Some(1);
        result
    }

    fn resolved_branch() -> ResolvedTreatmentBranch {
        ResolvedTreatmentBranch {
            instance_id: "instance".to_string(),
            source_state_id: "source".to_string(),
            parent_branch_id: None,
            target_relpath: "src/lib.rs".into(),
            source_content: "fn old() {}\n".to_string(),
            source_content_hash: "source-hash".to_string(),
            selected_branch_id: Some("branch".to_string()),
            branch: TreatmentBranchNode {
                branch_id: "branch".to_string(),
                candidate_id: "candidate".to_string(),
                patch_id: None,
                branch_label: "candidate branch".to_string(),
                synthesized_spec_id: "spec".to_string(),
                proposed_content: "fn new() {}\n".to_string(),
                proposed_content_hash: "proposed-hash".to_string(),
                generation_target: None,
                generation_coordinate: None,
                status: TreatmentBranchStatus::Applied,
                apply_id: None,
                applied_content_hash: None,
                derived_artifact_id: None,
            },
        }
    }

    fn c4(root: &std::path::Path, runtime_id: RuntimeId) -> C4 {
        let node_dir = root.join("node");
        let workspace_root = root.join("workspace");
        let binary_path = node_dir.join("bin/ploke-eval");
        let target_relpath = std::path::PathBuf::from("src/lib.rs");
        let node = Prototype1NodeRecord {
            schema_version: "prototype1-node.v1".to_string(),
            node_id: "node".to_string(),
            parent_node_id: Some("parent".to_string()),
            generation: 1,
            instance_id: "instance".to_string(),
            source_state_id: "source".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: None,
            branch_id: "branch".to_string(),
            candidate_id: "candidate".to_string(),
            target_relpath: target_relpath.clone(),
            node_dir: node_dir.clone(),
            workspace_root: workspace_root.clone(),
            binary_path: binary_path.clone(),
            runner_request_path: node_dir.join("runner-request.json"),
            runner_result_path: node_dir.join("runner-result.json"),
            status: Prototype1NodeStatus::Running,
            created_at: "2026-05-25T00:00:00Z".to_string(),
            updated_at: "2026-05-25T00:00:00Z".to_string(),
        };
        let request = Prototype1RunnerRequest {
            schema_version: "prototype1-runner-request.v1".to_string(),
            campaign_id: "campaign".to_string(),
            node_id: "node".to_string(),
            generation: 1,
            instance_id: "instance".to_string(),
            source_state_id: "source".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            branch_id: "branch".to_string(),
            target_relpath: target_relpath.clone(),
            workspace_root: workspace_root.clone(),
            binary_path: binary_path.clone(),
            stop_on_error: false,
            runner_args: Vec::new(),
        };

        Prototype {
            campaign_id: "campaign".to_string(),
            campaign_manifest_path: root.join("campaign.json"),
            node,
            request,
            resolved: resolved_branch(),
            artifact: Artifact {
                repo_root: workspace_root,
                target_relpath,
                source_content_hash: ContentHash("source-hash".to_string()),
                current_content_hash: ContentHash("proposed-hash".to_string()),
                proposed_content_hash: ContentHash("proposed-hash".to_string()),
                _lineage: PhantomData::<ChildLineage>,
            },
            binary: Binary {
                parent_running: true,
                child_path: binary_path,
                child_runtime: Some(runtime_id),
                _lineage: PhantomData::<ParentLineage>,
                _child: PhantomData::<Present>,
                _ack: PhantomData::<Acknowledged>,
            },
        }
    }

    fn historical_node_150_c4(root: &std::path::Path) -> C4 {
        let runtime_id: RuntimeId = "8d4f99c6-c167-4c02-90d6-2055b174c34e"
            .parse()
            .expect("historical runtime id");
        let mut runtime = c4(root, runtime_id);
        runtime.campaign_id = "p1-gemini35-flash-direct-15g2x3-20260525-035000".to_string();
        runtime.node.node_id = "node-15006265e24b3b9b".to_string();
        runtime.node.generation = 1;
        runtime.node.branch_id = "branch-c56614c6e6a63aa9".to_string();
        runtime.node.candidate_id = "broad-harness-g1-03".to_string();
        runtime.node.status = Prototype1NodeStatus::Running;
        runtime.request.campaign_id = runtime.campaign_id.clone();
        runtime.request.node_id = runtime.node.node_id.clone();
        runtime.request.generation = runtime.node.generation;
        runtime.request.branch_id = runtime.node.branch_id.clone();
        runtime.resolved.selected_branch_id = Some(runtime.node.branch_id.clone());
        runtime.resolved.branch.branch_id = runtime.node.branch_id.clone();
        runtime.resolved.branch.candidate_id = runtime.node.candidate_id.clone();
        runtime.resolved.branch.branch_label =
            "broad harness edit broad-harness-request:node-f4cf695decef97df:r6".to_string();
        runtime.resolved.branch.synthesized_spec_id =
            "prototype1:broad-headless-tui-adapter-v1".to_string();
        runtime
    }

    fn send_terminal_channel_payload_after_delay(
        runtime: &C4,
        runner_result: Prototype1RunnerResult,
        treatment: Option<Prototype1TreatmentEvidence>,
    ) -> thread::JoinHandle<()> {
        let runtime_id = runtime.binary.child_runtime.expect("runtime id");
        let endpoints = Endpoints::new(
            channel_root(&runtime.node.node_dir, runtime_id),
            runtime.campaign_id.clone(),
            runtime.node.node_id.clone(),
            runtime_id,
        );
        let refs = Refs {
            campaign_id: runtime.campaign_id.clone(),
            node_id: runtime.node.node_id.clone(),
            instance_id: runtime.node.instance_id.clone(),
            source_state_id: runtime.node.source_state_id.clone(),
            branch_id: runtime.node.branch_id.clone(),
            candidate_id: runtime.node.candidate_id.clone(),
            branch_label: runtime.resolved.branch.branch_label.clone(),
            spec_id: runtime.resolved.branch.synthesized_spec_id.clone(),
        };
        let paths = Paths {
            repo_root: runtime.artifact.repo_root.clone(),
            workspace_root: runtime.node.workspace_root.clone(),
            binary_path: runtime.node.binary_path.clone(),
            target_relpath: runtime.node.target_relpath.clone(),
            absolute_path: runtime
                .node
                .workspace_root
                .join(&runtime.node.target_relpath),
        };
        let journal_path = runtime.node.node_dir.join("child-token-journal.jsonl");
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            let child = RuntimeChild::new(journal_path, runtime_id, 1, refs, paths, 100)
                .ready()
                .expect("child ready")
                .evaluating()
                .expect("child evaluating");
            Channel::for_child(&child, endpoints, FileTransport)
                .send_terminal_result(runner_result, treatment)
                .expect("send terminal result");
        })
    }

    fn send_terminal_channel_result_after_delay(
        runtime: &C4,
        runner_result: Prototype1RunnerResult,
        treatment: Prototype1TreatmentEvidence,
    ) -> thread::JoinHandle<()> {
        send_terminal_channel_payload_after_delay(runtime, runner_result, Some(treatment))
    }

    fn send_result_written_projection(runtime: &C4, runner_result_path: PathBuf) {
        let runtime_id = runtime.binary.child_runtime.expect("runtime id");
        let endpoints = Endpoints::new(
            channel_root(&runtime.node.node_dir, runtime_id),
            runtime.campaign_id.clone(),
            runtime.node.node_id.clone(),
            runtime_id,
        );
        let refs = Refs {
            campaign_id: runtime.campaign_id.clone(),
            node_id: runtime.node.node_id.clone(),
            instance_id: runtime.node.instance_id.clone(),
            source_state_id: runtime.node.source_state_id.clone(),
            branch_id: runtime.node.branch_id.clone(),
            candidate_id: runtime.node.candidate_id.clone(),
            branch_label: runtime.resolved.branch.branch_label.clone(),
            spec_id: runtime.resolved.branch.synthesized_spec_id.clone(),
        };
        let paths = Paths {
            repo_root: runtime.artifact.repo_root.clone(),
            workspace_root: runtime.node.workspace_root.clone(),
            binary_path: runtime.node.binary_path.clone(),
            target_relpath: runtime.node.target_relpath.clone(),
            absolute_path: runtime
                .node
                .workspace_root
                .join(&runtime.node.target_relpath),
        };
        let child = RuntimeChild::new(
            runtime.node.node_dir.join("child-token-journal.jsonl"),
            runtime_id,
            1,
            refs,
            paths,
            100,
        )
        .ready()
        .expect("child ready")
        .evaluating()
        .expect("child evaluating");
        Channel::for_child(&child, endpoints, FileTransport)
            .send_result_written(runner_result_path)
            .expect("send result-written projection");
    }

    #[test]
    fn observe_child_times_out_on_success_sidecar_without_channel_result() {
        let temp = tempdir().expect("tempdir");
        let runtime_id = RuntimeId::new();
        let runtime = c4(temp.path(), runtime_id);
        let runner_result = runner_result(&runtime);
        let runner_result_path = result_path(&runtime.node.node_dir, runtime_id);
        std::fs::create_dir_all(runner_result_path.parent().expect("result parent"))
            .expect("create results dir");
        std::fs::write(
            &runner_result_path,
            serde_json::to_string(&runner_result).expect("serialize runner result"),
        )
        .expect("write sidecar runner result");

        let mut journal = PrototypeJournal::new(temp.path().join("transition-journal.jsonl"));
        let err = ObserveChild::new(Duration::from_millis(1))
            .transition(runtime, &mut journal)
            .expect_err("sidecar success projection must not advance C4");

        assert!(matches!(
            err,
            CommitError::Transition(ObserveChildError::TimedOutWaitingForResult { .. })
        ));
    }

    #[test]
    fn observe_child_ignores_result_written_projection_for_success() {
        let temp = tempdir().expect("tempdir");
        let runtime_id = RuntimeId::new();
        let runtime = c4(temp.path(), runtime_id);
        let runner_result = runner_result(&runtime);
        let runner_result_path = result_path(&runtime.node.node_dir, runtime_id);
        std::fs::create_dir_all(runner_result_path.parent().expect("result parent"))
            .expect("create results dir");
        std::fs::write(
            &runner_result_path,
            serde_json::to_string(&runner_result).expect("serialize runner result"),
        )
        .expect("write sidecar runner result");
        send_result_written_projection(&runtime, runner_result_path);

        let mut journal = PrototypeJournal::new(temp.path().join("transition-journal.jsonl"));
        let err = ObserveChild::new(Duration::from_millis(1))
            .transition(runtime, &mut journal)
            .expect_err("ResultWritten projection must not advance C4");

        assert!(matches!(
            err,
            CommitError::Transition(ObserveChildError::TimedOutWaitingForResult { .. })
        ));
    }

    #[test]
    fn observe_child_rejects_success_channel_result_without_treatment() {
        let temp = tempdir().expect("tempdir");
        let runtime_id = RuntimeId::new();
        let runtime = c4(temp.path(), runtime_id);
        let runner_result = runner_result(&runtime);
        let sender = send_terminal_channel_payload_after_delay(&runtime, runner_result, None);

        let mut journal = PrototypeJournal::new(temp.path().join("transition-journal.jsonl"));
        let err = ObserveChild::new(Duration::from_secs(2))
            .transition(runtime, &mut journal)
            .expect_err("successful channel result without treatment must not advance C4");

        sender.join().expect("sender thread");
        assert!(matches!(
            err,
            CommitError::Transition(ObserveChildError::MissingTreatmentEvidence { .. })
        ));
    }

    #[test]
    fn observe_child_advances_failed_channel_result_without_treatment() {
        let temp = tempdir().expect("tempdir");
        let runtime_id = RuntimeId::new();
        let runtime = c4(temp.path(), runtime_id);
        let runner_result = failed_runner_result(&runtime);
        let sender = send_terminal_channel_payload_after_delay(&runtime, runner_result, None);

        let mut journal = PrototypeJournal::new(temp.path().join("transition-journal.jsonl"));
        let outcome = ObserveChild::new(Duration::from_secs(2))
            .transition(runtime, &mut journal)
            .expect("failed channel result should advance C4 as failed");

        sender.join().expect("sender thread");
        let Outcome::Advanced(next) = outcome;
        assert!(matches!(next.observed, ObservedChild::Failed(_)));
    }

    #[test]
    fn observe_child_waits_for_treatment_channel_result_after_success_sidecar() {
        let temp = tempdir().expect("tempdir");
        let runtime_id = RuntimeId::new();
        let runtime = c4(temp.path(), runtime_id);
        let runner_result = runner_result(&runtime);
        let runner_result_path = result_path(&runtime.node.node_dir, runtime_id);
        std::fs::create_dir_all(runner_result_path.parent().expect("result parent"))
            .expect("create results dir");
        std::fs::write(
            &runner_result_path,
            serde_json::to_string(&runner_result).expect("serialize runner result"),
        )
        .expect("write sidecar runner result");

        let sender =
            send_terminal_channel_result_after_delay(&runtime, runner_result, treatment_evidence());
        let mut journal = PrototypeJournal::new(temp.path().join("transition-journal.jsonl"));
        let outcome = ObserveChild::new(Duration::from_secs(2))
            .transition(runtime, &mut journal)
            .expect("observe child");

        sender.join().expect("sender thread");
        let Outcome::Advanced(next) = outcome;
        assert!(matches!(next.observed, ObservedChild::Succeeded(_)));
    }

    #[test]
    fn observe_child_replays_node_150_success_sidecar_then_historical_treatment_channel() {
        let temp = tempdir().expect("tempdir");
        let runtime = historical_node_150_c4(temp.path());
        let runtime_id = runtime.binary.child_runtime.expect("runtime id");
        let runner_result_path = result_path(&runtime.node.node_dir, runtime_id);
        std::fs::create_dir_all(runner_result_path.parent().expect("result parent"))
            .expect("create results dir");
        std::fs::write(
            &runner_result_path,
            include_str!("../../tests/fixtures/prototype1-node-150-handoff/runner-result.json"),
        )
        .expect("write historical sidecar runner result");

        let channel_path =
            channel_root(&runtime.node.node_dir, runtime_id).join("child-to-parent.jsonl");
        let sender = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            std::fs::create_dir_all(channel_path.parent().expect("channel parent"))
                .expect("create channel dir");
            std::fs::write(
                &channel_path,
                include_str!(
                    "../../tests/fixtures/prototype1-node-150-handoff/child-to-parent.jsonl"
                ),
            )
            .expect("write historical terminal channel result");
        });

        let mut journal = PrototypeJournal::new(temp.path().join("transition-journal.jsonl"));
        let outcome = ObserveChild::new(Duration::from_secs(2))
            .transition(runtime, &mut journal)
            .expect("observe historical child");

        sender.join().expect("sender thread");
        let Outcome::Advanced(next) = outcome;
        let ObservedChild::Succeeded(success) = next.observed else {
            panic!("historical child should be observed as succeeded");
        };
        assert_eq!(
            success.treatment.treatment_campaign_id,
            "p1-gemini35-flash-direct-15g2x3-20260525-035000-treatment-branch-c56614c6e6a63aa9-1779711014414"
        );
    }
}
