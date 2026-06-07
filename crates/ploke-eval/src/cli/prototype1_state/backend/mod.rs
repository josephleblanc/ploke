//! Workspace realization backends for Prototype 1.
//!
//! This module keeps branch/workspace management behind an adapter trait so the
//! active generation's logic does not depend directly on git. Git worktrees
//! are the first backend because they solve the current workspace
//! branching/restore problem cheaply, but they are not the semantic model.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use ploke_core::tool_types::ToolName;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::cli::Prototype1EditSurface;
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, PatchId};

use super::edit_surface::{
    self, harness_request::BroadEditPolicy, harness_result::transaction, request_policy, surface,
    tui,
};
use super::event::ContentHash;
use super::history::{
    CheckedSurface, HistoryError, HistoryHash, SurfaceCommitment, SurfaceEvidence, SurfaceTouch,
};

#[cfg(test)]
use super::history::ArtifactSurface;
use super::identity::ParentIdentity;

pub(crate) const EVAL_CORE_SURFACE_ROOT: &str = "crates/ploke-eval";
// This list is the ploke-eval-owned authority boundary for
// WorkspaceExceptPlokeEval. Ordinary child edits that touch these surfaces are
// expected to be rejected before admission or to prevent the child process from
// becoming a valid descendant.
pub(crate) const WORKSPACE_EXCEPT_AUTHORITY_PREFIXES: &[&str] = &[
    EVAL_CORE_SURFACE_ROOT,
    ".ploke",
    ".agents",
    ".codex",
    ".codex-skill-staging",
    ".tmp",
    ".symlinks",
    ".cargo",
    "docs/archive",
    "docs/active/bugs",
    "target",
    "dist",
];

pub(crate) const WORKSPACE_EXCEPT_AUTHORITY_FILENAMES: &[&str] =
    &["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"];

/// Git branch name for one backend-managed child lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitBranch(pub String);

impl std::fmt::Display for GitBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Backend-derived surface roots for the current Prototype 1 admission policy.
///
/// The fields and constructor stay private to this module so sibling modules
/// cannot fabricate the material used to construct a History
/// [`SurfaceCommitment`]. A caller can obtain one only by asking a
/// [`WorkspaceBackend`] implementation to hash concrete Artifact checkouts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SurfaceRoots {
    immutable: HistoryHash,
    mutated_before: HistoryHash,
    mutated_after: HistoryHash,
    ambient_before: HistoryHash,
    ambient_after: HistoryHash,
}

impl SurfaceRoots {
    fn new(
        immutable: HistoryHash,
        mutated_before: HistoryHash,
        mutated_after: HistoryHash,
        ambient_before: HistoryHash,
        ambient_after: HistoryHash,
    ) -> Self {
        Self {
            immutable,
            mutated_before,
            mutated_after,
            ambient_before,
            ambient_after,
        }
    }

    pub(crate) fn immutable(&self) -> &HistoryHash {
        &self.immutable
    }

    pub(crate) fn mutated_before(&self) -> &HistoryHash {
        &self.mutated_before
    }

    pub(crate) fn mutated_after(&self) -> &HistoryHash {
        &self.mutated_after
    }

    pub(crate) fn ambient_before(&self) -> &HistoryHash {
        &self.ambient_before
    }

    pub(crate) fn ambient_after(&self) -> &HistoryHash {
        &self.ambient_after
    }
}

/// Fully qualified git branch ref used for verification against worktree
/// metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitBranchRef(String);

impl std::fmt::Display for GitBranchRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Git commit id for a checked-out workspace `HEAD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitCommit(pub String);

impl std::fmt::Display for GitCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Git object id for a tree object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "algorithm", content = "bytes")]
pub(crate) enum GitObjectId {
    /// SHA-1 object id used by standard git repositories.
    Sha1([u8; 20]),
    /// SHA-256 object id used by repositories initialized with SHA-256 object
    /// format.
    Sha256([u8; 32]),
}

impl GitObjectId {
    #[allow(dead_code)] // Staged for History genesis/successor admission wiring.
    fn parse_hex(value: &str) -> Result<Self, BackendError> {
        match value.len() {
            40 => {
                let mut bytes = [0_u8; 20];
                decode_hex(value, &mut bytes)?;
                Ok(Self::Sha1(bytes))
            }
            64 => {
                let mut bytes = [0_u8; 32];
                decode_hex(value, &mut bytes)?;
                Ok(Self::Sha256(bytes))
            }
            _ => Err(BackendError::InvalidTreeKey {
                value: value.to_string(),
            }),
        }
    }
}

/// Backend-owned key derived from the current clean git tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GitTreeKey {
    tree: GitObjectId,
}

#[allow(dead_code)] // Staged for History genesis/successor admission wiring.
fn decode_hex(value: &str, output: &mut [u8]) -> Result<(), BackendError> {
    if value.len() != output.len() * 2 {
        return Err(BackendError::InvalidTreeKey {
            value: value.to_string(),
        });
    }
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let hex = std::str::from_utf8(chunk).map_err(|_| BackendError::InvalidTreeKey {
            value: value.to_string(),
        })?;
        output[index] = u8::from_str_radix(hex, 16).map_err(|_| BackendError::InvalidTreeKey {
            value: value.to_string(),
        })?;
    }
    Ok(())
}

/// Request to realize one descendant workspace from a parent artifact world.
///
/// Path roles matter here:
/// - `repo_root` is the currently active parent workspace root whose artifact
///   world we are descending from
/// - `node_dir` is allocator-owned persistent storage for this candidate node;
///   it is not itself the realized child workspace
/// - `target_relpath` is the bounded surface the backend is allowed to mutate
///
/// The backend uses these fields to either:
/// - create a new child workspace rooted under `node_dir`, derived from
///   `repo_root`, then write the proposed target content there
/// - or positively verify that an existing managed child workspace is safe to
///   reuse for the same descendant
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RealizeRequest {
    /// Parent workspace root from which the child should be realized.
    pub repo_root: PathBuf,
    /// Stable scheduler-owned node identity used for backend allocation.
    pub node_id: String,
    /// Persistent node storage root; the backend allocates child workspace
    /// paths beneath this directory.
    pub node_dir: PathBuf,
    /// Relative path of the bounded artifact surface to mediate.
    pub target_relpath: PathBuf,
    /// Expected parent content for the bounded target.
    pub source_content: String,
    /// Proposed child content for the bounded target.
    pub proposed_content: String,
}

/// One lower edit touch supplied by the TUI proposal path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProposedTouch {
    pub(crate) target: String,
    pub(crate) relpath: PathBuf,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) expected_file_hash: String,
    pub(crate) replacement: String,
}

/// Candidate edit proposal after the harness has identified concrete material
/// spans but before `ploke-eval` admits the edit as a child candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditProposal {
    pub(crate) surface: Prototype1EditSurface,
    pub(crate) proposal_id: String,
    pub(crate) run_id: String,
    pub(crate) proposal_producer: request_policy::ProposalProducer,
    pub(crate) generator_surface: tui::GeneratorSurfaceVersion,
    pub(crate) touches: Vec<ProposedTouch>,
    pub(crate) reported_after_file_hash: Option<String>,
}

/// Eval-owned authority required to admit an edit-surface proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditSurfaceAdmission {
    coordinate: Coordinate,
    policy: surface::SurfacePolicyId,
}

impl EditSurfaceAdmission {
    pub(crate) fn new(coordinate: Coordinate, policy: surface::SurfacePolicyId) -> Self {
        Self { coordinate, policy }
    }

    pub(crate) fn coordinate(&self) -> &Coordinate {
        &self.coordinate
    }

    pub(crate) fn policy(&self) -> &surface::SurfacePolicyId {
        &self.policy
    }

    fn base_artifact_id(&self) -> Result<&ArtifactId, BackendError> {
        match &self.coordinate.target {
            OperationTarget::Artifact { artifact_id } => Ok(artifact_id),
            target => Err(BackendError::EditSurfaceCheck {
                detail: format!(
                    "edit-surface admission requires OperationTarget::Artifact, got {:?}",
                    target
                ),
            }),
        }
    }
}

/// Checked single-file edit that has passed the authority-side surface gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedSurfaceEdit {
    surface: Prototype1EditSurface,
    proposal_id: String,
    run_id: String,
    proposal_producer: request_policy::ProposalProducer,
    generator_surface: tui::GeneratorSurfaceVersion,
    checked_surface: CheckedSurface,
    policy: surface::SurfacePolicyId,
    source_content: String,
    proposed_content: String,
    source_content_hash: String,
    proposed_content_hash: String,
    delta: edit_surface::ArtifactDelta,
}

impl CheckedSurfaceEdit {
    pub(crate) fn surface(&self) -> Prototype1EditSurface {
        self.surface
    }

    pub(crate) fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    pub(crate) fn target_relpath(&self) -> &Path {
        &self.checked_surface.transition.target_relpath
    }

    #[cfg(test)]
    pub(crate) fn coordinate(&self) -> Coordinate {
        self.checked_surface.grant.coordinate().operation()
    }

    #[cfg(test)]
    pub(crate) fn policy(&self) -> &surface::SurfacePolicyId {
        &self.policy
    }

    pub(crate) fn source_content(&self) -> &str {
        &self.source_content
    }

    pub(crate) fn proposed_content(&self) -> &str {
        &self.proposed_content
    }

    pub(crate) fn source_content_hash(&self) -> &str {
        &self.source_content_hash
    }

    pub(crate) fn proposed_content_hash(&self) -> &str {
        &self.proposed_content_hash
    }

    pub(crate) fn base_artifact_id(&self) -> &ArtifactId {
        &self.checked_surface.transition.base.artifact_id
    }

    pub(crate) fn patch_id(&self) -> &PatchId {
        &self.checked_surface.transition.patch_id
    }

    pub(crate) fn derived_artifact_id(&self) -> &ArtifactId {
        &self.checked_surface.transition.after.artifact_id
    }

    #[cfg(test)]
    pub(crate) fn delta(&self) -> &edit_surface::ArtifactDelta {
        &self.delta
    }

    pub(crate) fn surface_evidence(
        &self,
        producer_id: &str,
    ) -> Result<SurfaceEvidence, HistoryError> {
        let touches = self
            .delta
            .touches()
            .iter()
            .map(|touch| {
                let span = touch.span();
                let target = span.target();
                SurfaceTouch {
                    target_relpath: target.path().clone(),
                    target_name: target.name().to_string(),
                    span_relpath: span.path().clone(),
                    start: span.start(),
                    end: span.end(),
                    base_hash: span.hash().as_str().to_string(),
                    replacement: touch.replacement().to_string(),
                    replacement_hash: format!(
                        "{:x}",
                        Sha256::digest(touch.replacement().as_bytes())
                    ),
                }
            })
            .collect::<Vec<_>>();

        SurfaceEvidence::checked(
            producer_id,
            self.proposal_id.clone(),
            self.run_id.clone(),
            self.checked_surface.clone(),
            self.source_content_hash.clone(),
            self.proposed_content_hash.clone(),
            self.proposal_producer.clone(),
            self.generator_surface.clone(),
            touches,
        )
    }
}

pub(crate) type AdmittedBroadHarnessResult = transaction::Transaction<transaction::state::Admitted>;

impl transaction::Transaction<transaction::state::Admitted> {
    pub(crate) fn request_id(&self) -> &str {
        self.request().request_id()
    }

    pub(crate) fn request_hash(&self) -> &str {
        self.request().request_hash().as_str()
    }

    pub(crate) fn coordinate(&self) -> &Coordinate {
        self.admission().binding().coordinate()
    }

    #[cfg(test)]
    pub(crate) fn policy(&self) -> &edit_surface::harness_request::RequestAdmissionPolicyId {
        self.admission().binding().policy_id()
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        self.workspace().candidate_root()
    }

    #[cfg(test)]
    pub(crate) fn submitted_result_path(&self) -> &Path {
        self.submission().result_path()
    }

    pub(crate) fn changed_paths(&self) -> &[PathBuf] {
        self.changes().paths()
    }

    pub(crate) fn base_artifact_id(&self) -> &ArtifactId {
        self.artifact().base_artifact_id()
    }

    pub(crate) fn derived_artifact_id(&self) -> &ArtifactId {
        self.artifact().derived_artifact_id()
    }

    #[cfg(test)]
    pub(crate) fn artifact_surface(&self) -> &ArtifactSurface {
        self.artifact().surface()
    }
}

/// Backend view of one headless TUI attempt after the executor has returned.
///
/// This is intentionally only a workspace-diff carrier. The TUI app remains
/// responsible for running the chat/tools; the backend validates the resulting
/// candidate checkout before any admission path is allowed to persist it as an
/// Artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TuiAttemptDiff {
    source_root: PathBuf,
    candidate_root: PathBuf,
    surface: Prototype1EditSurface,
    base_head: GitCommit,
    changed_paths: Vec<PathBuf>,
}

impl TuiAttemptDiff {
    pub(crate) fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub(crate) fn candidate_root(&self) -> &Path {
        &self.candidate_root
    }

    #[cfg(test)]
    pub(crate) fn surface(&self) -> Prototype1EditSurface {
        self.surface
    }

    pub(crate) fn base_head(&self) -> &GitCommit {
        &self.base_head
    }

    pub(crate) fn changed_paths(&self) -> &[PathBuf] {
        &self.changed_paths
    }

    fn into_changed_paths(self) -> Vec<PathBuf> {
        self.changed_paths
    }
}

/// Retry-friendly boundary rejection for a returned TUI attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AttemptRejection {
    SourceDirty {
        path: PathBuf,
        dirty_paths: Vec<PathBuf>,
    },
    WorkspaceNotIsolated {
        source_repository: PathBuf,
        workspace: PathBuf,
    },
    NoChange {
        path: PathBuf,
    },
    StaleBase {
        path: PathBuf,
        expected_head: GitCommit,
        observed_head: GitCommit,
    },
    InvalidPath {
        path: PathBuf,
    },
    OutOfPolicy {
        surface: Prototype1EditSurface,
        path: PathBuf,
    },
    UnexpectedDirty {
        path: PathBuf,
        dirty_paths: Vec<PathBuf>,
    },
}

impl AttemptRejection {
    fn into_backend_error(self) -> BackendError {
        match self {
            Self::SourceDirty { path, dirty_paths } => {
                BackendError::DirtyWorktree { path, dirty_paths }
            }
            Self::WorkspaceNotIsolated {
                source_repository,
                workspace,
            } => BackendError::BroadHarnessWorkspaceNotIsolated {
                source_repository,
                workspace,
            },
            Self::NoChange { path } => BackendError::BroadHarnessNoChanges { path },
            Self::StaleBase {
                path,
                expected_head,
                observed_head,
            } => BackendError::BroadHarnessStaleBase {
                path,
                expected_head,
                observed_head,
            },
            Self::InvalidPath { path } => BackendError::InvalidEditSurfacePath { path },
            Self::OutOfPolicy { surface, path } => BackendError::OutOfEditSurface { surface, path },
            Self::UnexpectedDirty { path, dirty_paths } => {
                BackendError::DirtyWorktree { path, dirty_paths }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TuiAttemptOutcome {
    Accepted(TuiAttemptDiff),
    Rejected(AttemptRejection),
}

impl TuiAttemptOutcome {
    pub(crate) fn rejected(rejection: AttemptRejection) -> Self {
        Self::Rejected(rejection)
    }

    fn into_result(self) -> Result<TuiAttemptDiff, BackendError> {
        match self {
            Self::Accepted(diff) => Ok(diff),
            Self::Rejected(rejection) => Err(rejection.into_backend_error()),
        }
    }
}

/// Realized descendant workspace.
///
/// This is the backend's concrete witness that a child artifact world exists.
/// The important relation is:
/// - `parent_root`: the workspace root the child was derived from
/// - `parent_head`: the checked-out git `HEAD` commit of that parent workspace
/// - `root`: the child's own realized workspace root
/// - `head`: the checked-out git `HEAD` commit of that child workspace
///
/// That relation is currently simple because git worktrees derive the child
/// directly from the parent checkout, but it needs to stay explicit so later
/// backends can represent descendant realization honestly without smuggling the
/// parent/child relationship through call-site convention alone.
///
/// Note that `parent_head` / `head` are git commit identities, not artifact
/// content witnesses. The bounded target may already have diverged in the child
/// workspace without changing `head`, because realization currently writes the
/// proposed content into the worktree without committing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Workspace<Branch = GitBranch, Head = GitCommit, Root = PathBuf> {
    /// Parent workspace root this child was realized from.
    pub parent_root: Root,
    /// Checked-out git `HEAD` of the parent workspace at realization time.
    pub parent_head: Head,
    /// Backend-managed branch identity for the child workspace.
    pub branch: Branch,
    /// Realized child workspace root.
    pub root: Root,
    /// Checked-out git `HEAD` of the child workspace after realization/reuse.
    pub head: Head,
}

/// Typed failure for workspace realization backends.
#[derive(Debug, Error)]
pub(crate) enum BackendError {
    #[error("failed to create directory '{path}': {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read target file '{path}': {source}")]
    ReadTarget {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write target file '{path}': {source}")]
    WriteTarget {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("worktree path '{path}' exists but is not managed by git worktree metadata")]
    UnmanagedPath { path: PathBuf },
    #[error(
        "worktree at '{path}' belongs to branch '{observed_branch}', expected '{expected_branch}'"
    )]
    BranchMismatch {
        path: PathBuf,
        expected_branch: GitBranchRef,
        observed_branch: GitBranchRef,
    },
    #[error("worktree metadata exists for '{path}' but the path is missing on disk")]
    MissingPath { path: PathBuf },
    #[error(
        "worktree '{path}' has unexpected changes outside the mediated target: {dirty_paths:?}"
    )]
    DirtyWorktree {
        path: PathBuf,
        dirty_paths: Vec<PathBuf>,
    },
    #[error("active checkout '{path}' has local changes and cannot be switched: {dirty_paths:?}")]
    DirtyActiveCheckout {
        path: PathBuf,
        dirty_paths: Vec<PathBuf>,
    },
    #[error("parent checkout '{path}' does not match parent identity: {detail}")]
    ParentCheckoutMismatch { path: PathBuf, detail: String },
    #[allow(dead_code)] // Staged for History genesis/successor admission wiring.
    #[error("invalid git tree key '{value}'")]
    InvalidTreeKey { value: String },
    #[error("node worktree path '{observed}' did not match expected managed path '{expected}'")]
    WorkspacePathMismatch {
        expected: PathBuf,
        observed: PathBuf,
    },
    #[error("artifact workspace '{path}' is detached and has no branch identity")]
    DetachedArtifactWorkspace { path: PathBuf },
    #[error("target file '{path}' is missing from the realized worktree")]
    MissingTarget { path: PathBuf },
    #[error(
        "branch '{branch}' target '{target_relpath}' does not match the expected artifact content"
    )]
    BranchTargetMismatch {
        branch: GitBranch,
        target_relpath: PathBuf,
    },
    #[error(
        "target file '{path}' did not match stored source or proposed content before reuse \
         (observed={observed_hash}, source={source_hash}, proposed={proposed_hash})"
    )]
    UnexpectedTargetContent {
        path: PathBuf,
        observed_hash: super::event::ContentHash,
        source_hash: super::event::ContentHash,
        proposed_hash: super::event::ContentHash,
    },
    #[error("failed to run git command '{command}': {source}")]
    GitCommand {
        command: String,
        source: std::io::Error,
    },
    #[error("git command '{command}' failed with status {status}: {stderr}")]
    GitCommandStatus {
        command: String,
        status: i32,
        stderr: String,
    },
    #[error("surface path '{path}' is not valid UTF-8")]
    NonUtf8SurfacePath { path: PathBuf },
    #[error("declared surface pathspec '{pathspec}' matched no tracked files in '{root}'")]
    EmptySurfacePathspec { root: PathBuf, pathspec: String },
    #[error("declared surface file '{path}' is missing")]
    MissingSurfaceFile { path: PathBuf },
    #[error("immutable surface changed across candidate artifact: before={before}, after={after}")]
    ImmutableSurfaceChanged { before: String, after: String },
    #[error("artifact surface measurement failed: {detail}")]
    ArtifactSurfaceMeasurement { detail: String },
    #[error("edit-surface proposal for {surface:?} had no touches")]
    EmptyEditTouches { surface: Prototype1EditSurface },
    #[error("edit-surface proposal touched multiple files: {paths:?}")]
    MultiFileEdit { paths: Vec<PathBuf> },
    #[error("edit path '{path}' is outside edit surface {surface:?}")]
    OutOfEditSurface {
        surface: Prototype1EditSurface,
        path: PathBuf,
    },
    #[error("edit path '{path}' is not a normal repository-relative path")]
    InvalidEditSurfacePath { path: PathBuf },
    #[error("edit span {start}..{end} is invalid for '{path}' with length {len}")]
    InvalidEditSpan {
        path: PathBuf,
        start: usize,
        end: usize,
        len: usize,
    },
    #[error(
        "edit spans overlap in '{path}': previous {previous_start}..{previous_end}, next {next_start}..{next_end}"
    )]
    OverlappingEditSpans {
        path: PathBuf,
        previous_start: usize,
        previous_end: usize,
        next_start: usize,
        next_end: usize,
    },
    #[error("edit base hash for '{path}' is stale: expected {expected}, actual {actual}")]
    StaleEditBaseHash {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("checked edit-surface transition failed: {detail}")]
    EditSurfaceCheck { detail: String },
    #[error("submitted broad harness result did not bind to the published request: {detail}")]
    BroadHarnessRequestBinding { detail: String },
    #[error(
        "published broad harness source repository '{expected}' did not match active repo root '{observed}'"
    )]
    BroadHarnessSourceRepositoryMismatch {
        expected: PathBuf,
        observed: PathBuf,
    },
    #[error(
        "published broad harness candidate workspace '{workspace}' is not isolated from source repository '{source_repository}'"
    )]
    BroadHarnessWorkspaceNotIsolated {
        source_repository: PathBuf,
        workspace: PathBuf,
    },
    #[error("submitted broad harness candidate workspace '{path}' has no admitted changes")]
    BroadHarnessNoChanges { path: PathBuf },
    #[error(
        "submitted broad harness candidate workspace '{path}' is stale: expected base HEAD {expected_head}, observed {observed_head}"
    )]
    BroadHarnessStaleBase {
        path: PathBuf,
        expected_head: GitCommit,
        observed_head: GitCommit,
    },
}

/// Backend for realizing descendant workspaces.
///
/// The backend owns the operational mechanics of descendant workspace
/// materialization, artifact persistence, active-checkout installation, and
/// cleanup. The active generation should not need to know whether a child is
/// realized by git worktree, a virtual workspace layer, or another mechanism.
pub(crate) trait WorkspaceBackend {
    /// Backend-specific branch identity for one realized child workspace.
    type Branch: Clone + std::fmt::Debug + PartialEq + Eq;
    /// Backend-specific checked-out head identity for one workspace state.
    type Head: Clone + std::fmt::Debug + PartialEq + Eq;
    /// Backend-specific root locator for one realized workspace.
    type Root: Clone + std::fmt::Debug + PartialEq + Eq;
    /// Backend-specific key for the exact clean Artifact tree hosted by a
    /// checkout root.
    type TreeKey: Clone + std::fmt::Debug + PartialEq + Eq + Serialize;

    /// Realize or safely reuse one child workspace for the requested node.
    ///
    /// The contract is intentionally strict:
    /// - create the child workspace if it does not exist
    /// - reuse it only if it is positively verified as the expected managed
    ///   child workspace for this node
    /// - otherwise fail, rather than destructively replacing an occupied path
    fn realize(
        &self,
        request: &RealizeRequest,
    ) -> Result<Workspace<Self::Branch, Self::Head, Self::Root>, BackendError>;

    /// Explicitly remove one managed child workspace.
    ///
    /// Cleanup is separate from `realize()` on purpose so descendant
    /// realization stays non-destructive by default.
    fn remove(
        &self,
        repo_root: &Path,
        workspace: &Workspace<Self::Branch, Self::Head, Self::Root>,
    ) -> Result<(), BackendError>;

    /// Reconstruct the backend-owned workspace handle for one persisted node.
    ///
    /// This lets cleanup and handoff paths verify that a stored workspace root
    /// is the backend-managed child workspace for that node before performing
    /// any destructive operation.
    fn workspace_for_node(
        &self,
        node_id: &str,
        node_dir: &Path,
        workspace_root: &Path,
    ) -> Result<Workspace<Self::Branch, Self::Head, Self::Root>, BackendError>;

    /// Persist the realized child workspace as a durable Artifact.
    ///
    /// For git this commits the mediated target on the child branch. Other
    /// backends may snapshot, content-address, or otherwise record the same
    /// semantic event.
    fn persist_workspace_target(
        &self,
        workspace: &Workspace<Self::Branch, Self::Head, Self::Root>,
        target_relpath: &Path,
        message: &str,
    ) -> Result<Self::Head, BackendError>;

    /// Persist a bounded set of files in one realized workspace.
    fn persist_workspace_files(
        &self,
        workspace: &Workspace<Self::Branch, Self::Head, Self::Root>,
        relpaths: &[PathBuf],
        message: &str,
    ) -> Result<Self::Head, BackendError>;

    /// Verify that the durable Artifact handle carries the expected bounded
    /// target content before a caller installs or evaluates it.
    fn verify_artifact_target(
        &self,
        repo_root: &Path,
        artifact: &Self::Branch,
        target_relpath: &Path,
        expected_content: &str,
    ) -> Result<(), BackendError>;

    /// Install the selected durable Artifact into the stable active checkout.
    ///
    /// The active checkout path remains the parent runtime home; this operation
    /// changes the Artifact hosted there.
    fn install_artifact_in_active_checkout(
        &self,
        active_parent_root: &Path,
        artifact: &Self::Branch,
    ) -> Result<Self::Head, BackendError>;

    /// Create a fresh parent bootstrap branch in the stable active checkout.
    ///
    /// This is stricter than `checkout_branch`: a gen0 bootstrap must not
    /// reuse an existing branch, because the parent identity commit is the
    /// branch's identity witness.
    fn checkout_fresh_parent_branch(
        &self,
        active_parent_root: &Path,
        branch: &str,
    ) -> Result<Self::Head, BackendError>;

    /// Persist a bounded set of files in the stable active checkout.
    fn persist_active_checkout_files(
        &self,
        active_parent_root: &Path,
        relpaths: &[PathBuf],
        message: &str,
    ) -> Result<Self::Head, BackendError>;

    /// Validate that this checkout is allowed to begin acting as the given
    /// Parent.
    ///
    /// For git this is a branch/commit-message guard. Other backends should
    /// validate the same semantic condition against their own durable artifact
    /// metadata: the runtime is starting from the committed identity artifact
    /// that names the Parent about to run.
    fn validate_parent_checkout(
        &self,
        active_parent_root: &Path,
        identity: &ParentIdentity,
    ) -> Result<(), BackendError>;

    /// Derive the backend-owned key for the exact clean Artifact tree hosted
    /// by a checkout root.
    ///
    /// This is the operation History admission should rely on. Callers should
    /// not construct tree keys from strings or command-line arguments.
    #[allow(dead_code)] // Staged for History genesis/successor admission wiring.
    fn clean_tree_key(&self, active_parent_root: &Path) -> Result<Self::TreeKey, BackendError>;

    /// Compute the current Prototype 1 surface commitment from two checked-out
    /// Artifacts without executing either candidate.
    ///
    /// For the current single-ruler protocol, this is the code-level policy
    /// boundary: `crates/ploke-eval` is the immutable authority surface, and
    /// all tool-description files form the ordinary mutated surface. The
    /// method rejects if the immutable root differs across the transition.
    fn surface_commitment(
        &self,
        before_root: &Path,
        after_root: &Path,
    ) -> Result<SurfaceCommitment, BackendError>;
}

pub(crate) fn tracked_paths(
    worktree_root: &Path,
    pathspec: &str,
) -> Result<Vec<PathBuf>, BackendError> {
    let output = Command::new("git")
        .current_dir(worktree_root)
        .args(["ls-files", "--", pathspec])
        .output()
        .map_err(|source| BackendError::GitCommand {
            command: format!("git ls-files -- {pathspec}"),
            source,
        })?;

    if !output.status.success() {
        return Err(BackendError::GitCommandStatus {
            command: format!("git ls-files -- {pathspec}"),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(PathBuf::from)
        .collect())
}

fn tool_description_paths() -> Vec<PathBuf> {
    ToolName::ALL
        .iter()
        .map(|tool| PathBuf::from(tool.description_artifact_relpath()))
        .collect()
}

fn ploke_tui_tool_files() -> Vec<PathBuf> {
    vec![
        PathBuf::from("crates/ploke-tui/src/rag/tools.rs"),
        PathBuf::from("crates/ploke-tui/src/rag/editing.rs"),
    ]
}

pub(crate) fn mutated_surface_paths(worktree_root: &Path) -> Result<Vec<PathBuf>, BackendError> {
    let mut paths = edit_surface_paths(
        worktree_root,
        Prototype1EditSurface::WorkspaceExceptPlokeEval,
    )?;
    paths.extend(tool_description_paths());
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub(crate) fn edit_surface_paths(
    worktree_root: &Path,
    surface: Prototype1EditSurface,
) -> Result<Vec<PathBuf>, BackendError> {
    match surface {
        Prototype1EditSurface::PlokeTuiTools => {
            let mut paths = ploke_tui_tool_files();
            let tool_paths = tracked_paths(worktree_root, "crates/ploke-tui/src/tools")?;
            if tool_paths.is_empty() {
                return Err(BackendError::EmptySurfacePathspec {
                    root: worktree_root.to_path_buf(),
                    pathspec: "crates/ploke-tui/src/tools".to_string(),
                });
            }
            paths.extend(tool_paths);
            paths.sort();
            paths.dedup();
            Ok(paths)
        }
        Prototype1EditSurface::WorkspaceExceptPlokeEval => {
            let paths = tracked_paths(worktree_root, ".")?
                .into_iter()
                .filter(|path| !is_workspace_except_forbidden_path(path))
                .collect::<Vec<_>>();
            if paths.is_empty() {
                return Err(BackendError::EmptySurfacePathspec {
                    root: worktree_root.to_path_buf(),
                    pathspec: ". excluding Prototype 1 authority surfaces".to_string(),
                });
            }
            Ok(paths)
        }
    }
}

pub(crate) fn edit_surface_contains_path(
    worktree_root: &Path,
    surface: Prototype1EditSurface,
    path: &Path,
) -> Result<bool, BackendError> {
    Ok(edit_surface_paths(worktree_root, surface)?
        .iter()
        .any(|surface_path| surface_path == path))
}

pub(crate) fn path_matches_surface_policy(surface: Prototype1EditSurface, path: &Path) -> bool {
    match surface {
        Prototype1EditSurface::PlokeTuiTools => {
            path.starts_with("crates/ploke-tui/src/tools")
                || ploke_tui_tool_files().iter().any(|allowed| allowed == path)
        }
        Prototype1EditSurface::WorkspaceExceptPlokeEval => {
            !is_workspace_except_forbidden_path(path)
        }
    }
}

fn is_workspace_except_forbidden_path(path: &Path) -> bool {
    WORKSPACE_EXCEPT_AUTHORITY_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| WORKSPACE_EXCEPT_AUTHORITY_FILENAMES.contains(&name))
}

pub(crate) fn prototype_surface_for_broad_edit_policy(
    policy: BroadEditPolicy,
) -> Prototype1EditSurface {
    match policy {
        BroadEditPolicy::WorkspaceExceptPlokeEval => {
            Prototype1EditSurface::WorkspaceExceptPlokeEval
        }
    }
}

pub(crate) fn validate_normal_repo_relpath(path: &Path) -> Result<(), BackendError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(BackendError::InvalidEditSurfacePath {
            path: path.to_path_buf(),
        });
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(BackendError::InvalidEditSurfacePath {
                    path: path.to_path_buf(),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn content_hash(content: &str) -> String {
    ContentHash::of(content).0
}

pub(crate) fn validate_touch_spans(
    path: &Path,
    source: &str,
    touches: &[ProposedTouch],
) -> Result<(), BackendError> {
    let len = source.len();
    let mut previous: Option<&ProposedTouch> = None;
    for touch in touches {
        if touch.start > touch.end
            || touch.end > len
            || !source.is_char_boundary(touch.start)
            || !source.is_char_boundary(touch.end)
        {
            return Err(BackendError::InvalidEditSpan {
                path: path.to_path_buf(),
                start: touch.start,
                end: touch.end,
                len,
            });
        }
        if let Some(prev) = previous {
            if touch.start < prev.end {
                return Err(BackendError::OverlappingEditSpans {
                    path: path.to_path_buf(),
                    previous_start: prev.start,
                    previous_end: prev.end,
                    next_start: touch.start,
                    next_end: touch.end,
                });
            }
        }
        previous = Some(touch);
    }
    Ok(())
}

pub(crate) fn fold_touches(source: &str, touches: &[ProposedTouch]) -> String {
    let mut result = source.to_string();
    for touch in touches.iter().rev() {
        result.replace_range(touch.start..touch.end, &touch.replacement);
    }
    result
}

pub(crate) fn surface_hash(
    worktree_root: &Path,
    relpaths: &[PathBuf],
) -> Result<HistoryHash, BackendError> {
    let mut relpaths = relpaths.to_vec();
    relpaths.sort();

    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"prototype1.surface.root.v1\0");
    for relpath in relpaths {
        let relpath_str = relpath
            .to_str()
            .ok_or_else(|| BackendError::NonUtf8SurfacePath {
                path: relpath.clone(),
            })?;
        let absolute = worktree_root.join(&relpath);
        if !absolute.exists() {
            return Err(BackendError::MissingSurfaceFile { path: absolute });
        }
        let bytes = surface_entry_bytes(&absolute).map_err(|source| BackendError::ReadTarget {
            path: absolute,
            source,
        })?;
        let file_hash: [u8; 32] = Sha256::digest(&bytes).into();
        preimage.extend_from_slice(relpath_str.as_bytes());
        preimage.push(0);
        preimage.extend_from_slice(&file_hash);
        preimage.push(0);
    }

    Ok(HistoryHash::of_bytes(&preimage))
}

fn surface_entry_bytes(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(fs::read_link(path)?
            .as_os_str()
            .to_string_lossy()
            .into_owned()
            .into_bytes());
    }
    fs::read(path)
}

pub(crate) fn repo_entry_bytes(
    root: &Path,
    relpath: &Path,
) -> Result<Option<Vec<u8>>, BackendError> {
    let absolute = root.join(relpath);
    match fs::symlink_metadata(&absolute) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                let bytes = fs::read_link(&absolute)
                    .map(|target| {
                        target
                            .as_os_str()
                            .to_string_lossy()
                            .into_owned()
                            .into_bytes()
                    })
                    .map_err(|source| BackendError::ReadTarget {
                        path: absolute,
                        source,
                    })?;
                Ok(Some(bytes))
            } else {
                fs::read(&absolute)
                    .map(Some)
                    .map_err(|source| BackendError::ReadTarget {
                        path: absolute,
                        source,
                    })
            }
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(BackendError::ReadTarget {
            path: absolute,
            source,
        }),
    }
}

mod git_worktree;
mod harness_ingestion;
mod surface_admission;

pub(crate) use git_worktree::*;

#[cfg(test)]
pub(crate) use harness_ingestion::describe_submitted_broad_harness_result_error;

#[cfg(test)]
mod tests;
