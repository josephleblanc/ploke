//! Passive parent identity record.
//!
//! This mirrors the artifact-carried identity JSON. It has no bootstrap,
//! command-validation, filesystem, or runtime authority helpers.

use serde::{Deserialize, Serialize};

use crate::ids::CampaignId;

/// Stable repo-relative path used by current parent identity artifacts.
pub const PARENT_IDENTITY_RELPATH: &str = ".ploke/prototype1/parent_identity.json";

/// Durable schema version for parent identity artifacts.
pub const PARENT_IDENTITY_SCHEMA_VERSION: &str = "prototype1-parent-identity.v1";

/// Parent identity committed into a parent-capable artifact checkout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ParentIdentityRecord {
    pub schema_version: String,
    pub campaign_id: CampaignId,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rejects_unknown_lineage_field() {
        let value = serde_json::json!({
            "schema_version": PARENT_IDENTITY_SCHEMA_VERSION,
            "campaign_id": "campaign-1",
            "parent_id": "node-1",
            "node_id": "node-1",
            "generation": 1,
            "instance_id": "instance-1",
            "previous_parnt_id": "node-0",
            "parent_node_id": "node-0",
            "branch_id": "branch-1",
            "created_at": "2026-07-13T00:00:00Z"
        });

        let error = serde_json::from_value::<ParentIdentityRecord>(value)
            .expect_err("misspelled lineage field must fail");

        assert!(error.to_string().contains("previous_parnt_id"));
    }
}
