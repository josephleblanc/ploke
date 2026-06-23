#![allow(dead_code)]
// C2 -> C3 is used by run_planned_child; some replay/test helpers remain intentionally unused.

// ANCHOR: prototype1_c2_to_c3
//! Explicit `C2 -> C3` prototype configuration transition.
//!
//! Temporary note:
//! This file mirrors the current direct `cargo check` / `cargo build` child
//! binary path from the existing prototype process helper. The forward
//! transition and replay vocabulary are both present here; the live child path
//! uses this typed carrier while other recovery/debug paths may reconstruct it.
//!
//! `C2` is the staged parent state:
//! - the parent process is still running
//! - the artifact world is already materialized to the child lineage
//! - no promoted child binary exists yet
//!
//! `C3` is the built parent state:
//! - the parent process is still running
//! - the artifact world remains child-aligned
//! - a promoted child binary now exists, but is not running yet
// ANCHOR_END: prototype1_c2_to_c3

use crate::prelude::*;

use std::process::{Command as ProcessCommand, Output};

use tracing::{debug, instrument, warn};

use crate::intervention::{
    CommitError, CommitPhase, Intervention, Outcome, Prototype1NodeStatus, RecordStore, Surface,
    project_node_status, write_parent_node_projection,
};

use super::c1::{
    Binary, C2, Child, ChildAckState, ChildBinaryState, Parent, Present, Prototype, Unacknowledged,
};
use super::eval_store;
use super::event::{ContentHash, Hashes, Paths, RecordedAt, Refs, TransitionId, World};
use super::journal::{BuildEntry, BuildResult, FailureInfo, JournalEntry, PrototypeJournal};
use super::observe;

/// `C3`: parent binary over child artifact world with a promoted child binary
/// present but not running.
pub(crate) type C3 = Prototype<Parent, Child, Present, Unacknowledged>;

fn excerpt(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    if text.is_empty() {
        return None;
    }
    let max_chars = 4000usize;
    let excerpt = if text.chars().count() > max_chars {
        let tail = text
            .chars()
            .rev()
            .take(max_chars)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        format!("...[truncated]\n{tail}")
    } else {
        text
    };
    Some(excerpt)
}

fn failure(output: &Output) -> FailureInfo {
    FailureInfo {
        exit_code: output.status.code(),
        stdout_excerpt: excerpt(&output.stdout),
        stderr_excerpt: excerpt(&output.stderr),
    }
}

fn cleanup_scratch_dir(path: &Path) {
    match fs::remove_dir_all(path) {
        Ok(()) => {
            debug!(
                target: ploke_core::EXECUTION_DEBUG_TARGET,
                scratch_dir = %path.display(),
                "removed child build scratch target dir"
            );
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            warn!(
                target: ploke_core::EXECUTION_DEBUG_TARGET,
                scratch_dir = %path.display(),
                error = %source,
                "failed to remove child build scratch target dir"
            );
        }
    }
}

fn entry<ChildState, AckState>(
    config: &Prototype<Parent, Child, ChildState, AckState>,
    transition_id: TransitionId,
    phase: CommitPhase,
    result: Option<BuildResult>,
) -> BuildEntry
where
    ChildState: ChildBinaryState,
    AckState: ChildAckState,
{
    let child_lifecycle = if ChildState::PRESENT {
        if AckState::ACKNOWLEDGED {
            Some(super::event::ChildRuntimeLifecycle::Acknowledged)
        } else {
            Some(super::event::ChildRuntimeLifecycle::Built)
        }
    } else {
        None
    };
    BuildEntry {
        transition_id,
        phase,
        recorded_at: RecordedAt::now(),
        generation: config.node.generation,
        refs: Refs {
            // TODO(2026-04-26): Same ownership boundary as `c1.rs`: these
            // clones are for a durable journal record, not because pervasive
            // clone-heavy local semantics are acceptable. Once the value-like
            // carriers are tightened, revisit this and keep only the clones
            // that are genuinely required to give the journal owned data.
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
        world: World {
            node_status: config.node.status,
            running_binary: config.binary.parent_running,
            running_lineage: super::event::LineageMark::Parent,
            artifact_lineage: super::event::LineageMark::Child,
            child_lifecycle,
        },
        hashes: Hashes {
            // TODO(2026-04-26): Same temporary ownership smell as `c1.rs`:
            // these clones exist because `ContentHash` is still a
            // string-backed journal carrier. Replace that with a fixed-size
            // digest type once this scaffold is wired into the live path.
            source: config.artifact.source_content_hash.clone(),
            current: config.artifact.current_content_hash.clone(),
            proposed: config.artifact.proposed_content_hash.clone(),
        },
        result,
    }
}

/// Typed failure for the `C2 -> C3` build transition.
#[derive(Debug, Error)]
pub(crate) enum BuildChildError {
    #[error("failed to load node record '{node_id}'")]
    LoadNode {
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
    #[error("node '{node_id}' is not workspace_staged: observed '{observed:?}'")]
    UnexpectedNodeStatus {
        node_id: String,
        observed: Prototype1NodeStatus,
    },
    #[error("failed to read target artifact '{path}': {source}")]
    ReadTarget {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("expected staged artifact at '{path}' to match proposed branch content")]
    ArtifactNotMaterialized {
        path: PathBuf,
        expected_proposed_hash: ContentHash,
        observed_hash: ContentHash,
    },
    #[error("child binary already exists at '{path}' before build")]
    ChildBinaryAlreadyPresent { path: PathBuf },
    #[error("failed to create cargo scratch dir '{path}': {source}")]
    CreateScratchDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to create promoted binary dir '{path}': {source}")]
    CreateBinaryDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to invoke cargo check: {source}")]
    CheckInvoke { source: std::io::Error },
    #[error("failed to invoke cargo build: {source}")]
    BuildInvoke { source: std::io::Error },
    #[error("build succeeded but child binary '{path}' was not found")]
    MissingBuiltBinary { path: PathBuf },
    #[error("failed to promote child binary to '{path}': {source}")]
    PromoteBinary {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to update node '{node_id}' status")]
    UpdateNodeStatus {
        node_id: String,
        #[source]
        source: PrepareError,
    },
    #[error("failed to mirror child '{node_id}' build provenance to eval-store")]
    EvalStoreBuildProvenance {
        node_id: String,
        #[source]
        source: eval_store::EvalStoreError,
    },
}

/// Committed non-success result for the `C2 -> C3` build transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rejected {
    CheckFailed(FailureInfo),
    BuildFailed(FailureInfo),
}

impl Rejected {
    fn into_result(self) -> BuildResult {
        match self {
            Self::CheckFailed(info) => BuildResult::CheckFailed(info),
            Self::BuildFailed(info) => BuildResult::BuildFailed(info),
        }
    }
}

/// Surface over the staged repo root used by the child-build transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RepoSurface;

impl Surface<C2> for RepoSurface {
    type Target = ();
    type ReadView = PathBuf;
    type Error = BuildChildError;

    fn read_view(&self, config: &C2, _: &Self::Target) -> Result<Self::ReadView, Self::Error> {
        Ok(config.artifact.repo_root.clone())
    }
}

/// Concrete intervention mediating `C2 -> C3`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildChild {
    transition_id: TransitionId,
}

impl BuildChild {
    pub(crate) fn new() -> Self {
        Self {
            transition_id: TransitionId::new(),
        }
    }
}

impl Intervention<C2, C3> for BuildChild {
    type Surface = RepoSurface;
    type Journal = PrototypeJournal;
    type Error = BuildChildError;
    type Rejected = Rejected;

    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self, from, records),
        fields(
            phase = "build_child_runtime",
            transition = "C2->C3",
            node_id = %from.node.node_id,
            branch_id = %from.resolved.branch.branch_id,
            generation = from.node.generation,
        )
    )]
    fn transition(
        &self,
        from: C2,
        records: &mut Self::Journal,
    ) -> Result<
        Outcome<C3, Self::Rejected>,
        CommitError<Self::Error, <Self::Journal as RecordStore>::Error>,
    > {
        let repo_root = RepoSurface
            .read_view(&from, &())
            .map_err(CommitError::Transition)?;
        let scratch_dir = from.node.node_dir.join("target");
        let built_binary = scratch_dir
            .join("debug")
            .join(format!("ploke-eval{}", std::env::consts::EXE_SUFFIX));
        if from.binary.child_path.exists() {
            return Err(CommitError::Transition(
                BuildChildError::ChildBinaryAlreadyPresent {
                    path: from.binary.child_path.clone(),
                },
            ));
        }

        records
            .append(JournalEntry::BuildChild(entry(
                &from,
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
            branch_id = %from.resolved.branch.branch_id,
            scratch_dir = %scratch_dir.display(),
            "recorded build before entry"
        );

        fs::create_dir_all(&scratch_dir).map_err(|source| {
            CommitError::Transition(BuildChildError::CreateScratchDir {
                path: scratch_dir.clone(),
                source,
            })
        })?;
        if let Some(parent) = from.binary.child_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                CommitError::Transition(BuildChildError::CreateBinaryDir {
                    path: parent.to_path_buf(),
                    source,
                })
            })?;
        }

        let check = observe::command_output(
            observe::span!(
                "prototype1.child.build.cargo_check",
                transition_id = ?self.transition_id,
                campaign_id = %from.campaign_id,
                node_id = %from.node.node_id,
                generation = from.node.generation,
                scratch_dir = %scratch_dir.display(),
                child_binary = %from.binary.child_path.display(),
            ),
            "cargo",
            || {
                ProcessCommand::new("cargo")
                    .arg("check")
                    .arg("-p")
                    .arg("ploke-eval")
                    .arg("--bin")
                    .arg("ploke-eval")
                    .env("CARGO_TARGET_DIR", &scratch_dir)
                    .current_dir(&repo_root)
                    .output()
            },
        )
        .map_err(|source| CommitError::Transition(BuildChildError::CheckInvoke { source }))?;

        if !check.status.success() {
            let rejected = Rejected::CheckFailed(failure(&check));
            debug!(
                target: ploke_core::EXECUTION_DEBUG_TARGET,
                node_id = %from.node.node_id,
                branch_id = %from.resolved.branch.branch_id,
                rejected = ?rejected,
                "cargo check rejected child build"
            );
            let node = project_node_status(&from.node, Prototype1NodeStatus::Failed);
            write_parent_node_projection(&from.campaign_id, &node).map_err(|source| {
                CommitError::Transition(BuildChildError::UpdateNodeStatus {
                    node_id: from.node.node_id.clone(),
                    source,
                })
            })?;
            let failed = Prototype {
                campaign_id: from.campaign_id,
                campaign_manifest_path: from.campaign_manifest_path,
                node,
                request: from.request,
                resolved: from.resolved,
                artifact: from.artifact,
                binary: from.binary,
            };
            records
                .append(JournalEntry::BuildChild(entry(
                    &failed,
                    self.transition_id,
                    CommitPhase::After,
                    Some(rejected.clone().into_result()),
                )))
                .map_err(|source| CommitError::Record {
                    phase: CommitPhase::After,
                    source,
                })?;
            return Ok(Outcome::Rejected(rejected));
        }

        let build = observe::command_output(
            observe::span!(
                "prototype1.child.build.cargo_build",
                transition_id = ?self.transition_id,
                campaign_id = %from.campaign_id,
                node_id = %from.node.node_id,
                generation = from.node.generation,
                scratch_dir = %scratch_dir.display(),
                child_binary = %from.binary.child_path.display(),
            ),
            "cargo",
            || {
                ProcessCommand::new("cargo")
                    .arg("build")
                    .arg("-p")
                    .arg("ploke-eval")
                    .arg("--bin")
                    .arg("ploke-eval")
                    .env("CARGO_TARGET_DIR", &scratch_dir)
                    .current_dir(&repo_root)
                    .output()
            },
        )
        .map_err(|source| CommitError::Transition(BuildChildError::BuildInvoke { source }))?;

        if !build.status.success() {
            let rejected = Rejected::BuildFailed(failure(&build));
            debug!(
                target: ploke_core::EXECUTION_DEBUG_TARGET,
                node_id = %from.node.node_id,
                branch_id = %from.resolved.branch.branch_id,
                rejected = ?rejected,
                "cargo build rejected child build"
            );
            let node = project_node_status(&from.node, Prototype1NodeStatus::Failed);
            write_parent_node_projection(&from.campaign_id, &node).map_err(|source| {
                CommitError::Transition(BuildChildError::UpdateNodeStatus {
                    node_id: from.node.node_id.clone(),
                    source,
                })
            })?;
            let failed = Prototype {
                campaign_id: from.campaign_id,
                campaign_manifest_path: from.campaign_manifest_path,
                node,
                request: from.request,
                resolved: from.resolved,
                artifact: from.artifact,
                binary: from.binary,
            };
            records
                .append(JournalEntry::BuildChild(entry(
                    &failed,
                    self.transition_id,
                    CommitPhase::After,
                    Some(rejected.clone().into_result()),
                )))
                .map_err(|source| CommitError::Record {
                    phase: CommitPhase::After,
                    source,
                })?;
            return Ok(Outcome::Rejected(rejected));
        }

        if !built_binary.exists() {
            return Err(CommitError::Transition(
                BuildChildError::MissingBuiltBinary { path: built_binary },
            ));
        }

        observe::io_result(
            observe::span!(
                "prototype1.child.build.promote_binary",
                transition_id = ?self.transition_id,
                campaign_id = %from.campaign_id,
                node_id = %from.node.node_id,
                generation = from.node.generation,
                source_binary = %built_binary.display(),
                child_binary = %from.binary.child_path.display(),
            ),
            "promote_binary",
            || fs::copy(&built_binary, &from.binary.child_path),
        )
        .map_err(|source| {
            CommitError::Transition(BuildChildError::PromoteBinary {
                path: from.binary.child_path.clone(),
                source,
            })
        })?;
        cleanup_scratch_dir(&scratch_dir);
        let node = project_node_status(&from.node, Prototype1NodeStatus::BinaryBuilt);
        write_parent_node_projection(&from.campaign_id, &node).map_err(|source| {
            CommitError::Transition(BuildChildError::UpdateNodeStatus {
                node_id: from.node.node_id.clone(),
                source,
            })
        })?;

        let next = Prototype {
            campaign_id: from.campaign_id,
            campaign_manifest_path: from.campaign_manifest_path,
            node,
            request: from.request,
            resolved: from.resolved,
            artifact: from.artifact,
            binary: Binary {
                parent_running: true,
                child_path: from.binary.child_path,
                child_runtime: None,
                _lineage: std::marker::PhantomData,
                _child: std::marker::PhantomData,
                _ack: std::marker::PhantomData,
            },
        };

        records
            .append(JournalEntry::BuildChild(entry(
                &next,
                self.transition_id,
                CommitPhase::After,
                Some(BuildResult::Built),
            )))
            .map_err(|source| CommitError::Record {
                phase: CommitPhase::After,
                source,
            })?;
        debug!(
            target: ploke_core::EXECUTION_DEBUG_TARGET,
            node_id = %next.node.node_id,
            branch_id = %next.resolved.branch.branch_id,
            binary_path = %next.binary.child_path.display(),
            "recorded build after entry"
        );
        mirror_child_build_provenance(&next).map_err(|source| {
            CommitError::Transition(BuildChildError::EvalStoreBuildProvenance {
                node_id: next.node.node_id.clone(),
                source,
            })
        })?;

        Ok(Outcome::Advanced(next))
    }
}

fn mirror_child_build_provenance(next: &C3) -> Result<(), eval_store::EvalStoreError> {
    let db_path = eval_store::prototype1_eval_store_db_path(&next.campaign_manifest_path);
    if !db_path.exists() {
        return Ok(());
    }

    let artifact_id = next
        .node
        .derived_artifact_id
        .as_ref()
        .map(|id| id.to_string());
    let binary_path = next.binary.child_path.clone();
    let binary_hash = eval_store::file_sha256(&binary_path)?;
    let recorded_at = chrono::Utc::now().to_rfc3339();
    eval_store::write_build_provenance_to_owner_db(
        &db_path,
        eval_store::BuildProvenanceEvidence {
            binary_ref: eval_store::BinaryRefEvidence {
                campaign_id: next.campaign_id.clone(),
                artifact_id: artifact_id.clone(),
                built_by: None,
                source_ref: binary_path.display().to_string(),
                content_sha256: Some(binary_hash),
                protocol_digest: None,
                recorded_at: Some(recorded_at.clone()),
            },
            build_event: eval_store::BuildEventEvidence {
                campaign_id: next.campaign_id.clone(),
                node_id: next.node.node_id.clone(),
                runtime_id: None,
                artifact_id,
                phase: "promote".to_string(),
                outcome: "built".to_string(),
                binary_ref: None,
                log_ref: None,
                recorded_at,
            },
        },
    )?;
    Ok(())
}
