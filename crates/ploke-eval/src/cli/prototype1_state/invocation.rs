//! Persisted fresh-binary bootstrap contract for prototype1 runtimes.
//!
//! This record is intentionally smaller than a node record or runner request.
//! It is the attempt-scoped overlay a fresh binary needs in order to:
//! - identify which durable node it belongs to
//! - know which authority contract it carries
//! - participate in the current handoff attempt
//!
//! The invocation should not duplicate stable execution context that already
//! belongs to durable node/request state. Doing so would turn this type into a
//! second runner request instead of a true bootstrap contract.
//!
//! The important seam is that Prototype 1 currently has two narrow executable
//! fresh-binary roles:
//!
//! - `Child`: leaf evaluator, executes one node, records, exits
//! - `Successor`: acknowledges successor bootstrap, then idles only within a
//!   bounded standby window
//!
//! Keeping that split explicit here prevents the live runner seam from
//! quietly drifting into a generic "fresh binary can do anything" surface.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::spec::PrepareError;

use super::event::RuntimeId;

fn leaf_runner_argv(invocation_path: &Path) -> Vec<String> {
    vec![
        "loop".to_string(),
        "prototype1-runner".to_string(),
        "--invocation".to_string(),
        invocation_path.display().to_string(),
        "--execute".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ]
}

fn successor_runner_argv(invocation_path: &Path) -> Vec<String> {
    vec![
        "loop".to_string(),
        "prototype1-runner".to_string(),
        "--invocation".to_string(),
        invocation_path.display().to_string(),
        "--execute".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ]
}

/// Durable schema version for runtime invocations.
pub(crate) const SCHEMA_VERSION: &str = "prototype1-invocation.v1";

/// Runtime role for one fresh-binary invocation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    /// Leaf evaluator child: acknowledge, self-evaluate, record, exit.
    Child,
    /// Selected continuation runtime for bounded successor bootstrap.
    Successor,
}

/// Persisted bootstrap record for one fresh prototype1 runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub schema_version: String,
    pub role: Role,
    pub campaign_id: String,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub journal_path: PathBuf,
    pub created_at: String,
}

/// Executable leaf-child invocation.
///
/// This is the only invocation authority the live Prototype 1 runner may
/// execute today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildInvocation {
    inner: Invocation,
}

/// Executable selected-successor bootstrap contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SuccessorInvocation {
    inner: Invocation,
}

/// Classified invocation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InvocationAuthority {
    Child(ChildInvocation),
    Successor(SuccessorInvocation),
}

impl Invocation {
    /// Child-evaluator bootstrap contract.
    fn child(
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            role: Role::Child,
            campaign_id,
            node_id,
            runtime_id,
            journal_path,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Selected-successor bootstrap contract.
    fn successor(
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            role: Role::Successor,
            campaign_id,
            node_id,
            runtime_id,
            journal_path,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Classify the persisted invocation by its runtime authority.
    fn classify(self) -> InvocationAuthority {
        match self.role {
            Role::Child => InvocationAuthority::Child(ChildInvocation { inner: self }),
            Role::Successor => InvocationAuthority::Successor(SuccessorInvocation { inner: self }),
        }
    }
}

impl ChildInvocation {
    /// Create the executable leaf-child invocation used by the live runner.
    pub(crate) fn new(
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
    ) -> Self {
        Self {
            inner: Invocation::child(campaign_id, node_id, runtime_id, journal_path),
        }
    }

    /// Access the persisted wire record.
    pub(crate) fn as_invocation(&self) -> &Invocation {
        &self.inner
    }

    /// Campaign this child leaf run belongs to.
    pub(crate) fn campaign_id(&self) -> &str {
        &self.inner.campaign_id
    }

    /// Durable node this child leaf run evaluates.
    pub(crate) fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Runtime identity for this concrete child attempt.
    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.inner.runtime_id
    }

    /// Shared journal path used for child acknowledgement.
    pub(crate) fn journal_path(&self) -> &Path {
        &self.inner.journal_path
    }

    /// CLI argv for launching exactly one leaf child evaluation.
    pub(crate) fn launch_args(&self, invocation_path: &Path) -> Vec<String> {
        leaf_runner_argv(invocation_path)
    }
}

impl SuccessorInvocation {
    /// Create the executable successor invocation used by the detached
    /// handoff path.
    pub(crate) fn new(
        campaign_id: String,
        node_id: String,
        runtime_id: RuntimeId,
        journal_path: PathBuf,
    ) -> Self {
        Self {
            inner: Invocation::successor(campaign_id, node_id, runtime_id, journal_path),
        }
    }

    /// Access the persisted wire record.
    pub(crate) fn as_invocation(&self) -> &Invocation {
        &self.inner
    }

    /// Campaign this successor bootstrap belongs to.
    pub(crate) fn campaign_id(&self) -> &str {
        &self.inner.campaign_id
    }

    /// Durable node this successor bootstrap belongs to.
    pub(crate) fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Runtime identity for this successor bootstrap attempt.
    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.inner.runtime_id
    }

    /// CLI argv for launching exactly one successor bootstrap.
    pub(crate) fn launch_args(&self, invocation_path: &Path) -> Vec<String> {
        successor_runner_argv(invocation_path)
    }
}

/// Directory containing persisted invocation records for one node.
pub(crate) fn invocations_dir(node_dir: &Path) -> PathBuf {
    node_dir.join("invocations")
}

/// Attempt-scoped invocation path for one concrete runtime.
pub(crate) fn invocation_path(node_dir: &Path, runtime_id: RuntimeId) -> PathBuf {
    invocations_dir(node_dir).join(format!("{runtime_id}.json"))
}

/// Directory containing attempt-scoped result artifacts for one node.
pub(crate) fn results_dir(node_dir: &Path) -> PathBuf {
    node_dir.join("results")
}

/// Attempt-scoped result path for one concrete runtime.
pub(crate) fn result_path(node_dir: &Path, runtime_id: RuntimeId) -> PathBuf {
    results_dir(node_dir).join(format!("{runtime_id}.json"))
}

/// Persist one invocation record.
fn write_invocation(path: &Path, invocation: &Invocation) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(invocation).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

/// Load one persisted invocation record.
pub(crate) fn load(path: &Path) -> Result<Invocation, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })
}

/// Load and classify one persisted invocation by authority.
pub(crate) fn load_authority(path: &Path) -> Result<InvocationAuthority, PrepareError> {
    Ok(load(path)?.classify())
}

/// Load an executable invocation for the live Prototype 1 runner.
pub(crate) fn load_executable(path: &Path) -> Result<InvocationAuthority, PrepareError> {
    load_authority(path)
}

/// Persist one executable leaf-child invocation.
pub(crate) fn write_child_invocation(
    path: &Path,
    invocation: &ChildInvocation,
) -> Result<(), PrepareError> {
    write_invocation(path, invocation.as_invocation())
}

/// Persist one executable branch-successor invocation.
pub(crate) fn write_successor_invocation(
    path: &Path,
    invocation: &SuccessorInvocation,
) -> Result<(), PrepareError> {
    write_invocation(path, invocation.as_invocation())
}

/// Successor-ready acknowledgement written by a detached successor runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SuccessorReadyRecord {
    pub schema_version: String,
    pub campaign_id: String,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub pid: u32,
    pub recorded_at: String,
}

/// Durable schema version for successor-ready acknowledgements.
pub(crate) const SUCCESSOR_READY_SCHEMA_VERSION: &str = "prototype1-successor-ready.v1";

/// Directory containing successor-ready acknowledgements for one node.
pub(crate) fn successor_ready_dir(node_dir: &Path) -> PathBuf {
    node_dir.join("successor-ready")
}

/// Successor-ready acknowledgement path for one concrete runtime.
pub(crate) fn successor_ready_path(node_dir: &Path, runtime_id: RuntimeId) -> PathBuf {
    successor_ready_dir(node_dir).join(format!("{runtime_id}.json"))
}

/// Persist one successor-ready acknowledgement.
pub(crate) fn write_successor_ready_record(
    path: &Path,
    record: &SuccessorReadyRecord,
) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(record).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

/// Load one persisted successor-ready acknowledgement.
pub(crate) fn load_successor_ready_record(
    path: &Path,
) -> Result<SuccessorReadyRecord, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })
}
