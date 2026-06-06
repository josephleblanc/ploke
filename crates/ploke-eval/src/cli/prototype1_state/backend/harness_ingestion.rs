//! Broad harness ingestion: TUI attempt validation and submitted-result admission.

use std::fs;
use std::path::{Path, PathBuf};

use crate::loop_graph::ArtifactId;

use super::git_worktree::{GitWorktreeBackend, dirty_paths, run_git};
use super::{
    AdmittedBroadHarnessResult, AttemptRejection, BackendError, EditSurfaceAdmission, GitBranchRef,
    GitCommit, TuiAttemptDiff, TuiAttemptOutcome, path_matches_surface_policy,
    prototype_surface_for_broad_edit_policy, repo_entry_bytes, tracked_paths,
    validate_normal_repo_relpath,
};
use crate::cli::prototype1_state::edit_surface::harness_request::{
    PublishedBroadHarnessRequest, RequestAdmissionBinding,
};
use crate::cli::prototype1_state::edit_surface::harness_result::{
    SubmittedBroadHarnessResult, SubmittedBroadHarnessResultError, transaction,
};

impl GitWorktreeBackend {
    pub(crate) fn admit_submitted_broad_harness_result(
        &self,
        repo_root: &Path,
        admission: EditSurfaceAdmission,
        published: &PublishedBroadHarnessRequest,
        submitted: &SubmittedBroadHarnessResult,
    ) -> Result<AdmittedBroadHarnessResult, BackendError> {
        submitted.verify_request(published).map_err(|err| {
            BackendError::BroadHarnessRequestBinding {
                detail: describe_submitted_broad_harness_result_error(&err),
            }
        })?;

        let live_admission_binding =
            RequestAdmissionBinding::from_admission(&admission).map_err(|err| {
                BackendError::BroadHarnessRequestBinding {
                    detail: format!("live admission binding projection failed: {err:?}"),
                }
            })?;
        if published.admission_binding() != &live_admission_binding {
            return Err(BackendError::BroadHarnessRequestBinding {
                detail: format!(
                    "published request admission binding mismatch: expected '{live_admission_binding:?}', got '{:?}'",
                    published.admission_binding()
                ),
            });
        }

        let diff = self
            .validate_tui_attempt(repo_root, published)?
            .into_result()?;
        let candidate_root = diff.candidate_root().to_path_buf();
        let source_root = diff.source_root().to_path_buf();
        let base_head = diff.base_head().to_string();
        let changed_paths = diff.into_changed_paths();

        let base_artifact_id = admission.base_artifact_id()?.clone();
        let request_id = published.request_id().to_string();
        let changes = transaction::ChangeSet::new(changed_paths.clone())
            .map_err(transaction_error_to_backend_error)?;
        self.check_artifact_surface_inputs(&candidate_root)?;
        let persisted_head = self.persist_files(
            &candidate_root,
            &changed_paths,
            &format!("prototype1 broad harness result {request_id}"),
        )?;
        let derived_artifact_id = artifact_id_from_git_commit(&persisted_head);
        let artifact_surface = self.artifact_surface(&candidate_root)?;

        Ok(transaction::Transaction::admit(
            published.reference(),
            transaction::Admission::new(live_admission_binding),
            transaction::Workspace::new(source_root, candidate_root, Some(base_head)),
            transaction::Derivation::new(base_artifact_id, derived_artifact_id, artifact_surface),
            changes,
            transaction::Submission::new(
                submitted.candidate().submitted_result_path().to_path_buf(),
            ),
            None,
        ))
    }

    pub(crate) fn validate_tui_attempt(
        &self,
        repo_root: &Path,
        published: &PublishedBroadHarnessRequest,
    ) -> Result<TuiAttemptOutcome, BackendError> {
        let expected_source_repository = published.request().workspace.source_repository_path();
        if expected_source_repository != repo_root {
            return Err(BackendError::BroadHarnessSourceRepositoryMismatch {
                expected: expected_source_repository.to_path_buf(),
                observed: repo_root.to_path_buf(),
            });
        }

        let source_root = self.worktree_root(repo_root)?;
        let candidate_root = self.worktree_root(published.workspace_path())?;
        if candidate_root == source_root
            || candidate_root.starts_with(&source_root)
            || source_root.starts_with(&candidate_root)
        {
            return Ok(TuiAttemptOutcome::rejected(
                AttemptRejection::WorkspaceNotIsolated {
                    source_repository: source_root,
                    workspace: candidate_root,
                },
            ));
        }

        let source_dirty = dirty_paths(&source_root)?;
        if !source_dirty.is_empty() {
            return Ok(TuiAttemptOutcome::rejected(AttemptRejection::SourceDirty {
                path: source_root,
                dirty_paths: source_dirty,
            }));
        }

        let expected_base_head = self.head_commit(&source_root)?;
        let observed_candidate_head = self.head_commit(&candidate_root)?;
        if observed_candidate_head != expected_base_head {
            return Ok(TuiAttemptOutcome::rejected(AttemptRejection::StaleBase {
                path: candidate_root,
                expected_head: expected_base_head,
                observed_head: observed_candidate_head,
            }));
        }

        let surface = prototype_surface_for_broad_edit_policy(published.request().edit_policy);
        let changed_paths = changed_paths_between_roots(&source_root, &candidate_root)?;
        if changed_paths.is_empty() {
            return Ok(TuiAttemptOutcome::rejected(AttemptRejection::NoChange {
                path: candidate_root,
            }));
        }

        for path in &changed_paths {
            if validate_normal_repo_relpath(path).is_err() {
                return Ok(TuiAttemptOutcome::rejected(AttemptRejection::InvalidPath {
                    path: path.clone(),
                }));
            }
            if !path_matches_surface_policy(surface, path) {
                return Ok(TuiAttemptOutcome::rejected(AttemptRejection::OutOfPolicy {
                    surface,
                    path: path.clone(),
                }));
            }
        }

        let candidate_dirty = dirty_paths(&candidate_root)?;
        let unexpected_dirty = candidate_dirty
            .into_iter()
            .filter(|dirty| !changed_paths.iter().any(|changed| changed == dirty))
            .collect::<Vec<_>>();
        if !unexpected_dirty.is_empty() {
            return Ok(TuiAttemptOutcome::rejected(
                AttemptRejection::UnexpectedDirty {
                    path: candidate_root,
                    dirty_paths: unexpected_dirty,
                },
            ));
        }

        Ok(TuiAttemptOutcome::Accepted(TuiAttemptDiff {
            source_root,
            candidate_root,
            surface,
            base_head: expected_base_head,
            changed_paths,
        }))
    }
}
impl GitWorktreeBackend {
    pub(crate) fn prepare_broad_harness_workspace(
        &self,
        repo_root: &Path,
        published: &PublishedBroadHarnessRequest,
    ) -> Result<(), BackendError> {
        let workspace = published.workspace_path();
        let branch = self.broad_harness_branch_name(published.request_id());
        if let Some(parent) = workspace.parent() {
            fs::create_dir_all(parent).map_err(|source| BackendError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        match self.find_worktree(repo_root, workspace)? {
            Some(entry) => {
                let observed_branch = entry
                    .branch
                    .clone()
                    .unwrap_or_else(|| GitBranchRef("detached".to_string()));
                let expected_branch = self.branch_ref(&branch);
                if observed_branch != expected_branch {
                    return Err(BackendError::BranchMismatch {
                        path: workspace.to_path_buf(),
                        expected_branch,
                        observed_branch,
                    });
                }
                let dirty_paths = dirty_paths(workspace)?;
                if !dirty_paths.is_empty() {
                    return Err(BackendError::DirtyWorktree {
                        path: workspace.to_path_buf(),
                        dirty_paths,
                    });
                }
                let expected_base_head = self.head_commit(repo_root)?;
                let observed_head = self.head_commit(workspace)?;
                if observed_head != expected_base_head {
                    run_git(
                        workspace,
                        &["reset", "--hard", expected_base_head.0.as_str()],
                        format!(
                            "git reset --hard {} in {}",
                            expected_base_head,
                            workspace.display()
                        ),
                    )?;
                }
            }
            None if workspace.exists() => {
                return Err(BackendError::UnmanagedPath {
                    path: workspace.to_path_buf(),
                });
            }
            None => {
                run_git(
                    repo_root,
                    &[
                        "worktree",
                        "add",
                        "-b",
                        branch.0.as_str(),
                        workspace.to_string_lossy().as_ref(),
                        "HEAD",
                    ],
                    format!("git worktree add -b {branch} {} HEAD", workspace.display()),
                )?;
            }
        }

        Ok(())
    }
}
pub(crate) fn changed_paths_between_roots(
    before_root: &Path,
    after_root: &Path,
) -> Result<Vec<PathBuf>, BackendError> {
    let mut paths = tracked_paths(before_root, ".")?;
    paths.extend(tracked_paths(after_root, ".")?);
    paths.extend(dirty_paths(after_root)?);
    paths.sort();
    paths.dedup();

    let mut changed = Vec::new();
    for path in paths {
        validate_normal_repo_relpath(&path)?;
        let before = repo_entry_bytes(before_root, &path)?;
        let after = repo_entry_bytes(after_root, &path)?;
        if before != after {
            changed.push(path);
        }
    }
    Ok(changed)
}

fn artifact_id_from_git_commit(commit: &GitCommit) -> ArtifactId {
    ArtifactId::new(format!("artifact:git-commit:{}", commit.0))
}

pub(crate) fn describe_submitted_broad_harness_result_error(
    error: &SubmittedBroadHarnessResultError,
) -> String {
    match error {
        SubmittedBroadHarnessResultError::RequestIdMismatch { expected, actual } => {
            format!("request_id mismatch: expected '{expected}', got '{actual}'")
        }
        SubmittedBroadHarnessResultError::RequestHashMismatch { expected, actual } => {
            format!("request_hash mismatch: expected '{expected}', got '{actual}'")
        }
        SubmittedBroadHarnessResultError::ParentNodeMismatch { expected, actual } => format!(
            "parent_node_id mismatch: expected '{}', got '{}'",
            expected.as_str(),
            actual.as_str()
        ),
        SubmittedBroadHarnessResultError::WorkspacePathMismatch { expected, actual } => format!(
            "workspace_path mismatch: expected '{}', got '{}'",
            expected.display(),
            actual.display()
        ),
        SubmittedBroadHarnessResultError::SubmittedResultPathMismatch { expected, actual } => {
            format!(
                "submitted_result_path mismatch: expected '{}', got '{}'",
                expected.display(),
                actual.display()
            )
        }
        SubmittedBroadHarnessResultError::RequestAdmissionBindingMismatch { expected, actual } => {
            format!("request_admission_binding mismatch: expected '{expected:?}', got '{actual:?}'")
        }
        SubmittedBroadHarnessResultError::ChangedFileOutsideWorkspace { workspace_relpath } => {
            format!(
                "changed file '{}' escaped the candidate workspace root",
                workspace_relpath.display()
            )
        }
    }
}

fn transaction_error_to_backend_error(error: transaction::Error) -> BackendError {
    let detail = match error {
        transaction::Error::EmptyChangeSet => {
            "admitted transaction change set was empty".to_string()
        }
        transaction::Error::ChangedPathOutsideWorkspace { path } => format!(
            "admitted transaction changed path '{}' was not a normal repository-relative path",
            path.display()
        ),
    };
    BackendError::BroadHarnessRequestBinding { detail }
}
