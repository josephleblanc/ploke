#![allow(dead_code)] // REMOVE BY 2026-04-26: typed C1 -> C2 scaffold is not wired into the live controller yet

//! Explicit `C1 -> C2` prototype configuration transition.
//!
//! Temporary note:
//! This file now treats child realization as a backend-mediated workspace
//! operation rather than as an in-place mutation of the parent workspace. Git
//! worktrees are the first backend, but the transition logic should stay
//! independent of that concrete choice.
//!
//! `C1` is the aligned parent state:
//! - the parent process is running
//! - the live artifact world still matches the stored source content
//!
//! `C2` is the diverged parent state:
//! - the same parent process is still running
//! - the child artifact world has been realized in a separate workspace
//!
//! The transition is move-only and the state distinction is carried in the type
//! parameters:
//! - `Prototype<Parent, Parent, Absent, Unacknowledged>` for `C1`
//! - `Prototype<Parent, Child, Absent, Unacknowledged>` for `C2`
//!
//! Consuming `Prototype<Parent, Parent, Absent, Unacknowledged>` is the only
//! way to produce `Prototype<Parent, Child, Absent, Unacknowledged>`, which
//! prevents accidentally re-running the same state value twice.

use std::fs;
use std::marker::PhantomData;
use std::path::PathBuf;

use thiserror::Error;
use tracing::{debug, instrument};

use crate::intervention::{
    CommitError, CommitPhase, Configuration, Intervention, Outcome, Prototype1NodeRecord,
    Prototype1NodeStatus, RecordStore, ResolvedTreatmentBranch, Surface, load_node_record,
    resolve_treatment_branch, update_node_status, update_node_workspace_root,
};
use crate::spec::PrepareError;

use super::backend::{BackendError, GitWorktreeBackend, RealizeRequest, WorkspaceBackend};
use super::event::{
    ContentHash, Hashes, LineageMark, Paths, RecordedAt, Refs, RuntimeId, TransitionId, World,
};
use super::journal::{Entry, JournalEntry, PrototypeJournal};

/// Marker for the currently running parent lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Parent;

/// Marker for the candidate child lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Child;

/// Shared marker family for prototype lineage states.
pub(crate) trait Lineage {
    const MARK: LineageMark;
}

impl Lineage for Parent {
    const MARK: LineageMark = LineageMark::Parent;
}

impl Lineage for Child {
    const MARK: LineageMark = LineageMark::Child;
}

/// Shared marker family for whether the promoted child binary exists.
pub(crate) trait ChildBinaryState {
    const PRESENT: bool;
}

/// Shared marker family for whether the child runtime has acknowledged itself.
pub(crate) trait ChildAckState {
    const ACKNOWLEDGED: bool;
}

/// Marker for a configuration where no promoted child binary exists yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Absent;

/// Marker for a configuration where a promoted child binary exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Present;

impl ChildBinaryState for Absent {
    const PRESENT: bool = false;
}

impl ChildBinaryState for Present {
    const PRESENT: bool = true;
}

/// Marker for a configuration where the child runtime has not yet
/// acknowledged itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unacknowledged;

/// Marker for a configuration where the child runtime has acknowledged
/// itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Acknowledged;

impl ChildAckState for Unacknowledged {
    const ACKNOWLEDGED: bool = false;
}

impl ChildAckState for Acknowledged {
    const ACKNOWLEDGED: bool = true;
}

/// Artifact-bearing payload of a prototype configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Artifact<L: Lineage> {
    /// Repo root containing the live artifact world.
    pub repo_root: PathBuf,
    /// Branch target currently being mediated.
    pub target_relpath: PathBuf,
    /// Stored source hash for the branch target.
    pub source_content_hash: ContentHash,
    /// Observed live content hash at the target.
    pub current_content_hash: ContentHash,
    /// Proposed branch content hash for the target.
    pub proposed_content_hash: ContentHash,
    pub(crate) _lineage: PhantomData<L>,
}

/// Binary-bearing payload of a prototype configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Binary<L: Lineage, ChildState: ChildBinaryState, AckState: ChildAckState> {
    /// Whether the currently active parent process is still running.
    pub parent_running: bool,
    /// Where the promoted child binary belongs once it exists.
    pub child_path: PathBuf,
    /// Which concrete child runtime instance is currently known, if any.
    pub child_runtime: Option<RuntimeId>,
    pub(crate) _lineage: PhantomData<L>,
    pub(crate) _child: PhantomData<ChildState>,
    pub(crate) _ack: PhantomData<AckState>,
}

/// One prototype configuration indexed by running-binary lineage and
/// artifact-world lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Prototype<
    Running: Lineage,
    ArtifactWorld: Lineage,
    ChildState: ChildBinaryState,
    AckState: ChildAckState,
> {
    pub(crate) campaign_id: String,
    pub(crate) campaign_manifest_path: PathBuf,
    pub(crate) node: Prototype1NodeRecord,
    pub(crate) resolved: ResolvedTreatmentBranch,
    pub(crate) artifact: Artifact<ArtifactWorld>,
    pub(crate) binary: Binary<Running, ChildState, AckState>,
}

/// `C1`: parent binary over parent artifact world.
pub(crate) type C1 = Prototype<Parent, Parent, Absent, Unacknowledged>;

/// `C2`: parent binary over child artifact world.
pub(crate) type C2 = Prototype<Parent, Child, Absent, Unacknowledged>;

impl<
    Running: Lineage,
    ArtifactWorld: Lineage,
    ChildState: ChildBinaryState,
    AckState: ChildAckState,
> Configuration for Prototype<Running, ArtifactWorld, ChildState, AckState>
{
    type ArtifactState = Artifact<ArtifactWorld>;
    type BinaryState = Binary<Running, ChildState, AckState>;

    fn artifact_state(&self) -> &Self::ArtifactState {
        &self.artifact
    }

    fn binary_state(&self) -> &Self::BinaryState {
        &self.binary
    }
}

impl<
    Running: Lineage,
    ArtifactWorld: Lineage,
    ChildState: ChildBinaryState,
    AckState: ChildAckState,
> Prototype<Running, ArtifactWorld, ChildState, AckState>
{
    fn entry(&self, transition_id: TransitionId, phase: CommitPhase) -> Entry {
        Entry {
            transition_id,
            phase,
            recorded_at: RecordedAt::now(),
            generation: self.node.generation,
            refs: Refs {
                // TODO(2026-04-26): These string clones are the durable-record
                // ownership boundary, not ideal local working-state semantics.
                // Keep pressure on this: journal construction may need owned
                // text, but we should not let that normalize casual clone use
                // elsewhere. Tighten value-like carriers first (`ContentHash`,
                // then likely some IDs) so only truly record-owned fields clone.
                campaign_id: self.campaign_id.clone(),
                node_id: self.node.node_id.clone(),
                instance_id: self.node.instance_id.clone(),
                source_state_id: self.node.source_state_id.clone(),
                branch_id: self.node.branch_id.clone(),
                candidate_id: self.node.candidate_id.clone(),
                branch_label: self.resolved.branch.branch_label.clone(),
                spec_id: self.resolved.branch.synthesized_spec_id.clone(),
            },
            paths: Paths {
                repo_root: self.artifact.repo_root.clone(),
                workspace_root: self.artifact.repo_root.clone(),
                binary_path: self.node.binary_path.clone(),
                target_relpath: self.artifact.target_relpath.clone(),
                absolute_path: self.artifact.repo_root.join(&self.artifact.target_relpath),
            },
            world: World {
                node_status: self.node.status,
                running_binary: self.binary.parent_running,
                running_lineage: Running::MARK,
                artifact_lineage: ArtifactWorld::MARK,
                child_binary_present: ChildState::PRESENT,
                child_running: AckState::ACKNOWLEDGED,
            },
            hashes: Hashes {
                // TODO(2026-04-26): These clones are downstream of
                // `ContentHash(String)`. Once the journal/event carrier uses a
                // fixed-size digest with value semantics, tighten this up so
                // hash propagation here is copy-like rather than heap-backed.
                source: self.artifact.source_content_hash.clone(),
                current: self.artifact.current_content_hash.clone(),
                proposed: self.artifact.proposed_content_hash.clone(),
            },
        }
    }
}

/// Typed failure for the `C1 -> C2` materialization transition.
#[derive(Debug, Error)]
pub(crate) enum MaterializeBranchError {
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
    #[error("failed to read target artifact '{path}': {source}")]
    ReadTarget {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "expected live artifact at '{path}' to match stored source content before materialization"
    )]
    SourceMismatch {
        path: PathBuf,
        expected_source_hash: ContentHash,
        observed_hash: ContentHash,
    },
    #[error("failed to realize child workspace for branch '{branch_id}'")]
    RealizeWorkspace {
        branch_id: String,
        #[source]
        source: BackendError,
    },
    #[error("failed to update node '{node_id}' status to workspace_staged")]
    UpdateNodeStatus {
        node_id: String,
        #[source]
        source: PrepareError,
    },
}

impl Prototype<Parent, Parent, Absent, Unacknowledged> {
    /// Load and validate an aligned `C1` state for one planned node.
    pub(crate) fn load(
        campaign_id: impl Into<String>,
        campaign_manifest_path: impl Into<PathBuf>,
        node_id: &str,
        repo_root: impl Into<PathBuf>,
    ) -> Result<Self, MaterializeBranchError> {
        let campaign_id = campaign_id.into();
        let campaign_manifest_path = campaign_manifest_path.into();
        let repo_root = repo_root.into();

        let node = load_node_record(&campaign_manifest_path, node_id).map_err(|source| {
            MaterializeBranchError::LoadNode {
                node_id: node_id.to_string(),
                source,
            }
        })?;
        let resolved =
            resolve_treatment_branch(&campaign_id, &campaign_manifest_path, &node.branch_id)
                .map_err(|source| MaterializeBranchError::ResolveBranch {
                    node_id: node_id.to_string(),
                    branch_id: node.branch_id.clone(),
                    source,
                })?;

        let absolute_path = repo_root.join(&resolved.target_relpath);
        let child_path = node.binary_path.clone();
        let current = fs::read_to_string(&absolute_path).map_err(|source| {
            MaterializeBranchError::ReadTarget {
                path: absolute_path.clone(),
                source,
            }
        })?;
        let observed_hash = ContentHash::of(&current);

        if current != resolved.source_content {
            return Err(MaterializeBranchError::SourceMismatch {
                path: absolute_path,
                expected_source_hash: ContentHash(resolved.source_content_hash.clone()),
                observed_hash,
            });
        }

        Ok(Self {
            campaign_id,
            campaign_manifest_path,
            node,
            resolved: resolved.clone(),
            artifact: Artifact {
                repo_root,
                target_relpath: resolved.target_relpath.clone(),
                source_content_hash: ContentHash(resolved.source_content_hash.clone()),
                current_content_hash: ContentHash::of(&resolved.source_content),
                proposed_content_hash: ContentHash(resolved.branch.proposed_content_hash.clone()),
                _lineage: PhantomData,
            },
            binary: Binary {
                parent_running: true,
                child_path,
                child_runtime: None,
                _lineage: PhantomData,
                _child: PhantomData,
                _ack: PhantomData,
            },
        })
    }
}

/// Concrete bounded surface for the tool-description target mediated by the
/// `C1 -> C2` materialization transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ToolDescriptionSurface;

impl Surface<Prototype<Parent, Parent, Absent, Unacknowledged>> for ToolDescriptionSurface {
    type Target = PathBuf;
    type ReadView = String;
    type Error = MaterializeBranchError;

    fn read_view(
        &self,
        config: &Prototype<Parent, Parent, Absent, Unacknowledged>,
        target: &Self::Target,
    ) -> Result<Self::ReadView, Self::Error> {
        let absolute_path = config.artifact.repo_root.join(target);
        fs::read_to_string(&absolute_path).map_err(|source| MaterializeBranchError::ReadTarget {
            path: absolute_path,
            source,
        })
    }
}

/// Concrete intervention mediating
/// `Prototype<Parent, Parent, Absent, Unacknowledged> ->
/// Prototype<Parent, Child, Absent, Unacknowledged>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaterializeBranch<B = GitWorktreeBackend> {
    transition_id: TransitionId,
    backend: B,
}

impl MaterializeBranch<GitWorktreeBackend> {
    pub(crate) fn new() -> Self {
        Self {
            transition_id: TransitionId::new(),
            backend: GitWorktreeBackend,
        }
    }
}

impl<B> MaterializeBranch<B> {
    pub(crate) fn with_backend(backend: B) -> Self {
        Self {
            transition_id: TransitionId::new(),
            backend,
        }
    }
}

impl<B>
    Intervention<
        Prototype<Parent, Parent, Absent, Unacknowledged>,
        Prototype<Parent, Child, Absent, Unacknowledged>,
    > for MaterializeBranch<B>
where
    B: WorkspaceBackend<Root = PathBuf>,
{
    type Surface = ToolDescriptionSurface;
    type Journal = PrototypeJournal;
    type Error = MaterializeBranchError;
    type Rejected = std::convert::Infallible;

    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self, records),
        fields(node_id = %from.node.node_id, branch_id = %from.resolved.branch.branch_id)
    )]
    fn transition(
        &self,
        from: Prototype<Parent, Parent, Absent, Unacknowledged>,
        records: &mut Self::Journal,
    ) -> Result<
        Outcome<Prototype<Parent, Child, Absent, Unacknowledged>, Self::Rejected>,
        CommitError<Self::Error, <Self::Journal as crate::intervention::RecordStore>::Error>,
    > {
        let current = ToolDescriptionSurface
            .read_view(&from, &from.resolved.target_relpath.clone())
            .map_err(CommitError::Transition)?;
        let absolute_path = from.artifact.repo_root.join(&from.resolved.target_relpath);

        if current != from.resolved.source_content {
            return Err(CommitError::Transition(
                MaterializeBranchError::SourceMismatch {
                    path: absolute_path,
                    expected_source_hash: ContentHash(from.resolved.source_content_hash.clone()),
                    observed_hash: ContentHash::of(&current),
                },
            ));
        }

        records
            .append(JournalEntry::MaterializeBranch(
                from.entry(self.transition_id, CommitPhase::Before),
            ))
            .map_err(|source| CommitError::Record {
                phase: CommitPhase::Before,
                source,
            })?;
        debug!(
            target: ploke_core::EXECUTION_DEBUG_TARGET,
            node_id = %from.node.node_id,
            branch_id = %from.resolved.branch.branch_id,
            "recorded materialize before entry"
        );

        let realized = self
            .backend
            .realize(&RealizeRequest {
                repo_root: from.artifact.repo_root.clone(),
                node_id: from.node.node_id.clone(),
                node_dir: from.node.node_dir.clone(),
                target_relpath: from.resolved.target_relpath.clone(),
                source_content: from.resolved.source_content.clone(),
                proposed_content: from.resolved.branch.proposed_content.clone(),
            })
            .map_err(|source| {
                CommitError::Transition(MaterializeBranchError::RealizeWorkspace {
                    branch_id: from.resolved.branch.branch_id.clone(),
                    source,
                })
            })?;
        let _ = update_node_status(
            &from.campaign_id,
            &from.campaign_manifest_path,
            &from.node.node_id,
            Prototype1NodeStatus::WorkspaceStaged,
        )
        .map_err(|source| {
            CommitError::Transition(MaterializeBranchError::UpdateNodeStatus {
                node_id: from.node.node_id.clone(),
                source,
            })
        })?;
        let (_, updated_node, _) = update_node_workspace_root(
            &from.campaign_id,
            &from.campaign_manifest_path,
            &from.node.node_id,
            realized.root.clone(),
        )
        .map_err(|source| {
            CommitError::Transition(MaterializeBranchError::UpdateNodeStatus {
                node_id: from.node.node_id.clone(),
                source,
            })
        })?;
        debug!(
            target: ploke_core::EXECUTION_DEBUG_TARGET,
            node_id = %from.node.node_id,
            branch_id = %from.resolved.branch.branch_id,
            workspace_root = %realized.root.display(),
            "realized child workspace and persisted runner workspace root"
        );

        let next = Prototype {
            campaign_id: from.campaign_id,
            campaign_manifest_path: from.campaign_manifest_path,
            node: updated_node,
            resolved: from.resolved.clone(),
            artifact: Artifact {
                repo_root: realized.root,
                target_relpath: from.resolved.target_relpath.clone(),
                source_content_hash: ContentHash(from.resolved.source_content_hash.clone()),
                current_content_hash: ContentHash(
                    from.resolved.branch.proposed_content_hash.clone(),
                ),
                proposed_content_hash: ContentHash(
                    from.resolved.branch.proposed_content_hash.clone(),
                ),
                _lineage: PhantomData,
            },
            binary: Binary {
                parent_running: true,
                child_path: from.node.binary_path.clone(),
                child_runtime: None,
                _lineage: PhantomData,
                _child: PhantomData,
                _ack: PhantomData,
            },
        };

        records
            .append(JournalEntry::MaterializeBranch(
                next.entry(self.transition_id, CommitPhase::After),
            ))
            .map_err(|source| CommitError::Record {
                phase: CommitPhase::After,
                source,
            })?;
        debug!(
            target: ploke_core::EXECUTION_DEBUG_TARGET,
            node_id = %next.node.node_id,
            branch_id = %next.resolved.branch.branch_id,
            workspace_root = %next.artifact.repo_root.display(),
            "recorded materialize after entry"
        );

        Ok(Outcome::Advanced(next))
    }
}
