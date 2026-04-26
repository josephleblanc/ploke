#![allow(dead_code)] // REMOVE BY 2026-04-28: additive authority design sketch is not wired yet

//! Capability carriers for the Prototype 1 operational authority model.
//!
//! This module separates authority from documentation. A caller that has an
//! [`ActiveRoot`] does not thereby have parent authority; a caller that has a
//! [`ChildRoot`] does not thereby have permission to update the active
//! checkout. Authority-bearing roles are private-field, move-only tokens, and
//! dangerous state transitions are exposed only through validation or
//! authority-consuming constructors.
//!
//! Operational invariants encoded here:
//! - the active checkout root is the authoritative parent workspace
//! - child worktree roots are ephemeral candidate workspaces
//! - the shared record root is external append-only state, not a workspace
//! - child authority can acknowledge, self-evaluate, write one bounded result,
//!   and exit
//! - parent authority can mint child capabilities, select successors, validate
//!   active-root updates, clean up child roots, and exit
//! - successor bootstrap authority must validate the active root and binary
//!   relation before it can become parent authority

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use super::event::RuntimeId;

/// Validation failure for role-bearing paths.
#[derive(Debug)]
pub(crate) enum AuthorityError {
    MissingPath {
        path: PathBuf,
    },
    NotDirectory {
        path: PathBuf,
    },
    NotFile {
        path: PathBuf,
    },
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },
    SameRoot {
        role: &'static str,
        path: PathBuf,
    },
    NestedRoot {
        role: &'static str,
        path: PathBuf,
        ancestor_role: &'static str,
        ancestor: PathBuf,
    },
    UnboundedRecord {
        path: PathBuf,
        shared: PathBuf,
    },
    Unacknowledged {
        role: &'static str,
    },
    UnselectableOutcome {
        outcome: AttemptOutcome,
    },
}

impl fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPath { path } => {
                write!(f, "authority path '{}' does not exist", path.display())
            }
            Self::NotDirectory { path } => {
                write!(f, "authority path '{}' is not a directory", path.display())
            }
            Self::NotFile { path } => {
                write!(f, "authority path '{}' is not a file", path.display())
            }
            Self::Canonicalize { path, source } => {
                write!(f, "failed to canonicalize '{}': {source}", path.display())
            }
            Self::SameRoot { role, path } => {
                write!(f, "{role} root '{}' overlaps another root", path.display())
            }
            Self::NestedRoot {
                role,
                path,
                ancestor_role,
                ancestor,
            } => write!(
                f,
                "{role} root '{}' is nested under {ancestor_role} root '{}'",
                path.display(),
                ancestor.display()
            ),
            Self::UnboundedRecord { path, shared } => write!(
                f,
                "record path '{}' is outside shared record root '{}'",
                path.display(),
                shared.display()
            ),
            Self::Unacknowledged { role } => {
                write!(f, "{role} authority has not acknowledged its bounded role")
            }
            Self::UnselectableOutcome { outcome } => {
                write!(f, "child attempt outcome '{outcome:?}' cannot be selected")
            }
        }
    }
}

impl std::error::Error for AuthorityError {}

fn canonical_dir(path: impl AsRef<Path>) -> Result<PathBuf, AuthorityError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AuthorityError::MissingPath {
            path: path.to_path_buf(),
        });
    }
    if !path.is_dir() {
        return Err(AuthorityError::NotDirectory {
            path: path.to_path_buf(),
        });
    }
    fs::canonicalize(path).map_err(|source| AuthorityError::Canonicalize {
        path: path.to_path_buf(),
        source,
    })
}

fn canonical_file(path: impl AsRef<Path>) -> Result<PathBuf, AuthorityError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AuthorityError::MissingPath {
            path: path.to_path_buf(),
        });
    }
    if !path.is_file() {
        return Err(AuthorityError::NotFile {
            path: path.to_path_buf(),
        });
    }
    fs::canonicalize(path).map_err(|source| AuthorityError::Canonicalize {
        path: path.to_path_buf(),
        source,
    })
}

fn canonical_record(
    path: impl AsRef<Path>,
    shared: &SharedRoot,
) -> Result<PathBuf, AuthorityError> {
    let path = path.as_ref();
    let parent = path
        .parent()
        .ok_or_else(|| AuthorityError::UnboundedRecord {
            path: path.to_path_buf(),
            shared: shared.path.clone(),
        })?;
    let parent = canonical_dir(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| AuthorityError::UnboundedRecord {
            path: path.to_path_buf(),
            shared: shared.path.clone(),
        })?;
    let bounded = parent.join(file_name);
    if !bounded.starts_with(&shared.path) {
        return Err(AuthorityError::UnboundedRecord {
            path: bounded,
            shared: shared.path.clone(),
        });
    }
    Ok(bounded)
}

/// Active checkout root: the authoritative parent workspace root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveRoot {
    path: PathBuf,
}

impl ActiveRoot {
    /// Validate an existing directory as the current active checkout root.
    pub(crate) fn validate(path: impl AsRef<Path>) -> Result<Self, AuthorityError> {
        Ok(Self {
            path: canonical_dir(path)?,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Shared record root: external append-only state visible to all runtimes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedRoot {
    path: PathBuf,
}

impl SharedRoot {
    /// Validate shared state as external to the active checkout.
    pub(crate) fn validate_external(
        path: impl AsRef<Path>,
        active: &ActiveRoot,
    ) -> Result<Self, AuthorityError> {
        let path = canonical_dir(path)?;
        if path == active.path {
            return Err(AuthorityError::SameRoot {
                role: "shared record",
                path,
            });
        }
        if path.starts_with(&active.path) {
            return Err(AuthorityError::NestedRoot {
                role: "shared record",
                path,
                ancestor_role: "active checkout",
                ancestor: active.path.clone(),
            });
        }
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Child worktree root: an ephemeral candidate workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildRoot<Branch> {
    path: PathBuf,
    branch: Branch,
    node_id: String,
}

impl<Branch> ChildRoot<Branch> {
    /// Validate a child root under parent authority.
    ///
    /// This constructor deliberately requires [`Parent`] so ordinary callers
    /// cannot re-label the active checkout or shared record area as a child
    /// workspace.
    pub(crate) fn validate_from_parent(
        parent: &Parent,
        path: impl AsRef<Path>,
        branch: Branch,
        node_id: impl Into<String>,
    ) -> Result<Self, AuthorityError> {
        let path = canonical_dir(path)?;
        if path == parent.active.path {
            return Err(AuthorityError::SameRoot {
                role: "child worktree",
                path,
            });
        }
        if path == parent.shared.path {
            return Err(AuthorityError::SameRoot {
                role: "child worktree",
                path,
            });
        }
        if path.starts_with(&parent.shared.path) {
            return Err(AuthorityError::NestedRoot {
                role: "child worktree",
                path,
                ancestor_role: "shared record",
                ancestor: parent.shared.path.clone(),
            });
        }
        Ok(Self {
            path,
            branch,
            node_id: node_id.into(),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn branch(&self) -> &Branch {
        &self.branch
    }

    pub(crate) fn node_id(&self) -> &str {
        &self.node_id
    }
}

/// Verified active root: active checkout plus binary relation witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedActive {
    active: ActiveRoot,
    binary_path: PathBuf,
}

impl VerifiedActive {
    /// Validate that the running binary belongs to the active checkout root.
    pub(crate) fn validate(
        active: ActiveRoot,
        binary_path: impl AsRef<Path>,
    ) -> Result<Self, AuthorityError> {
        let binary_path = canonical_file(binary_path)?;
        if !binary_path.starts_with(active.path()) {
            return Err(AuthorityError::NestedRoot {
                role: "active binary",
                path: binary_path,
                ancestor_role: "active checkout",
                ancestor: active.path,
            });
        }
        Ok(Self {
            active,
            binary_path,
        })
    }

    pub(crate) fn active(&self) -> &ActiveRoot {
        &self.active
    }

    pub(crate) fn binary_path(&self) -> &Path {
        &self.binary_path
    }
}

/// Completed child attempt: bounded result record written by child authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Completed<Branch> {
    child: ChildRoot<Branch>,
    runtime_id: RuntimeId,
    result_path: PathBuf,
    outcome: AttemptOutcome,
}

impl<Branch> Completed<Branch> {
    pub(crate) fn child(&self) -> &ChildRoot<Branch> {
        &self.child
    }

    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.runtime_id
    }

    pub(crate) fn result_path(&self) -> &Path {
        &self.result_path
    }

    pub(crate) fn outcome(&self) -> AttemptOutcome {
        self.outcome
    }
}

/// Bounded terminal result class for a child attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttemptOutcome {
    Accepted,
    Rejected,
    Failed,
}

/// Selected successor: parent-selected continuation candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selected<Branch> {
    child: ChildRoot<Branch>,
    selected_by: RuntimeId,
}

impl<Branch> Selected<Branch> {
    pub(crate) fn child(&self) -> &ChildRoot<Branch> {
        &self.child
    }

    pub(crate) fn selected_by(&self) -> RuntimeId {
        self.selected_by
    }
}

/// Parent authority: may create, build, spawn, observe, select, update, clean
/// up, and exit.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Parent {
    active: ActiveRoot,
    shared: SharedRoot,
    runtime_id: RuntimeId,
}

impl Parent {
    /// Claim parent authority from a verified active checkout relation.
    pub(crate) fn from_verified(
        verified: VerifiedActive,
        shared: SharedRoot,
        runtime_id: RuntimeId,
    ) -> Self {
        Self {
            active: verified.active,
            shared,
            runtime_id,
        }
    }

    pub(crate) fn active(&self) -> &ActiveRoot {
        &self.active
    }

    pub(crate) fn shared(&self) -> &SharedRoot {
        &self.shared
    }

    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.runtime_id
    }

    /// Mint child authority for one validated child worktree.
    pub(crate) fn child<Branch>(
        &self,
        root: ChildRoot<Branch>,
        runtime_id: RuntimeId,
    ) -> Child<Branch> {
        Child {
            root,
            shared: self.shared.clone(),
            runtime_id,
            acknowledged: false,
        }
    }

    /// Select a completed child attempt as the successor.
    pub(crate) fn select<Branch>(
        &self,
        completed: Completed<Branch>,
    ) -> Result<Selected<Branch>, AuthorityError> {
        if completed.outcome != AttemptOutcome::Accepted {
            return Err(AuthorityError::UnselectableOutcome {
                outcome: completed.outcome,
            });
        }
        Ok(Selected {
            child: completed.child,
            selected_by: self.runtime_id,
        })
    }

    /// Validate the active checkout/binary relation after updating the active
    /// root to the selected successor.
    pub(crate) fn verify_update<Branch>(
        &self,
        _selected: &Selected<Branch>,
        active: ActiveRoot,
        binary_path: impl AsRef<Path>,
    ) -> Result<VerifiedActive, AuthorityError> {
        VerifiedActive::validate(active, binary_path)
    }

    /// Consume child-root evidence after the parent has completed cleanup.
    pub(crate) fn cleaned<Branch>(&self, child: ChildRoot<Branch>) -> Cleaned<Branch> {
        Cleaned { child }
    }

    /// Terminate parent authority without granting further capabilities.
    pub(crate) fn exit(self) -> Exited {
        Exited {
            runtime_id: self.runtime_id,
        }
    }
}

/// Child authority: may acknowledge, self-evaluate, write one bounded result,
/// and exit.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Child<Branch> {
    root: ChildRoot<Branch>,
    shared: SharedRoot,
    runtime_id: RuntimeId,
    acknowledged: bool,
}

impl<Branch> Child<Branch> {
    pub(crate) fn root(&self) -> &ChildRoot<Branch> {
        &self.root
    }

    pub(crate) fn shared(&self) -> &SharedRoot {
        &self.shared
    }

    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.runtime_id
    }

    /// Record that this child has acknowledged its bounded role.
    pub(crate) fn acknowledge(mut self) -> Self {
        self.acknowledged = true;
        self
    }

    /// Complete the child attempt by binding its result path to the shared
    /// record root. Consuming `self` prevents the same child authority token
    /// from writing multiple terminal results.
    pub(crate) fn complete(
        self,
        result_path: impl AsRef<Path>,
        outcome: AttemptOutcome,
    ) -> Result<Completed<Branch>, AuthorityError> {
        if !self.acknowledged {
            return Err(AuthorityError::Unacknowledged { role: "child" });
        }
        Ok(Completed {
            child: self.root,
            runtime_id: self.runtime_id,
            result_path: canonical_record(result_path, &self.shared)?,
            outcome,
        })
    }
}

/// Successor bootstrap authority: may acknowledge handoff, then become parent
/// only after active-root/binary validation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Bootstrap<Branch> {
    selected: Selected<Branch>,
    shared: SharedRoot,
    runtime_id: RuntimeId,
    acknowledged: bool,
}

impl<Branch> Bootstrap<Branch> {
    /// Create successor bootstrap authority from a parent-selected successor.
    pub(crate) fn from_selected(
        selected: Selected<Branch>,
        shared: SharedRoot,
        runtime_id: RuntimeId,
    ) -> Self {
        Self {
            selected,
            shared,
            runtime_id,
            acknowledged: false,
        }
    }

    pub(crate) fn selected(&self) -> &Selected<Branch> {
        &self.selected
    }

    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.runtime_id
    }

    /// Acknowledge the handoff without granting parent authority yet.
    pub(crate) fn acknowledge(mut self) -> Self {
        self.acknowledged = true;
        self
    }

    /// Validate the active checkout/binary relation and promote this
    /// bootstrap authority to parent authority.
    pub(crate) fn become_parent(
        self,
        active: ActiveRoot,
        binary_path: impl AsRef<Path>,
    ) -> Result<Parent, AuthorityError> {
        if !self.acknowledged {
            return Err(AuthorityError::Unacknowledged {
                role: "successor bootstrap",
            });
        }
        let verified = VerifiedActive::validate(active, binary_path)?;
        Ok(Parent::from_verified(
            verified,
            self.shared,
            self.runtime_id,
        ))
    }
}

/// Cleanup witness for an ephemeral child worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cleaned<Branch> {
    child: ChildRoot<Branch>,
}

impl<Branch> Cleaned<Branch> {
    pub(crate) fn child(&self) -> &ChildRoot<Branch> {
        &self.child
    }
}

/// Parent-exit witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Exited {
    runtime_id: RuntimeId,
}

impl Exited {
    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.runtime_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Harness {
        _active_dir: tempfile::TempDir,
        _shared_dir: tempfile::TempDir,
        _child_dir: tempfile::TempDir,
        binary_path: PathBuf,
        result_path: PathBuf,
        parent: Parent,
    }

    impl Harness {
        fn new() -> Self {
            let active_dir = tempfile::tempdir().expect("active dir");
            let shared_dir = tempfile::tempdir().expect("shared dir");
            let child_dir = tempfile::tempdir().expect("child dir");
            let binary_path = active_dir.path().join("ploke-eval");
            fs::write(&binary_path, b"binary").expect("binary");
            let result_path = shared_dir.path().join("result.json");

            let active = ActiveRoot::validate(active_dir.path()).expect("active root");
            let shared =
                SharedRoot::validate_external(shared_dir.path(), &active).expect("shared root");
            let verified = VerifiedActive::validate(active, &binary_path).expect("verified active");
            let parent = Parent::from_verified(verified, shared, RuntimeId::new());

            Self {
                _active_dir: active_dir,
                _shared_dir: shared_dir,
                _child_dir: child_dir,
                binary_path,
                result_path,
                parent,
            }
        }

        fn child_root(&self) -> ChildRoot<String> {
            ChildRoot::validate_from_parent(
                &self.parent,
                self._child_dir.path(),
                "branch-a".to_string(),
                "node-a",
            )
            .expect("child root")
        }
    }

    #[test]
    fn child_must_acknowledge_before_terminal_result() {
        let harness = Harness::new();
        let child = harness.parent.child(harness.child_root(), RuntimeId::new());

        let err = child
            .complete(&harness.result_path, AttemptOutcome::Accepted)
            .expect_err("unacknowledged child cannot complete");

        assert!(matches!(
            err,
            AuthorityError::Unacknowledged { role: "child" }
        ));
    }

    #[test]
    fn parent_can_select_only_accepted_completed_attempts() {
        let harness = Harness::new();
        let child = harness
            .parent
            .child(harness.child_root(), RuntimeId::new())
            .acknowledge();
        let completed = child
            .complete(&harness.result_path, AttemptOutcome::Rejected)
            .expect("bounded result");

        let err = harness
            .parent
            .select(completed)
            .expect_err("rejected child cannot be selected");

        assert!(matches!(
            err,
            AuthorityError::UnselectableOutcome {
                outcome: AttemptOutcome::Rejected
            }
        ));
    }

    #[test]
    fn successor_must_acknowledge_before_becoming_parent() {
        let harness = Harness::new();
        let child = harness
            .parent
            .child(harness.child_root(), RuntimeId::new())
            .acknowledge();
        let completed = child
            .complete(&harness.result_path, AttemptOutcome::Accepted)
            .expect("bounded result");
        let selected = harness.parent.select(completed).expect("selected");
        let bootstrap =
            Bootstrap::from_selected(selected, harness.parent.shared().clone(), RuntimeId::new());
        let active = ActiveRoot::validate(harness._active_dir.path()).expect("active root");

        let err = bootstrap
            .become_parent(active, &harness.binary_path)
            .expect_err("unacknowledged successor cannot become parent");

        assert!(matches!(
            err,
            AuthorityError::Unacknowledged {
                role: "successor bootstrap"
            }
        ));
    }
}
