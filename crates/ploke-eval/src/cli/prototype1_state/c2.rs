#![allow(dead_code)] // REMOVE BY 2026-04-26: typed C2 -> C3 scaffold is not wired into the live controller yet

//! Explicit `C2 -> C3` prototype configuration transition.
//!
//! Temporary note:
//! This file mirrors the current direct `cargo check` / `cargo build` child
//! binary path from the existing prototype process helper. It does not yet
//! replace that runtime path. The forward transition and replay vocabulary are
//! both present here, but the live controller does not yet consume them.
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

use std::fs;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Output};

use thiserror::Error;

use crate::intervention::{
    CommitError, CommitPhase, Intervention, Outcome, Prototype1NodeStatus, RecordStore, Surface,
    load_node_record, resolve_treatment_branch, update_node_status,
};
use crate::spec::PrepareError;

use super::c1::{
    Artifact, Binary, C2, Child, ChildAckState, ChildBinaryState, Parent, Present, Prototype,
    Unacknowledged,
};
use super::event::{ContentHash, Hashes, Paths, RecordedAt, Refs, TransitionId, World};
use super::journal::{BuildEntry, BuildResult, FailureInfo, JournalEntry, PrototypeJournal};

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
            workspace_root: config.node.workspace_root.clone(),
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
            child_binary_present: ChildState::PRESENT,
            child_running: AckState::ACKNOWLEDGED,
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

impl C2 {
    /// Load and validate a staged `C2` state for one node.
    pub(crate) fn load(
        campaign_id: impl Into<String>,
        campaign_manifest_path: impl Into<PathBuf>,
        node_id: &str,
        repo_root: impl Into<PathBuf>,
    ) -> Result<Self, BuildChildError> {
        let campaign_id = campaign_id.into();
        let campaign_manifest_path = campaign_manifest_path.into();
        let repo_root = repo_root.into();

        let node = load_node_record(&campaign_manifest_path, node_id).map_err(|source| {
            BuildChildError::LoadNode {
                node_id: node_id.to_string(),
                source,
            }
        })?;
        if node.status != Prototype1NodeStatus::WorkspaceStaged {
            return Err(BuildChildError::UnexpectedNodeStatus {
                node_id: node_id.to_string(),
                observed: node.status,
            });
        }
        let resolved =
            resolve_treatment_branch(&campaign_id, &campaign_manifest_path, &node.branch_id)
                .map_err(|source| BuildChildError::ResolveBranch {
                    node_id: node_id.to_string(),
                    branch_id: node.branch_id.clone(),
                    source,
                })?;

        let absolute_path = repo_root.join(&resolved.target_relpath);
        let child_path = node.binary_path.clone();
        let current =
            fs::read_to_string(&absolute_path).map_err(|source| BuildChildError::ReadTarget {
                path: absolute_path.clone(),
                source,
            })?;
        let observed_hash = ContentHash::of(&current);
        if current != resolved.branch.proposed_content {
            return Err(BuildChildError::ArtifactNotMaterialized {
                path: absolute_path,
                expected_proposed_hash: ContentHash(resolved.branch.proposed_content_hash.clone()),
                observed_hash,
            });
        }
        if child_path.exists() {
            return Err(BuildChildError::ChildBinaryAlreadyPresent { path: child_path });
        }

        let target_relpath = resolved.target_relpath.clone();
        let source_content_hash = ContentHash(resolved.source_content_hash.clone());
        let proposed_content_hash = ContentHash(resolved.branch.proposed_content_hash.clone());

        Ok(Self {
            campaign_id,
            campaign_manifest_path,
            node,
            resolved,
            artifact: Artifact {
                repo_root,
                target_relpath,
                source_content_hash,
                current_content_hash: proposed_content_hash.clone(),
                proposed_content_hash,
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

        let check = ProcessCommand::new("cargo")
            .arg("check")
            .arg("-p")
            .arg("ploke-eval")
            .arg("--bin")
            .arg("ploke-eval")
            .env("CARGO_TARGET_DIR", &scratch_dir)
            .current_dir(&repo_root)
            .output()
            .map_err(|source| CommitError::Transition(BuildChildError::CheckInvoke { source }))?;

        if !check.status.success() {
            let rejected = Rejected::CheckFailed(failure(&check));
            let (_, node) = update_node_status(
                &from.campaign_id,
                &from.campaign_manifest_path,
                &from.node.node_id,
                Prototype1NodeStatus::Failed,
            )
            .map_err(|source| {
                CommitError::Transition(BuildChildError::UpdateNodeStatus {
                    node_id: from.node.node_id.clone(),
                    source,
                })
            })?;
            let failed = Prototype {
                campaign_id: from.campaign_id,
                campaign_manifest_path: from.campaign_manifest_path,
                node,
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

        let build = ProcessCommand::new("cargo")
            .arg("build")
            .arg("-p")
            .arg("ploke-eval")
            .arg("--bin")
            .arg("ploke-eval")
            .env("CARGO_TARGET_DIR", &scratch_dir)
            .current_dir(&repo_root)
            .output()
            .map_err(|source| CommitError::Transition(BuildChildError::BuildInvoke { source }))?;

        if !build.status.success() {
            let rejected = Rejected::BuildFailed(failure(&build));
            let (_, node) = update_node_status(
                &from.campaign_id,
                &from.campaign_manifest_path,
                &from.node.node_id,
                Prototype1NodeStatus::Failed,
            )
            .map_err(|source| {
                CommitError::Transition(BuildChildError::UpdateNodeStatus {
                    node_id: from.node.node_id.clone(),
                    source,
                })
            })?;
            let failed = Prototype {
                campaign_id: from.campaign_id,
                campaign_manifest_path: from.campaign_manifest_path,
                node,
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

        fs::copy(&built_binary, &from.binary.child_path).map_err(|source| {
            CommitError::Transition(BuildChildError::PromoteBinary {
                path: from.binary.child_path.clone(),
                source,
            })
        })?;
        let (_, node) = update_node_status(
            &from.campaign_id,
            &from.campaign_manifest_path,
            &from.node.node_id,
            Prototype1NodeStatus::BinaryBuilt,
        )
        .map_err(|source| {
            CommitError::Transition(BuildChildError::UpdateNodeStatus {
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

        Ok(Outcome::Advanced(next))
    }
}
