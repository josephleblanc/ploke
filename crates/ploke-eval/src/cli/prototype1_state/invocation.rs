//! Persisted fresh-binary bootstrap contract for prototype1 runtimes.
//!
//! This record is intentionally smaller than a node record or runner request.
//! It is the attempt-scoped overlay a fresh binary needs in order to:
//! - identify which durable node it belongs to
//! - know which role it should play
//! - participate in the current handoff attempt
//!
//! The invocation should not duplicate stable execution context that already
//! belongs to durable node/request state. Doing so would turn this type into a
//! second runner request instead of a true bootstrap contract.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::spec::PrepareError;

use super::event::RuntimeId;

/// Durable schema version for runtime invocations.
pub(crate) const SCHEMA_VERSION: &str = "prototype1-invocation.v1";

/// Runtime role for one fresh-binary invocation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    /// Leaf evaluator child: acknowledge, self-evaluate, record, exit.
    Child,
    /// Selected continuation runtime that becomes the next active authority.
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

impl Invocation {
    /// Child-evaluator bootstrap contract.
    pub(crate) fn child(
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
pub(crate) fn write_invocation(path: &Path, invocation: &Invocation) -> Result<(), PrepareError> {
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

/// CLI argv for launching a fresh binary from one invocation record.
pub(crate) fn launch_args(invocation_path: &Path) -> Vec<String> {
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
