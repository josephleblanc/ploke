//! Workspace realization backends for Prototype 1.
//!
//! This module keeps branch/workspace management behind an adapter trait so the
//! active generation's logic does not depend directly on git. Git worktrees
//! are the first backend because they solve the current workspace
//! branching/restore problem cheaply, but they are not the semantic model.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use ploke_core::{WriteSnippetData, tool_types::ToolName};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::cli::Prototype1EditSurface;
use crate::intervention::{text_file_artifact_id, text_replacement_patch_id};
use crate::loop_graph::{ArtifactId, PatchId};

use super::edit_surface::{self, graph, surface, tui};
use super::event::ContentHash;
use super::history::{HistoryHash, SurfaceCommitment};
use super::identity::{PARENT_IDENTITY_RELPATH, ParentIdentity, parent_identity_commit_message};

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
    pub(crate) touches: Vec<ProposedTouch>,
    pub(crate) reported_after_file_hash: Option<String>,
}

/// Convert resolved TUI byte-span writes into the backend proposal carrier.
///
/// `WriteSnippetData::expected_file_hash` is TUI tracking evidence, not the
/// authority-side expected base hash. This projection reads the target file
/// from the parent checkout and fills `ProposedTouch::expected_file_hash` with
/// the backend-owned content hash that `validate_edit_surface_candidate`
/// expects.
pub(crate) fn proposal_from_resolved_writes(
    repo_root: &Path,
    surface: Prototype1EditSurface,
    proposal_id: impl Into<String>,
    run_id: impl Into<String>,
    writes: &[WriteSnippetData],
) -> Result<EditProposal, BackendError> {
    let mut touches = Vec::with_capacity(writes.len());
    for write in writes {
        let relpath = write_relpath(repo_root, &write.file_path)?;
        validate_normal_repo_relpath(&relpath)?;
        let absolute_target = repo_root.join(&relpath);
        let source_content =
            fs::read_to_string(&absolute_target).map_err(|source| BackendError::ReadTarget {
                path: absolute_target,
                source,
            })?;
        touches.push(ProposedTouch {
            target: write.name.clone(),
            relpath,
            start: write.start_byte,
            end: write.end_byte,
            expected_file_hash: content_hash(&source_content),
            replacement: write.replacement.clone(),
        });
    }

    Ok(EditProposal {
        surface,
        proposal_id: proposal_id.into(),
        run_id: run_id.into(),
        touches,
        reported_after_file_hash: None,
    })
}

fn write_relpath(repo_root: &Path, file_path: &Path) -> Result<PathBuf, BackendError> {
    if file_path.is_absolute() {
        file_path
            .strip_prefix(repo_root)
            .map(Path::to_path_buf)
            .map_err(|_| BackendError::InvalidEditSurfacePath {
                path: file_path.to_path_buf(),
            })
    } else {
        Ok(file_path.to_path_buf())
    }
}

/// Checked single-file edit that has passed the authority-side surface gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedSurfaceEdit {
    surface: Prototype1EditSurface,
    proposal_id: String,
    run_id: String,
    target_relpath: PathBuf,
    source_content: String,
    proposed_content: String,
    source_content_hash: String,
    proposed_content_hash: String,
    base_artifact_id: ArtifactId,
    patch_id: PatchId,
    derived_artifact_id: ArtifactId,
    delta: edit_surface::ArtifactDelta,
}

impl CheckedSurfaceEdit {
    pub(crate) fn surface(&self) -> Prototype1EditSurface {
        self.surface
    }

    pub(crate) fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    pub(crate) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(crate) fn target_relpath(&self) -> &Path {
        &self.target_relpath
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
        &self.base_artifact_id
    }

    pub(crate) fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }

    pub(crate) fn derived_artifact_id(&self) -> &ArtifactId {
        &self.derived_artifact_id
    }

    pub(crate) fn delta(&self) -> &edit_surface::ArtifactDelta {
        &self.delta
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

/// Git worktree realization backend.
///
/// TODO(2026-04-27_git-backend): We are not fully satisfied with this seam yet. If git
/// remains the workspace backend, we want native git object validation and
/// native git operations here instead of shelling out to `git` commands and
/// treating their output as our primary source of truth. The current approach
/// is acceptable for the prototype because it keeps the operational model
/// simple, but it is not the long-term shape we want.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct GitWorktreeBackend;

impl GitWorktreeBackend {
    /// Deterministic branch allocation for one scheduler node.
    fn branch_name(&self, node_id: &str) -> GitBranch {
        // Use a flat ref name rather than a nested namespace. Nested names
        // like `prototype1/<node>` are fragile because any existing flat ref
        // at an intermediate path segment blocks creation of descendant refs.
        GitBranch(format!("prototype1-{node_id}"))
    }

    /// Deterministic child workspace location under the node-owned storage
    /// root.
    fn workspace_root(&self, node_dir: &Path) -> PathBuf {
        node_dir.join("worktree")
    }

    /// Fully qualified branch ref used when verifying existing worktree state.
    fn branch_ref(&self, branch: &GitBranch) -> GitBranchRef {
        GitBranchRef(format!("refs/heads/{}", branch.0))
    }

    /// Find the git-managed worktree entry, if any, for one expected child
    /// workspace root.
    fn find_worktree(
        &self,
        repo_root: &Path,
        root: &Path,
    ) -> Result<Option<WorktreeEntry>, BackendError> {
        Ok(list_worktrees(repo_root)?
            .into_iter()
            .find(|entry| entry.root == root))
    }

    /// Check whether the child branch already exists before deciding whether to
    /// create it or attach a new worktree to it.
    fn branch_exists(&self, repo_root: &Path, branch: &GitBranch) -> Result<bool, BackendError> {
        let branch_ref = self.branch_ref(branch);
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["show-ref", "--verify", "--quiet", &branch_ref.0])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: format!("git show-ref --verify --quiet {branch_ref}"),
                source,
            })?;

        Ok(output.status.success())
    }

    /// Resolve the checked-out git `HEAD` for one workspace.
    fn head_commit(&self, repo_root: &Path) -> Result<GitCommit, BackendError> {
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["rev-parse", "HEAD"])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: "git rev-parse HEAD".to_string(),
                source,
            })?;

        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git rev-parse HEAD".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(GitCommit(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    }

    fn current_branch(&self, repo_root: &Path) -> Result<String, BackendError> {
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["branch", "--show-current"])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: "git branch --show-current".to_string(),
                source,
            })?;

        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git branch --show-current".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            return Err(BackendError::ParentCheckoutMismatch {
                path: repo_root.to_path_buf(),
                detail: "active checkout is detached; parent checkout requires a branch"
                    .to_string(),
            });
        }
        Ok(branch)
    }

    fn head_commit_message(&self, repo_root: &Path) -> Result<String, BackendError> {
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["log", "-1", "--pretty=%B"])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: "git log -1 --pretty=%B".to_string(),
                source,
            })?;

        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git log -1 --pretty=%B".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn head_changed_paths(&self, repo_root: &Path) -> Result<Vec<PathBuf>, BackendError> {
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: "git diff-tree --no-commit-id --name-only -r HEAD".to_string(),
                source,
            })?;

        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git diff-tree --no-commit-id --name-only -r HEAD".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(PathBuf::from)
            .collect())
    }

    fn head_parent_reachable_from_other_branch(
        &self,
        repo_root: &Path,
        branch: &str,
    ) -> Result<bool, BackendError> {
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: "git for-each-ref --format=%(refname:short) refs/heads".to_string(),
                source,
            })?;

        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git for-each-ref --format=%(refname:short) refs/heads".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        for candidate in String::from_utf8_lossy(&output.stdout).lines() {
            if candidate == branch {
                continue;
            }
            let status = Command::new("git")
                .current_dir(repo_root)
                .args(["merge-base", "--is-ancestor", "HEAD^", candidate])
                .status()
                .map_err(|source| BackendError::GitCommand {
                    command: format!("git merge-base --is-ancestor HEAD^ {candidate}"),
                    source,
                })?;
            if status.success() {
                return Ok(true);
            }
            if status.code() != Some(1) {
                return Err(BackendError::GitCommandStatus {
                    command: format!("git merge-base --is-ancestor HEAD^ {candidate}"),
                    status: status.code().unwrap_or(-1),
                    stderr: String::new(),
                });
            }
        }

        Ok(false)
    }

    /// Validate a concrete, already-produced TUI edit proposal and return the
    /// current single-file child-candidate carrier.
    ///
    /// This is deliberately not a proposal generator. The live CLI path must
    /// still fail closed until a real TUI proposal producer supplies these
    /// spans and replacements.
    pub(crate) fn validate_edit_surface_candidate(
        &self,
        repo_root: &Path,
        proposal: EditProposal,
    ) -> Result<CheckedSurfaceEdit, BackendError> {
        use super::edit_surface::graph::View as _;

        if proposal.touches.is_empty() {
            return Err(BackendError::EmptyEditTouches {
                surface: proposal.surface,
            });
        }

        let mut paths = proposal
            .touches
            .iter()
            .map(|touch| touch.relpath.clone())
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        if paths.len() != 1 {
            return Err(BackendError::MultiFileEdit { paths });
        }
        let target_relpath = paths.remove(0);
        validate_normal_repo_relpath(&target_relpath)?;
        if !is_allowed_edit_surface_path(proposal.surface, &target_relpath) {
            return Err(BackendError::OutOfEditSurface {
                surface: proposal.surface,
                path: target_relpath,
            });
        }

        let absolute_target = repo_root.join(&target_relpath);
        let source_content =
            fs::read_to_string(&absolute_target).map_err(|source| BackendError::ReadTarget {
                path: absolute_target,
                source,
            })?;
        let source_hash = content_hash(&source_content);
        let source_surface_hash = surface::Hash::new(source_hash.clone());

        let mut touches = proposal.touches;
        touches.sort_by_key(|touch| (touch.start, touch.end));
        validate_touch_spans(&target_relpath, &source_content, &touches)?;
        for touch in &touches {
            if touch.expected_file_hash != source_hash {
                return Err(BackendError::StaleEditBaseHash {
                    path: target_relpath.clone(),
                    expected: touch.expected_file_hash.clone(),
                    actual: source_hash.clone(),
                });
            }
        }

        let proposed_content = fold_touches(&source_content, &touches);
        let proposed_hash = content_hash(&proposed_content);
        let proposed_surface_hash = surface::Hash::new(proposed_hash.clone());
        let base_artifact_id = text_file_artifact_id(&target_relpath, &source_content);
        let derived_artifact_id = text_file_artifact_id(&target_relpath, &proposed_content);
        let patch_id =
            text_replacement_patch_id(&target_relpath, &source_content, &proposed_content);
        let base_ref = surface::Ref::new(base_artifact_id.clone(), source_surface_hash.clone());
        let after_ref =
            surface::Ref::new(derived_artifact_id.clone(), proposed_surface_hash.clone());

        let artifact = surface::Artifact::new(
            base_ref.clone(),
            [(target_relpath.clone(), source_surface_hash.clone())],
        );
        let targets = touches
            .iter()
            .enumerate()
            .map(|(index, touch)| {
                graph::Target::new(
                    target_relpath.clone(),
                    format!("{}:{}", touch.target, index),
                )
            })
            .collect::<Vec<_>>();
        let graph_nodes = touches
            .iter()
            .zip(targets.iter())
            .map(|(touch, target)| {
                graph::Node::new(
                    target.clone(),
                    target_relpath.clone(),
                    touch.start,
                    touch.end,
                )
            })
            .collect::<Vec<_>>();
        let graph = graph::Mock::new(graph_nodes.clone(), []);
        let projection =
            graph
                .project(&artifact)
                .map_err(|err| BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                })?;
        let rules = targets
            .iter()
            .cloned()
            .map(graph::Rule::Include)
            .collect::<Vec<_>>();
        let graph_bounds =
            graph
                .bounds(&projection, &rules)
                .map_err(|err| BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                })?;
        let tui_projection = tui::Projector::new(
            "prototype1:ploke-tui-tools",
            tui::Source::derived(
                "prototype1:ploke-tui-tools",
                "v1",
                "backend-owned single-file edit surface bridge",
            ),
            [tui::Rule::named(
                "prototype1:ploke-tui-tools",
                "v1",
                "crates/ploke-tui/src/tools/** plus documented rag tool files",
            )],
        )
        .project(&projection);
        let tui_bounds =
            tui::Bounds::new(tui_projection.clone(), graph_bounds.clone()).map_err(|err| {
                BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                }
            })?;

        let mut checked_touches = Vec::new();
        for (index, touch) in touches.iter().enumerate() {
            let material = tui::MaterialSpan::new(
                targets[index].clone(),
                target_relpath.clone(),
                touch.start,
                touch.end,
                source_surface_hash.clone(),
                tui::MaterialSource::TuiSplice,
            );
            let checked = tui_bounds
                .touch(&artifact, material, touch.replacement.clone())
                .map_err(|err| BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                })?;
            checked_touches.push(checked);
        }

        let grant = surface::Grant::new(
            base_ref.clone(),
            graph_bounds,
            surface::Area::new(
                checked_touches
                    .iter()
                    .map(|touch| touch.span().clone())
                    .collect::<Vec<_>>(),
            ),
        )
        .map_err(|err| BackendError::EditSurfaceCheck {
            detail: err.to_string(),
        })?;
        let staged = tui::Proposal::stage(tui::Stage {
            proposal: &proposal.proposal_id,
            run: &proposal.run_id,
            base: &base_ref,
            after: after_ref.clone(),
            projection: &tui_projection,
            touches: checked_touches.clone(),
            auto_apply: false,
        })
        .map_err(|err| BackendError::EditSurfaceCheck {
            detail: err.to_string(),
        })?;
        let check = grant
            .check(staged.draft())
            .map_err(|err| BackendError::EditSurfaceCheck {
                detail: err.to_string(),
            })?;
        let reported_after_hash = proposal
            .reported_after_file_hash
            .map(surface::Hash::new)
            .unwrap_or_else(|| proposed_surface_hash.clone());
        let writes = checked_touches
            .iter()
            .map(|touch| tui::Write::applied(touch, reported_after_hash.clone()))
            .collect::<Vec<_>>();
        let after_artifact = surface::Artifact::new(
            after_ref,
            [(target_relpath.clone(), proposed_surface_hash.clone())],
        );
        let applied = tui::Apply::from_results(staged, check, writes)
            .map_err(|err| BackendError::EditSurfaceCheck {
                detail: err.to_string(),
            })?
            .validate(&after_artifact)
            .map_err(|err| BackendError::EditSurfaceCheck {
                detail: err.to_string(),
            })?;
        let delta = applied
            .delta()
            .cloned()
            .ok_or_else(|| BackendError::EditSurfaceCheck {
                detail: "checked edit did not reach applied state".to_string(),
            })?;

        Ok(CheckedSurfaceEdit {
            surface: proposal.surface,
            proposal_id: proposal.proposal_id,
            run_id: proposal.run_id,
            target_relpath,
            source_content,
            proposed_content,
            source_content_hash: source_hash,
            proposed_content_hash: proposed_hash,
            base_artifact_id,
            patch_id,
            derived_artifact_id,
            delta,
        })
    }

    fn persist_files(
        &self,
        repo_root: &Path,
        relpaths: &[PathBuf],
        message: &str,
    ) -> Result<GitCommit, BackendError> {
        if relpaths.is_empty() {
            return self.head_commit(repo_root);
        }
        let dirty_paths = dirty_paths(repo_root)?;
        let unexpected: Vec<_> = dirty_paths
            .into_iter()
            .filter(|path| !relpaths.iter().any(|allowed| allowed == path))
            .collect();
        if !unexpected.is_empty() {
            return Err(BackendError::DirtyWorktree {
                path: repo_root.to_path_buf(),
                dirty_paths: unexpected,
            });
        }

        let relpath_args: Vec<String> = relpaths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();

        let mut add = Command::new("git");
        add.current_dir(repo_root).arg("add").arg("--");
        for relpath in &relpath_args {
            add.arg(relpath);
        }
        let status = add.status().map_err(|source| BackendError::GitCommand {
            command: "git add -- <paths>".to_string(),
            source,
        })?;
        if !status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git add -- <paths>".to_string(),
                status: status.code().unwrap_or(-1),
                stderr: String::new(),
            });
        }

        let mut diff = Command::new("git");
        diff.current_dir(repo_root)
            .arg("diff")
            .arg("--cached")
            .arg("--quiet")
            .arg("--");
        for relpath in &relpath_args {
            diff.arg(relpath);
        }
        let status = diff.status().map_err(|source| BackendError::GitCommand {
            command: "git diff --cached --quiet -- <paths>".to_string(),
            source,
        })?;
        if status.success() {
            return self.head_commit(repo_root);
        }
        if status.code() != Some(1) {
            return Err(BackendError::GitCommandStatus {
                command: "git diff --cached --quiet -- <paths>".to_string(),
                status: status.code().unwrap_or(-1),
                stderr: String::new(),
            });
        }

        let mut commit = Command::new("git");
        commit
            .current_dir(repo_root)
            .arg("commit")
            .arg("--no-gpg-sign")
            .arg("-m")
            .arg(message)
            .arg("--");
        for relpath in &relpath_args {
            commit.arg(relpath);
        }
        let status = commit.status().map_err(|source| BackendError::GitCommand {
            command: "git commit --no-gpg-sign -m <message> -- <paths>".to_string(),
            source,
        })?;
        if !status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git commit --no-gpg-sign -m <message> -- <paths>".to_string(),
                status: status.code().unwrap_or(-1),
                stderr: String::new(),
            });
        }
        self.head_commit(repo_root)
    }

    /// Verify that an existing child worktree is safe to reuse.
    ///
    /// Reuse is allowed only when:
    /// - git already recognizes the path as a worktree
    /// - the worktree belongs to the expected child branch
    /// - the path exists on disk
    /// - no unexpected files are dirty there
    /// - the mediated target still matches either the parent source content or
    ///   the already-proposed child content
    fn ensure_reusable(
        &self,
        request: &RealizeRequest,
        branch: &GitBranch,
        root: &Path,
        entry: &WorktreeEntry,
    ) -> Result<(), BackendError> {
        let expected_branch = self.branch_ref(branch);
        let observed_branch = entry
            .branch
            .clone()
            .unwrap_or_else(|| GitBranchRef("detached".to_string()));
        if observed_branch != expected_branch {
            return Err(BackendError::BranchMismatch {
                path: root.to_path_buf(),
                expected_branch,
                observed_branch,
            });
        }
        if !root.exists() {
            return Err(BackendError::MissingPath {
                path: root.to_path_buf(),
            });
        }

        let dirty_paths = dirty_paths(root)?;
        let allowed = [request.target_relpath.clone()];
        let unexpected: Vec<_> = dirty_paths
            .into_iter()
            .filter(|path| !allowed.contains(path))
            .collect();
        if !unexpected.is_empty() {
            return Err(BackendError::DirtyWorktree {
                path: root.to_path_buf(),
                dirty_paths: unexpected,
            });
        }

        let absolute_target = root.join(&request.target_relpath);
        if !absolute_target.exists() {
            return Err(BackendError::MissingTarget {
                path: absolute_target,
            });
        }

        let current =
            fs::read_to_string(&absolute_target).map_err(|source| BackendError::ReadTarget {
                path: absolute_target.clone(),
                source,
            })?;
        if current != request.source_content && current != request.proposed_content {
            return Err(BackendError::UnexpectedTargetContent {
                path: absolute_target,
                observed_hash: super::event::ContentHash::of(&current),
                source_hash: super::event::ContentHash::of(&request.source_content),
                proposed_hash: super::event::ContentHash::of(&request.proposed_content),
            });
        }

        Ok(())
    }
}

impl WorkspaceBackend for GitWorktreeBackend {
    type Branch = GitBranch;
    type Head = GitCommit;
    type Root = PathBuf;
    type TreeKey = GitTreeKey;

    /// Realize one child workspace by either creating a new git worktree or
    /// safely reusing an existing verified one.
    ///
    /// This function is intentionally non-destructive:
    /// - it never removes an occupied path merely because it exists
    /// - it fails if the path is unmanaged or belongs to the wrong child
    /// - cleanup remains an explicit caller decision through `remove()`
    fn realize(
        &self,
        request: &RealizeRequest,
    ) -> Result<Workspace<Self::Branch, Self::Head, Self::Root>, BackendError> {
        let branch = self.branch_name(&request.node_id);
        let root = self.workspace_root(&request.node_dir);
        let parent_head = self.head_commit(&request.repo_root)?;

        if let Some(parent) = root.parent() {
            fs::create_dir_all(parent).map_err(|source| BackendError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        match self.find_worktree(&request.repo_root, &root)? {
            Some(entry) => self.ensure_reusable(request, &branch, &root, &entry)?,
            None if root.exists() => {
                return Err(BackendError::UnmanagedPath { path: root.clone() });
            }
            None => {
                if self.branch_exists(&request.repo_root, &branch)? {
                    run_git(
                        &request.repo_root,
                        &[
                            "worktree",
                            "add",
                            root.to_string_lossy().as_ref(),
                            &branch.0,
                        ],
                        format!("git worktree add {} {branch}", root.display()),
                    )?;
                } else {
                    run_git(
                        &request.repo_root,
                        &[
                            "worktree",
                            "add",
                            "-b",
                            &branch.0,
                            root.to_string_lossy().as_ref(),
                            "HEAD",
                        ],
                        format!("git worktree add -b {branch} {} HEAD", root.display()),
                    )?;
                }
            }
        }

        let absolute_target = root.join(&request.target_relpath);
        if let Some(parent) = absolute_target.parent() {
            fs::create_dir_all(parent).map_err(|source| BackendError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let current =
            fs::read_to_string(&absolute_target).map_err(|source| BackendError::ReadTarget {
                path: absolute_target.clone(),
                source,
            })?;
        if current != request.source_content && current != request.proposed_content {
            return Err(BackendError::UnexpectedTargetContent {
                path: absolute_target.clone(),
                observed_hash: super::event::ContentHash::of(&current),
                source_hash: super::event::ContentHash::of(&request.source_content),
                proposed_hash: super::event::ContentHash::of(&request.proposed_content),
            });
        }
        fs::write(&absolute_target, &request.proposed_content).map_err(|source| {
            BackendError::WriteTarget {
                path: absolute_target,
                source,
            }
        })?;

        let head = self.head_commit(&root)?;

        Ok(Workspace {
            parent_root: request.repo_root.clone(),
            parent_head,
            branch,
            root,
            head,
        })
    }

    /// Remove one managed child worktree after confirming it still belongs to
    /// the expected branch/path pair.
    fn remove(
        &self,
        repo_root: &Path,
        workspace: &Workspace<Self::Branch, Self::Head, Self::Root>,
    ) -> Result<(), BackendError> {
        let branch = self.branch_ref(&workspace.branch);
        let Some(entry) = self.find_worktree(repo_root, &workspace.root)? else {
            if workspace.root.exists() {
                return Err(BackendError::UnmanagedPath {
                    path: workspace.root.clone(),
                });
            }
            return Ok(());
        };
        let observed_branch = entry
            .branch
            .clone()
            .unwrap_or_else(|| GitBranchRef("detached".to_string()));
        if observed_branch != branch {
            return Err(BackendError::BranchMismatch {
                path: workspace.root.clone(),
                expected_branch: branch,
                observed_branch,
            });
        }
        run_git(
            repo_root,
            &[
                "worktree",
                "remove",
                "--force",
                workspace.root.to_string_lossy().as_ref(),
            ],
            format!("git worktree remove --force {}", workspace.root.display()),
        )
    }

    /// Reconstruct the managed child workspace identity for cleanup or
    /// handoff.
    ///
    /// This is intentionally stricter than accepting an arbitrary persisted
    /// path. A node-owned child worktree is only cleanup-eligible when the
    /// persisted workspace root still matches the backend's deterministic
    /// allocation under that node directory.
    fn workspace_for_node(
        &self,
        node_id: &str,
        node_dir: &Path,
        workspace_root: &Path,
    ) -> Result<Workspace<Self::Branch, Self::Head, Self::Root>, BackendError> {
        let expected = self.workspace_root(node_dir);
        if workspace_root != expected {
            return Err(BackendError::WorkspacePathMismatch {
                expected,
                observed: workspace_root.to_path_buf(),
            });
        }
        Ok(Workspace {
            parent_root: PathBuf::new(),
            parent_head: GitCommit(String::new()),
            branch: self.branch_name(node_id),
            root: workspace_root.to_path_buf(),
            head: GitCommit(String::new()),
        })
    }

    /// Commit the mediated target in one child workspace so the branch becomes
    /// a recoverable artifact before the temporary worktree is removed.
    fn persist_workspace_target(
        &self,
        workspace: &Workspace<Self::Branch, Self::Head, Self::Root>,
        target_relpath: &Path,
        message: &str,
    ) -> Result<Self::Head, BackendError> {
        self.persist_workspace_files(workspace, &[target_relpath.to_path_buf()], message)
    }

    fn persist_workspace_files(
        &self,
        workspace: &Workspace<Self::Branch, Self::Head, Self::Root>,
        relpaths: &[PathBuf],
        message: &str,
    ) -> Result<Self::Head, BackendError> {
        self.persist_files(&workspace.root, relpaths, message)
    }

    /// Verify that a branch already carries the expected target content.
    fn verify_artifact_target(
        &self,
        repo_root: &Path,
        artifact: &Self::Branch,
        target_relpath: &Path,
        expected_content: &str,
    ) -> Result<(), BackendError> {
        let spec = format!("{}:{}", artifact.0, target_relpath.to_string_lossy());
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["show", &spec])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: format!("git show {spec}"),
                source,
            })?;
        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: format!("git show {spec}"),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        if output.stdout != expected_content.as_bytes() {
            return Err(BackendError::BranchTargetMismatch {
                branch: artifact.clone(),
                target_relpath: target_relpath.to_path_buf(),
            });
        }
        Ok(())
    }

    /// Move the stable parent checkout to a selected durable branch.
    ///
    /// This intentionally refuses to switch a dirty active checkout. The loop
    /// should preserve operator work and prior generation state rather than
    /// carrying stray local changes across parent authority handoff.
    fn install_artifact_in_active_checkout(
        &self,
        active_parent_root: &Path,
        artifact: &Self::Branch,
    ) -> Result<Self::Head, BackendError> {
        let dirty_paths = dirty_paths(active_parent_root)?;
        if !dirty_paths.is_empty() {
            return Err(BackendError::DirtyActiveCheckout {
                path: active_parent_root.to_path_buf(),
                dirty_paths,
            });
        }
        run_git(
            active_parent_root,
            &["switch", &artifact.0],
            format!("git switch {artifact}"),
        )?;
        self.head_commit(active_parent_root)
    }

    fn checkout_fresh_parent_branch(
        &self,
        active_parent_root: &Path,
        branch: &str,
    ) -> Result<Self::Head, BackendError> {
        let dirty_paths = dirty_paths(active_parent_root)?;
        if !dirty_paths.is_empty() {
            return Err(BackendError::DirtyActiveCheckout {
                path: active_parent_root.to_path_buf(),
                dirty_paths,
            });
        }
        let branch = GitBranch(branch.to_string());
        if self.branch_exists(active_parent_root, &branch)? {
            return Err(BackendError::ParentCheckoutMismatch {
                path: active_parent_root.to_path_buf(),
                detail: format!(
                    "gen0 parent branch '{branch}' already exists; initialize on a fresh branch"
                ),
            });
        }
        run_git(
            active_parent_root,
            &["switch", "-c", &branch.0],
            format!("git switch -c {branch}"),
        )?;
        self.head_commit(active_parent_root)
    }

    fn persist_active_checkout_files(
        &self,
        active_parent_root: &Path,
        relpaths: &[PathBuf],
        message: &str,
    ) -> Result<Self::Head, BackendError> {
        self.persist_files(active_parent_root, relpaths, message)
    }

    fn validate_parent_checkout(
        &self,
        active_parent_root: &Path,
        identity: &ParentIdentity,
    ) -> Result<(), BackendError> {
        let dirty_paths = dirty_paths(active_parent_root)?;
        if !dirty_paths.is_empty() {
            return Err(BackendError::DirtyActiveCheckout {
                path: active_parent_root.to_path_buf(),
                dirty_paths,
            });
        }

        let branch = self.current_branch(active_parent_root)?;
        if let Some(expected_branch) = identity.artifact_branch.as_deref() {
            if branch != expected_branch {
                return Err(BackendError::ParentCheckoutMismatch {
                    path: active_parent_root.to_path_buf(),
                    detail: format!(
                        "active branch '{branch}' does not match parent identity artifact_branch '{expected_branch}'"
                    ),
                });
            }
        }

        let expected_message = parent_identity_commit_message(identity);
        let observed_message = self.head_commit_message(active_parent_root)?;
        if observed_message != expected_message {
            return Err(BackendError::ParentCheckoutMismatch {
                path: active_parent_root.to_path_buf(),
                detail: format!(
                    "HEAD commit message '{observed_message}' does not match expected parent identity message '{expected_message}'"
                ),
            });
        }

        let changed_paths = self.head_changed_paths(active_parent_root)?;
        let identity_relpath = PathBuf::from(PARENT_IDENTITY_RELPATH);
        if !changed_paths.contains(&identity_relpath) {
            return Err(BackendError::ParentCheckoutMismatch {
                path: active_parent_root.to_path_buf(),
                detail: format!(
                    "HEAD commit does not carry parent identity path '{}'",
                    identity_relpath.display()
                ),
            });
        }

        if identity.generation == 0 {
            if changed_paths.len() != 1 || changed_paths[0] != identity_relpath {
                return Err(BackendError::ParentCheckoutMismatch {
                    path: active_parent_root.to_path_buf(),
                    detail: format!(
                        "gen0 parent identity commit must only change '{}', observed {changed_paths:?}",
                        identity_relpath.display()
                    ),
                });
            }
            if !self.head_parent_reachable_from_other_branch(active_parent_root, &branch)? {
                return Err(BackendError::ParentCheckoutMismatch {
                    path: active_parent_root.to_path_buf(),
                    detail: format!(
                        "gen0 parent branch '{branch}' does not appear fresh; HEAD^ is not reachable from another local branch"
                    ),
                });
            }
        }

        Ok(())
    }

    fn clean_tree_key(&self, active_parent_root: &Path) -> Result<Self::TreeKey, BackendError> {
        let dirty_paths = dirty_paths(active_parent_root)?;
        if !dirty_paths.is_empty() {
            return Err(BackendError::DirtyActiveCheckout {
                path: active_parent_root.to_path_buf(),
                dirty_paths,
            });
        }

        let output = Command::new("git")
            .current_dir(active_parent_root)
            .args(["rev-parse", "HEAD^{tree}"])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: "git rev-parse HEAD^{tree}".to_string(),
                source,
            })?;

        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git rev-parse HEAD^{tree}".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(GitTreeKey {
            tree: GitObjectId::parse_hex(&value)?,
        })
    }

    fn surface_commitment(
        &self,
        before_root: &Path,
        after_root: &Path,
    ) -> Result<SurfaceCommitment, BackendError> {
        let immutable_before_paths = tracked_paths(before_root, "crates/ploke-eval")?;
        if immutable_before_paths.is_empty() {
            return Err(BackendError::EmptySurfacePathspec {
                root: before_root.to_path_buf(),
                pathspec: "crates/ploke-eval".to_string(),
            });
        }
        let immutable_before = surface_hash(before_root, &immutable_before_paths)?;
        let immutable_after_paths = tracked_paths(after_root, "crates/ploke-eval")?;
        if immutable_after_paths.is_empty() {
            return Err(BackendError::EmptySurfacePathspec {
                root: after_root.to_path_buf(),
                pathspec: "crates/ploke-eval".to_string(),
            });
        }
        let immutable_after = surface_hash(after_root, &immutable_after_paths)?;
        if immutable_before != immutable_after {
            return Err(BackendError::ImmutableSurfaceChanged {
                before: immutable_before.as_str().to_string(),
                after: immutable_after.as_str().to_string(),
            });
        }

        let mutated_before = surface_hash(before_root, &mutated_surface_paths(before_root)?)?;
        let mutated_after = surface_hash(after_root, &mutated_surface_paths(after_root)?)?;
        let ambient_before = surface_hash(before_root, &[])?;
        let ambient_after = surface_hash(after_root, &[])?;

        Ok(SurfaceCommitment::from_backend_roots(SurfaceRoots::new(
            immutable_before,
            mutated_before,
            mutated_after,
            ambient_before,
            ambient_after,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorktreeEntry {
    root: PathBuf,
    branch: Option<GitBranchRef>,
}

/// Parse `git worktree list --porcelain` into the small amount of metadata the
/// backend needs for verification.
fn list_worktrees(repo_root: &Path) -> Result<Vec<WorktreeEntry>, BackendError> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .map_err(|source| BackendError::GitCommand {
            command: "git worktree list --porcelain".to_string(),
            source,
        })?;

    if !output.status.success() {
        return Err(BackendError::GitCommandStatus {
            command: "git worktree list --porcelain".to_string(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_worktree_list(&stdout))
}

/// Parse `git status --porcelain --untracked-files=all` into repo-relative
/// paths so reuse checks can reject unexpected dirty state.
fn dirty_paths(worktree_root: &Path) -> Result<Vec<PathBuf>, BackendError> {
    let output = Command::new("git")
        .current_dir(worktree_root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .map_err(|source| BackendError::GitCommand {
            command: "git status --porcelain --untracked-files=all".to_string(),
            source,
        })?;

    if !output.status.success() {
        return Err(BackendError::GitCommandStatus {
            command: "git status --porcelain --untracked-files=all".to_string(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_dirty_paths(&stdout))
}

fn tracked_paths(worktree_root: &Path, pathspec: &str) -> Result<Vec<PathBuf>, BackendError> {
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

fn mutated_surface_paths(worktree_root: &Path) -> Result<Vec<PathBuf>, BackendError> {
    let mut paths = tool_description_paths();
    paths.extend(ploke_tui_tool_files());
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

fn is_allowed_edit_surface_path(surface: Prototype1EditSurface, path: &Path) -> bool {
    match surface {
        Prototype1EditSurface::PlokeTuiTools => {
            path.starts_with("crates/ploke-tui/src/tools")
                || ploke_tui_tool_files().iter().any(|allowed| allowed == path)
        }
    }
}

fn validate_normal_repo_relpath(path: &Path) -> Result<(), BackendError> {
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

fn content_hash(content: &str) -> String {
    ContentHash::of(content).0
}

fn validate_touch_spans(
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

fn fold_touches(source: &str, touches: &[ProposedTouch]) -> String {
    let mut result = source.to_string();
    for touch in touches.iter().rev() {
        result.replace_range(touch.start..touch.end, &touch.replacement);
    }
    result
}

fn surface_hash(worktree_root: &Path, relpaths: &[PathBuf]) -> Result<HistoryHash, BackendError> {
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
        let bytes = fs::read(&absolute).map_err(|source| BackendError::ReadTarget {
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

/// Execute one short-lived git command and return a typed backend error on
/// failure, preserving stderr for diagnostics.
fn run_git(
    repo_root: &Path,
    args: &[&str],
    command_label: impl Into<String>,
) -> Result<(), BackendError> {
    let command_label = command_label.into();
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .map_err(|source| BackendError::GitCommand {
            command: command_label.clone(),
            source,
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(BackendError::GitCommandStatus {
            command: command_label,
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

fn parse_worktree_list(stdout: &str) -> Vec<WorktreeEntry> {
    let mut entries = Vec::new();
    let mut current: Option<WorktreeEntry> = None;

    for line in stdout.lines() {
        if line.is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(WorktreeEntry {
                root: PathBuf::from(path),
                branch: None,
            });
            continue;
        }

        if let Some(branch) = line.strip_prefix("branch ") {
            if let Some(entry) = &mut current {
                entry.branch = Some(GitBranchRef(branch.to_string()));
            }
        }
    }

    if let Some(entry) = current {
        entries.push(entry);
    }

    entries
}

fn parse_dirty_paths(stdout: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for line in stdout.lines() {
        if line.len() < 3 {
            continue;
        }
        let path = line[2..].trim_start();
        let normalized = path
            .split(" -> ")
            .last()
            .expect("split last is always present");
        paths.push(PathBuf::from(normalized));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::{
        BackendError, GitWorktreeBackend, WorkspaceBackend, WorktreeEntry, parse_dirty_paths,
        parse_worktree_list,
    };
    use crate::cli::prototype1_state::identity::{
        PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentity, parent_identity_commit_message,
        parent_identity_relpath, write_parent_identity,
    };
    use ploke_core::{PROJECT_NAMESPACE_UUID, TrackingHash, WriteSnippetData};
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use uuid::Uuid;

    fn run_git_test(repo_root: &std::path::Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_git_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo_root = tmp.path();
        run_git_test(repo_root, &["init"]);
        run_git_test(
            repo_root,
            &["config", "user.email", "prototype1@example.com"],
        );
        run_git_test(repo_root, &["config", "user.name", "Prototype 1 Test"]);
        fs::write(repo_root.join("README.md"), "base\n").expect("write base");
        run_git_test(repo_root, &["add", "README.md"]);
        run_git_test(repo_root, &["commit", "--no-gpg-sign", "-m", "base commit"]);
        tmp
    }

    fn init_surface_repo(eval_text: &str, tool_text: &str) -> tempfile::TempDir {
        let tmp = init_git_repo();
        let repo_root = tmp.path();
        let eval_path = repo_root.join("crates/ploke-eval/src");
        fs::create_dir_all(&eval_path).expect("create eval dir");
        fs::write(eval_path.join("lib.rs"), eval_text).expect("write eval file");
        let tui_tool_path = repo_root.join("crates/ploke-tui/src/tools");
        fs::create_dir_all(&tui_tool_path).expect("create tui tools dir");
        fs::write(tui_tool_path.join("code_edit.rs"), tool_text).expect("write tui tool file");
        for relpath in super::ploke_tui_tool_files() {
            let path = repo_root.join(relpath);
            fs::create_dir_all(path.parent().expect("tui file has parent"))
                .expect("create tui file dir");
            fs::write(path, tool_text).expect("write tui file");
        }
        for relpath in super::tool_description_paths() {
            let path = repo_root.join(relpath);
            fs::create_dir_all(path.parent().expect("tool file has parent"))
                .expect("create tool dir");
            fs::write(path, tool_text).expect("write tool file");
        }
        run_git_test(repo_root, &["add", "crates"]);
        run_git_test(
            repo_root,
            &["commit", "--no-gpg-sign", "-m", "surface files"],
        );
        tmp
    }

    fn write_tui_target(repo_root: &std::path::Path, content: &str) -> PathBuf {
        let relpath = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
        let path = repo_root.join(&relpath);
        fs::create_dir_all(path.parent().expect("target has parent")).expect("create target dir");
        fs::write(path, content).expect("write tui target");
        relpath
    }

    fn proposal_for(relpath: PathBuf, source: &str) -> super::EditProposal {
        let hash = super::content_hash(source);
        super::EditProposal {
            surface: crate::cli::Prototype1EditSurface::PlokeTuiTools,
            proposal_id: "proposal-1".to_string(),
            run_id: "run-1".to_string(),
            reported_after_file_hash: None,
            touches: vec![super::ProposedTouch {
                target: "code_edit".to_string(),
                relpath,
                start: 4,
                end: 7,
                expected_file_hash: hash,
                replacement: "new".to_string(),
            }],
        }
    }

    fn write_data_for(repo_root: &std::path::Path, relpath: &std::path::Path) -> WriteSnippetData {
        WriteSnippetData {
            id: Uuid::new_v4(),
            name: "code_edit".to_string(),
            file_path: repo_root.join(relpath),
            expected_file_hash: TrackingHash(Uuid::new_v4()),
            start_byte: 4,
            end_byte: 7,
            replacement: "new".to_string(),
            namespace: PROJECT_NAMESPACE_UUID,
        }
    }

    fn identity(generation: u32, parent_id: &str, artifact_branch: &str) -> ParentIdentity {
        ParentIdentity {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: "campaign-1".to_string(),
            parent_id: parent_id.to_string(),
            node_id: parent_id.to_string(),
            generation,
            instance_id: Some("instance-1".to_string()),
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: format!("branch-{parent_id}"),
            artifact_branch: Some(artifact_branch.to_string()),
            created_at: "2026-04-26T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn parses_worktree_list_porcelain_output() {
        let stdout = "\
worktree /repo
HEAD abcdef
branch refs/heads/main

worktree /repo/node/worktree
HEAD 123456
branch refs/heads/prototype1-node-1
";

        let entries = parse_worktree_list(stdout);
        assert_eq!(
            entries,
            vec![
                WorktreeEntry {
                    root: PathBuf::from("/repo"),
                    branch: Some(super::GitBranchRef("refs/heads/main".to_string())),
                },
                WorktreeEntry {
                    root: PathBuf::from("/repo/node/worktree"),
                    branch: Some(super::GitBranchRef(
                        "refs/heads/prototype1-node-1".to_string(),
                    )),
                },
            ]
        );
    }

    #[test]
    fn allocates_flat_child_branch_names() {
        let backend = super::GitWorktreeBackend;
        let branch = backend.branch_name("node-1");

        assert_eq!(branch.0, "prototype1-node-1".to_string());
        assert!(!branch.0.contains('/'));
    }

    #[test]
    fn parses_dirty_paths_from_status_output() {
        let stdout = "\
 M src/lib.rs
?? notes.txt
R  old.rs -> new.rs
";

        let paths = parse_dirty_paths(stdout);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("src/lib.rs"),
                PathBuf::from("notes.txt"),
                PathBuf::from("new.rs"),
            ]
        );
    }

    #[test]
    fn edit_surface_bridge_accepts_single_file_proposal() {
        let tmp = init_git_repo();
        let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
        let checked = GitWorktreeBackend
            .validate_edit_surface_candidate(
                tmp.path(),
                proposal_for(relpath.clone(), "let old = 1;\n"),
            )
            .expect("single-file checked edit");

        assert_eq!(checked.target_relpath(), relpath.as_path());
        assert_eq!(checked.source_content(), "let old = 1;\n");
        assert_eq!(checked.proposed_content(), "let new = 1;\n");
        assert_eq!(checked.delta().touches().len(), 1);
        assert_ne!(checked.base_artifact_id(), checked.derived_artifact_id());
    }

    #[test]
    fn edit_surface_resolved_write_conversion_uses_backend_content_hash_and_validates() {
        let tmp = init_git_repo();
        let source = "let old = 1;\n";
        let relpath = write_tui_target(tmp.path(), source);
        let write = write_data_for(tmp.path(), &relpath);
        let tracking_hash = write.expected_file_hash.0.to_string();

        let tracking_only = super::EditProposal {
            surface: crate::cli::Prototype1EditSurface::PlokeTuiTools,
            proposal_id: "proposal-tracking".to_string(),
            run_id: "run-1".to_string(),
            reported_after_file_hash: None,
            touches: vec![super::ProposedTouch {
                target: write.name.clone(),
                relpath: relpath.clone(),
                start: write.start_byte,
                end: write.end_byte,
                expected_file_hash: tracking_hash.clone(),
                replacement: write.replacement.clone(),
            }],
        };

        let err = GitWorktreeBackend
            .validate_edit_surface_candidate(tmp.path(), tracking_only)
            .expect_err("TUI TrackingHash is not the backend content hash");
        assert!(matches!(err, BackendError::StaleEditBaseHash { .. }));

        let converted = super::proposal_from_resolved_writes(
            tmp.path(),
            crate::cli::Prototype1EditSurface::PlokeTuiTools,
            "proposal-content",
            "run-1",
            &[write],
        )
        .expect("convert resolved writes");

        assert_eq!(
            converted.touches[0].expected_file_hash,
            super::content_hash(source)
        );
        assert_ne!(converted.touches[0].expected_file_hash, tracking_hash);

        let checked = GitWorktreeBackend
            .validate_edit_surface_candidate(tmp.path(), converted)
            .expect("converted proposal validates");
        assert_eq!(checked.target_relpath(), relpath.as_path());
        assert_eq!(checked.source_content_hash(), super::content_hash(source));
        assert_eq!(checked.proposed_content(), "let new = 1;\n");
    }

    #[test]
    fn edit_surface_bridge_rejects_zero_touches() {
        let tmp = init_git_repo();
        let err = GitWorktreeBackend
            .validate_edit_surface_candidate(
                tmp.path(),
                super::EditProposal {
                    surface: crate::cli::Prototype1EditSurface::PlokeTuiTools,
                    proposal_id: "proposal-1".to_string(),
                    run_id: "run-1".to_string(),
                    touches: Vec::new(),
                    reported_after_file_hash: None,
                },
            )
            .expect_err("zero touches must reject");

        assert!(matches!(err, BackendError::EmptyEditTouches { .. }));
    }

    #[test]
    fn edit_surface_bridge_rejects_multi_file_proposal() {
        let tmp = init_git_repo();
        let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
        let hash = super::content_hash("let old = 1;\n");
        let mut proposal = proposal_for(relpath, "let old = 1;\n");
        proposal.touches.push(super::ProposedTouch {
            target: "rag_tools".to_string(),
            relpath: PathBuf::from("crates/ploke-tui/src/rag/tools.rs"),
            start: 0,
            end: 0,
            expected_file_hash: hash,
            replacement: "x".to_string(),
        });

        let err = GitWorktreeBackend
            .validate_edit_surface_candidate(tmp.path(), proposal)
            .expect_err("multi-file proposal must reject");

        assert!(matches!(err, BackendError::MultiFileEdit { .. }));
    }

    #[test]
    fn edit_surface_bridge_rejects_overlapping_spans() {
        let tmp = init_git_repo();
        let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
        let hash = super::content_hash("let old = 1;\n");
        let proposal = super::EditProposal {
            surface: crate::cli::Prototype1EditSurface::PlokeTuiTools,
            proposal_id: "proposal-1".to_string(),
            run_id: "run-1".to_string(),
            reported_after_file_hash: None,
            touches: vec![
                super::ProposedTouch {
                    target: "first".to_string(),
                    relpath: relpath.clone(),
                    start: 4,
                    end: 8,
                    expected_file_hash: hash.clone(),
                    replacement: "new".to_string(),
                },
                super::ProposedTouch {
                    target: "second".to_string(),
                    relpath,
                    start: 7,
                    end: 10,
                    expected_file_hash: hash,
                    replacement: "other".to_string(),
                },
            ],
        };

        let err = GitWorktreeBackend
            .validate_edit_surface_candidate(tmp.path(), proposal)
            .expect_err("overlapping spans must reject");

        assert!(matches!(err, BackendError::OverlappingEditSpans { .. }));
    }

    #[test]
    fn edit_surface_bridge_rejects_out_of_surface_path() {
        let tmp = init_git_repo();
        let relpath = PathBuf::from("crates/ploke-eval/src/cli.rs");
        let path = tmp.path().join(&relpath);
        fs::create_dir_all(path.parent().expect("target has parent")).expect("create target dir");
        fs::write(path, "let old = 1;\n").expect("write target");

        let err = GitWorktreeBackend
            .validate_edit_surface_candidate(tmp.path(), proposal_for(relpath, "let old = 1;\n"))
            .expect_err("out-of-surface path must reject");

        assert!(matches!(err, BackendError::OutOfEditSurface { .. }));
    }

    #[test]
    fn edit_surface_bridge_rejects_prefix_path_escape() {
        let tmp = init_git_repo();
        let escaped = PathBuf::from("crates/ploke-tui/src/tools/../../../../ploke-eval/src/lib.rs");
        let resolved = tmp.path().join(&escaped);
        fs::create_dir_all(resolved.parent().expect("target has parent"))
            .expect("create escaped target dir");
        fs::write(resolved, "let old = 1;\n").expect("write escaped target");

        let err = GitWorktreeBackend
            .validate_edit_surface_candidate(tmp.path(), proposal_for(escaped, "let old = 1;\n"))
            .expect_err("path escape must reject before surface prefix check");

        assert!(matches!(err, BackendError::InvalidEditSurfacePath { .. }));
    }

    #[test]
    fn edit_surface_bridge_rejects_stale_base_hash() {
        let tmp = init_git_repo();
        let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
        let mut proposal = proposal_for(relpath, "let old = 1;\n");
        proposal.touches[0].expected_file_hash = "stale".to_string();

        let err = GitWorktreeBackend
            .validate_edit_surface_candidate(tmp.path(), proposal)
            .expect_err("stale expected hash must reject");

        assert!(matches!(err, BackendError::StaleEditBaseHash { .. }));
    }

    #[test]
    fn edit_surface_bridge_rejects_wrong_reported_after_hash() {
        let tmp = init_git_repo();
        let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
        let mut proposal = proposal_for(relpath, "let old = 1;\n");
        proposal.reported_after_file_hash = Some("wrong-after".to_string());

        let err = GitWorktreeBackend
            .validate_edit_surface_candidate(tmp.path(), proposal)
            .expect_err("wrong executor after hash must reject");

        assert!(matches!(err, BackendError::EditSurfaceCheck { .. }));
        assert!(err.to_string().contains("after artifact hash mismatch"));
    }

    #[test]
    fn validates_fresh_gen0_parent_checkout() {
        let tmp = init_git_repo();
        let repo_root = tmp.path();
        let backend = GitWorktreeBackend;
        let branch = "prototype1-parent-gen0";

        backend
            .checkout_fresh_parent_branch(repo_root, branch)
            .expect("fresh branch");
        let identity = identity(0, "node-0", branch);
        write_parent_identity(repo_root, &identity).expect("write identity");
        backend
            .persist_active_checkout_files(
                repo_root,
                &[parent_identity_relpath()],
                &parent_identity_commit_message(&identity),
            )
            .expect("commit identity");

        backend
            .validate_parent_checkout(repo_root, &identity)
            .expect("gen0 parent checkout");
    }

    #[test]
    fn rejects_contaminated_gen0_parent_branch() {
        let tmp = init_git_repo();
        let repo_root = tmp.path();
        let backend = GitWorktreeBackend;
        let branch = "prototype1-parent-gen0";

        backend
            .checkout_fresh_parent_branch(repo_root, branch)
            .expect("fresh branch");
        let identity = identity(0, "node-0", branch);
        write_parent_identity(repo_root, &identity).expect("write identity");
        backend
            .persist_active_checkout_files(
                repo_root,
                &[parent_identity_relpath()],
                &parent_identity_commit_message(&identity),
            )
            .expect("commit identity");
        fs::write(repo_root.join("contamination.txt"), "not parent identity\n")
            .expect("write contamination");
        run_git_test(repo_root, &["add", "contamination.txt"]);
        run_git_test(
            repo_root,
            &["commit", "--no-gpg-sign", "-m", "unexpected follow-up"],
        );

        let err = backend
            .validate_parent_checkout(repo_root, &identity)
            .expect_err("contaminated gen0 branch should reject");
        assert!(err.to_string().contains("does not match expected"));
    }

    #[test]
    fn validates_gen1_parent_checkout_after_artifact_commit() {
        let tmp = init_git_repo();
        let repo_root = tmp.path();
        let backend = GitWorktreeBackend;
        let branch = "prototype1-node-1";

        run_git_test(repo_root, &["switch", "-c", branch]);
        fs::write(repo_root.join("target.txt"), "artifact\n").expect("write artifact");
        run_git_test(repo_root, &["add", "target.txt"]);
        run_git_test(
            repo_root,
            &[
                "commit",
                "--no-gpg-sign",
                "-m",
                "prototype1: persist buildable artifact for node node-1",
            ],
        );
        let identity = identity(1, "node-1", branch);
        write_parent_identity(repo_root, &identity).expect("write identity");
        backend
            .persist_active_checkout_files(
                repo_root,
                &[parent_identity_relpath()],
                &parent_identity_commit_message(&identity),
            )
            .expect("commit identity");

        backend
            .validate_parent_checkout(repo_root, &identity)
            .expect("gen1 parent checkout");
    }

    #[test]
    fn surface_commitment_allows_tool_text_mutation() {
        let before = init_surface_repo("pub fn policy() {}\n", "before\n");
        let after = init_surface_repo("pub fn policy() {}\n", "after\n");
        let backend = GitWorktreeBackend;

        backend
            .surface_commitment(before.path(), after.path())
            .expect("tool text mutation preserves immutable surface");
    }

    #[test]
    fn surface_commitment_represents_ploke_tui_tool_mutation() {
        let before = init_surface_repo("pub fn policy() {}\n", "same\n");
        let after = init_surface_repo("pub fn policy() {}\n", "same\n");
        fs::write(
            after.path().join("crates/ploke-tui/src/tools/code_edit.rs"),
            "changed tui tool\n",
        )
        .expect("mutate tui tool file");
        let backend = GitWorktreeBackend;

        let unchanged = backend
            .surface_commitment(before.path(), before.path())
            .expect("unchanged surface");
        let changed = backend
            .surface_commitment(before.path(), after.path())
            .expect("tui tool mutation is represented");

        assert_ne!(unchanged, changed);
    }

    #[test]
    fn surface_commitment_rejects_eval_mutation() {
        let before = init_surface_repo("pub fn policy() {}\n", "same\n");
        let after = init_surface_repo("pub fn policy_changed() {}\n", "same\n");
        let backend = GitWorktreeBackend;

        let err = backend
            .surface_commitment(before.path(), after.path())
            .expect_err("ploke-eval mutation must reject ordinary succession");
        assert!(matches!(err, BackendError::ImmutableSurfaceChanged { .. }));
    }

    #[test]
    fn surface_commitment_rejects_missing_eval_surface() {
        let before = init_git_repo();
        let after = init_surface_repo("pub fn policy() {}\n", "same\n");
        let backend = GitWorktreeBackend;

        let err = backend
            .surface_commitment(before.path(), after.path())
            .expect_err("missing ploke-eval surface must reject ordinary succession");
        assert!(matches!(err, BackendError::EmptySurfacePathspec { .. }));
    }
}
