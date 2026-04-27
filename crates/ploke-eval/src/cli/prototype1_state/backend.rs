//! Workspace realization backends for Prototype 1.
//!
//! This module keeps branch/workspace management behind an adapter trait so the
//! active generation's logic does not depend directly on git. Git worktrees
//! are the first backend because they solve the current workspace
//! branching/restore problem cheaply, but they are not the semantic model.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use super::identity::{PARENT_IDENTITY_RELPATH, ParentIdentity, parent_identity_commit_message};

/// Git branch name for one backend-managed child lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitBranch(pub String);

impl std::fmt::Display for GitBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
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
        GitWorktreeBackend, WorkspaceBackend, WorktreeEntry, parse_dirty_paths, parse_worktree_list,
    };
    use crate::cli::prototype1_state::identity::{
        PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentity, parent_identity_commit_message,
        parent_identity_relpath, write_parent_identity,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

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

    fn identity(generation: u32, parent_id: &str, artifact_branch: &str) -> ParentIdentity {
        ParentIdentity {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: "campaign-1".to_string(),
            parent_id: parent_id.to_string(),
            node_id: parent_id.to_string(),
            generation,
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
}
