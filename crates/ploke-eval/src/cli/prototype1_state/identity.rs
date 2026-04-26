//! Artifact-carried parent identity for Prototype 1.
//!
//! This file is committed into every parent-capable Artifact at a stable path.
//! A Runtime hydrated from that checkout reads this record to learn which
//! Parent it is, instead of treating command-line arguments as identity.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::intervention::Prototype1NodeRecord;
use crate::spec::PrepareError;

/// Stable repo-relative path for the parent identity artifact.
pub(crate) const PARENT_IDENTITY_RELPATH: &str = ".ploke/prototype1/parent_identity.json";

/// Durable schema version for parent identity artifacts.
pub(crate) const PARENT_IDENTITY_SCHEMA_VERSION: &str = "prototype1-parent-identity.v1";

/// Parent identity committed into a parent Artifact checkout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ParentIdentity {
    pub schema_version: String,
    pub campaign_id: String,
    pub parent_id: String,
    pub node_id: String,
    pub generation: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node_id: Option<String>,
    pub branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_branch: Option<String>,
    pub created_at: String,
}

impl ParentIdentity {
    /// Construct identity from the scheduler node mirror.
    pub(crate) fn from_node(
        campaign_id: impl Into<String>,
        node: &Prototype1NodeRecord,
        previous_parent: Option<&ParentIdentity>,
        artifact_branch: Option<String>,
    ) -> Self {
        Self {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: campaign_id.into(),
            parent_id: node.node_id.clone(),
            node_id: node.node_id.clone(),
            generation: node.generation,
            previous_parent_id: previous_parent.map(|identity| identity.parent_id.clone()),
            parent_node_id: node.parent_node_id.clone(),
            branch_id: node.branch_id.clone(),
            artifact_branch,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Validate this identity against the active campaign/node expectation.
    pub(crate) fn validate_for_command(
        &self,
        campaign_id: &str,
        command_node_id: Option<&str>,
    ) -> Result<(), PrepareError> {
        if self.schema_version != PARENT_IDENTITY_SCHEMA_VERSION {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "parent identity schema '{}' is not supported",
                    self.schema_version
                ),
            });
        }
        if self.campaign_id != campaign_id {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "parent identity campaign '{}' does not match command campaign '{}'",
                    self.campaign_id, campaign_id
                ),
            });
        }
        if let Some(node_id) = command_node_id {
            if self.node_id != node_id {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "parent identity node '{}' does not match command node '{}'",
                        self.node_id, node_id
                    ),
                });
            }
        }
        Ok(())
    }
}

/// Repo-relative parent identity path.
pub(crate) fn parent_identity_relpath() -> PathBuf {
    PathBuf::from(PARENT_IDENTITY_RELPATH)
}

/// Absolute parent identity path for a checkout root.
pub(crate) fn parent_identity_path(repo_root: &Path) -> PathBuf {
    repo_root.join(PARENT_IDENTITY_RELPATH)
}

/// Canonical commit message for admitting a parent-capable Artifact.
pub(crate) fn parent_identity_commit_message(identity: &ParentIdentity) -> String {
    format!(
        "prototype1: initializing gen {} parent {}",
        identity.generation, identity.parent_id
    )
}

/// Load a parent identity from a checkout root.
pub(crate) fn load_parent_identity(repo_root: &Path) -> Result<ParentIdentity, PrepareError> {
    let path = parent_identity_path(repo_root);
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest { path, source })
}

/// Load a parent identity when present.
pub(crate) fn load_parent_identity_optional(
    repo_root: &Path,
) -> Result<Option<ParentIdentity>, PrepareError> {
    let path = parent_identity_path(repo_root);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|source| PrepareError::ParseManifest { path, source }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(PrepareError::ReadManifest { path, source }),
    }
}

/// Write a parent identity into a checkout root without committing it.
pub(crate) fn write_parent_identity(
    repo_root: &Path,
    identity: &ParentIdentity,
) -> Result<PathBuf, PrepareError> {
    let path = parent_identity_path(repo_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(identity).map_err(PrepareError::Serialize)?;
    fs::write(&path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> ParentIdentity {
        ParentIdentity {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: "campaign-1".to_string(),
            parent_id: "node-1".to_string(),
            node_id: "node-1".to_string(),
            generation: 0,
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: "branch-1".to_string(),
            artifact_branch: Some("prototype1-parent-0".to_string()),
            created_at: "2026-04-26T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn validates_matching_command_identity() {
        identity()
            .validate_for_command("campaign-1", Some("node-1"))
            .expect("matching identity");
    }

    #[test]
    fn rejects_mismatched_command_identity() {
        let err = identity()
            .validate_for_command("campaign-1", Some("node-2"))
            .expect_err("mismatched node should reject");
        assert!(err.to_string().contains("does not match command node"));
    }
}
