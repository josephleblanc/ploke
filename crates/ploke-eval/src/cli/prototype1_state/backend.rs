#![allow(dead_code)] // REMOVE BY 2026-04-26: worktree backend scaffold is not wired into the live controller yet

//! Workspace realization backends for Prototype 1.
//!
//! This module keeps branch/workspace management behind a narrow trait so the
//! active generation's logic does not depend directly on git. Git worktrees
//! are the first backend because they solve the current workspace
//! branching/restore problem cheaply, but they are not the semantic model.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

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
    #[error("target file '{path}' is missing from the realized worktree")]
    MissingTarget { path: PathBuf },
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
/// materialization. The active generation should not need to know whether the
/// child is realized by git worktree, a virtual workspace layer, or some other
/// mechanism.
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
        GitBranch(format!("prototype1/{node_id}"))
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
    use super::{WorktreeEntry, parse_dirty_paths, parse_worktree_list};
    use std::path::PathBuf;

    #[test]
    fn parses_worktree_list_porcelain_output() {
        let stdout = "\
worktree /repo
HEAD abcdef
branch refs/heads/main

worktree /repo/node/worktree
HEAD 123456
branch refs/heads/prototype1/node-1
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
                        "refs/heads/prototype1/node-1".to_string(),
                    )),
                },
            ]
        );
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
}
