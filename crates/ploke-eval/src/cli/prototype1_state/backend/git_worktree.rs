//! Git worktree realization: branch allocation, checkout lifecycle, and git helpers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{
    BackendError, GitBranch, GitBranchRef, GitCommit, GitObjectId, GitTreeKey, RealizeRequest,
    SurfaceRoots, Workspace, WorkspaceBackend, mutated_surface_paths, surface_hash, tracked_paths,
    validate_normal_repo_relpath,
};
use crate::cli::prototype1_state::history::{
    ArtifactSurface, SurfaceCommitment, TreeKeyCommitment, TreeKeyHash,
};
use crate::cli::prototype1_state::identity::{
    PARENT_IDENTITY_RELPATH, ParentIdentity, parent_identity_commit_message,
};

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
    pub(in crate::cli::prototype1_state::backend) fn branch_name(
        &self,
        node_id: &str,
    ) -> GitBranch {
        // Use a flat ref name rather than a nested namespace. Nested names
        // like `prototype1/<node>` are fragile because any existing flat ref
        // at an intermediate path segment blocks creation of descendant refs.
        GitBranch(format!("prototype1-{node_id}"))
    }

    pub(crate) fn broad_harness_branch_name(&self, request_id: &str) -> GitBranch {
        GitBranch(format!(
            "prototype1-broad-{}",
            sanitize_git_branch_component(request_id)
        ))
    }

    /// Deterministic child workspace location under the node-owned storage
    /// root.
    fn workspace_root(&self, node_dir: &Path) -> PathBuf {
        node_dir.join("worktree")
    }

    pub(crate) fn workspace_for_artifact_root(
        &self,
        workspace_root: &Path,
    ) -> Result<Workspace, BackendError> {
        if !workspace_root.exists() {
            return Err(BackendError::MissingPath {
                path: workspace_root.to_path_buf(),
            });
        }
        let branch =
            self.current_branch(workspace_root)
                .map(GitBranch)
                .map_err(|err| match err {
                    BackendError::ParentCheckoutMismatch { path, detail }
                        if detail.contains("detached") =>
                    {
                        BackendError::DetachedArtifactWorkspace { path }
                    }
                    other => other,
                })?;
        let head = self.head_commit(workspace_root)?;
        Ok(Workspace {
            parent_root: PathBuf::new(),
            parent_head: GitCommit(String::new()),
            branch,
            root: workspace_root.to_path_buf(),
            head,
        })
    }

    pub(crate) fn artifact_id_for_head(&self, root: &Path) -> Result<ArtifactId, BackendError> {
        let head = self.head_commit(root)?;
        Ok(artifact_id_from_git_commit(&head))
    }

    pub(crate) fn branch_tree_key_hash(
        &self,
        repo_root: &Path,
        branch: &GitBranch,
    ) -> Result<TreeKeyHash, BackendError> {
        let spec = format!("{}^{{tree}}", branch.0);
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["rev-parse", &spec])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: format!("git rev-parse {spec}"),
                source,
            })?;

        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: format!("git rev-parse {spec}"),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        GitTreeKey {
            tree: GitObjectId::parse_hex(&value)?,
        }
        .tree_key_hash()
        .map_err(|source| BackendError::ArtifactSurfaceMeasurement {
            detail: source.to_string(),
        })
    }

    /// Fully qualified branch ref used when verifying existing worktree state.
    pub(crate) fn branch_ref(&self, branch: &GitBranch) -> GitBranchRef {
        GitBranchRef(format!("refs/heads/{}", branch.0))
    }

    /// Find the git-managed worktree entry, if any, for one expected child
    /// workspace root.
    pub(crate) fn find_worktree(
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
    pub(crate) fn head_commit(&self, repo_root: &Path) -> Result<GitCommit, BackendError> {
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

    pub(crate) fn worktree_root(&self, repo_root: &Path) -> Result<PathBuf, BackendError> {
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .map_err(|source| BackendError::GitCommand {
                command: "git rev-parse --show-toplevel".to_string(),
                source,
            })?;

        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git rev-parse --show-toplevel".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(PathBuf::from(
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
}
impl GitWorktreeBackend {
    pub(crate) fn stash_to_workspace(
        &self,
        source_root: &Path,
        workspace_root: &Path,
        changed_paths: &[PathBuf],
        message: &str,
    ) -> Result<Vec<PathBuf>, BackendError> {
        let relpaths = repo_relpaths(source_root, changed_paths)?;
        if relpaths.is_empty() {
            return Ok(relpaths);
        }

        let mut push = Command::new("git");
        push.current_dir(source_root)
            .arg("stash")
            .arg("push")
            .arg("--include-untracked")
            .arg("-m")
            .arg(message)
            .arg("--");
        for relpath in &relpaths {
            push.arg(relpath);
        }
        let output = push.output().map_err(|source| BackendError::GitCommand {
            command: "git stash push --include-untracked -m <message> -- <paths>".to_string(),
            source,
        })?;
        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git stash push --include-untracked -m <message> -- <paths>".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let mut pop = Command::new("git");
        pop.current_dir(workspace_root)
            .args(["stash", "pop", "stash@{0}"]);
        let output = pop.output().map_err(|source| BackendError::GitCommand {
            command: "git stash pop stash@{0}".to_string(),
            source,
        })?;
        if !output.status.success() {
            return Err(BackendError::GitCommandStatus {
                command: "git stash pop stash@{0}".to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(relpaths)
    }
}
impl GitWorktreeBackend {
    pub(crate) fn persist_files(
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
                observed_hash: crate::cli::prototype1_state::event::ContentHash::of(&current),
                source_hash: crate::cli::prototype1_state::event::ContentHash::of(
                    &request.source_content,
                ),
                proposed_hash: crate::cli::prototype1_state::event::ContentHash::of(
                    &request.proposed_content,
                ),
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
                observed_hash: crate::cli::prototype1_state::event::ContentHash::of(&current),
                source_hash: crate::cli::prototype1_state::event::ContentHash::of(
                    &request.source_content,
                ),
                proposed_hash: crate::cli::prototype1_state::event::ContentHash::of(
                    &request.proposed_content,
                ),
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
        if let Some(expected_branch) = identity.artifact_branch() {
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

        if identity.generation() == 0 {
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
impl GitWorktreeBackend {
    pub(crate) fn check_artifact_surface_inputs(&self, root: &Path) -> Result<(), BackendError> {
        let immutable_paths = tracked_paths(root, "crates/ploke-eval")?;
        if immutable_paths.is_empty() {
            return Err(BackendError::EmptySurfacePathspec {
                root: root.to_path_buf(),
                pathspec: "crates/ploke-eval".to_string(),
            });
        }
        surface_hash(root, &immutable_paths)?;
        surface_hash(root, &mutated_surface_paths(root)?)?;
        surface_hash(root, &[])?;
        Ok(())
    }

    pub(crate) fn artifact_surface(&self, root: &Path) -> Result<ArtifactSurface, BackendError> {
        let tree_key = self
            .clean_tree_key(root)?
            .tree_key_hash()
            .map_err(|source| BackendError::ArtifactSurfaceMeasurement {
                detail: source.to_string(),
            })?;
        let immutable_paths = tracked_paths(root, "crates/ploke-eval")?;
        if immutable_paths.is_empty() {
            return Err(BackendError::EmptySurfacePathspec {
                root: root.to_path_buf(),
                pathspec: "crates/ploke-eval".to_string(),
            });
        }
        let immutable = surface_hash(root, &immutable_paths)?;
        let mutated = surface_hash(root, &mutated_surface_paths(root)?)?;
        let ambient = surface_hash(root, &[])?;
        Ok(ArtifactSurface::from_backend_measurement(
            tree_key, immutable, mutated, ambient,
        ))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorktreeEntry {
    pub(crate) root: PathBuf,
    pub(crate) branch: Option<GitBranchRef>,
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
use crate::loop_graph::ArtifactId;

fn artifact_id_from_git_commit(commit: &GitCommit) -> ArtifactId {
    ArtifactId::new(format!("artifact:git-commit:{}", commit.0))
}

pub(crate) fn dirty_paths(worktree_root: &Path) -> Result<Vec<PathBuf>, BackendError> {
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

pub(crate) fn repo_relpaths(root: &Path, paths: &[PathBuf]) -> Result<Vec<PathBuf>, BackendError> {
    let mut relpaths = Vec::with_capacity(paths.len());
    for path in paths {
        let relpath = if path.is_absolute() {
            path.strip_prefix(root)
                .map_err(|_| BackendError::InvalidEditSurfacePath { path: path.clone() })?
        } else {
            path.as_path()
        };
        validate_normal_repo_relpath(relpath)?;
        relpaths.push(relpath.to_path_buf());
    }
    relpaths.sort();
    relpaths.dedup();
    Ok(relpaths)
}
/// failure, preserving stderr for diagnostics.
pub(crate) fn run_git(
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

pub(crate) fn sanitize_git_branch_component(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for ch in input.chars() {
        let allowed = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-');
        let next = if allowed { ch } else { '-' };
        if next == '-' {
            if !last_was_dash {
                output.push(next);
            }
            last_was_dash = true;
        } else {
            output.push(next);
            last_was_dash = false;
        }
    }
    let trimmed = output.trim_matches(['.', '-']).to_string();
    if trimmed.is_empty() {
        "request".to_string()
    } else {
        trimmed
    }
}

pub(in crate::cli::prototype1_state::backend) fn parse_worktree_list(
    stdout: &str,
) -> Vec<WorktreeEntry> {
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

pub(in crate::cli::prototype1_state::backend) fn parse_dirty_paths(stdout: &str) -> Vec<PathBuf> {
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
