//! Passive parent identity record.
//!
//! This mirrors the artifact-carried identity JSON. It has no bootstrap,
//! command-validation, filesystem, or runtime authority helpers.

use serde::{Deserialize, Serialize};

/// Stable repo-relative path used by current parent identity artifacts.
pub const PARENT_IDENTITY_RELPATH: &str = ".ploke/prototype1/parent_identity.json";

/// Durable schema version for parent identity artifacts.
pub const PARENT_IDENTITY_SCHEMA_VERSION: &str = "prototype1-parent-identity.v1";

/// Parent identity committed into a parent-capable artifact checkout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParentIdentityRecord {
    pub schema_version: String,
    pub campaign_id: String,
    pub parent_id: String,
    pub node_id: String,
    pub generation: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node_id: Option<String>,
    pub branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_branch: Option<String>,
    pub created_at: String,
}
