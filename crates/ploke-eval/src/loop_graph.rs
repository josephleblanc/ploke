#![allow(dead_code)] // Forward graph vocabulary; wired into live records incrementally.

//! Graph vocabulary for future loop provenance records.
//!
//! This module carries the durable identities and operation targets needed to
//! describe artifact/runtime generation without collapsing everything into git
//! branch ancestry. The live controller is still narrower than this model, but
//! new persisted records should prefer these carriers when they need to name
//! artifacts, patches, or n-ary patch/artifact inputs.
//!
//! This vocabulary intentionally lives at crate level rather than under the
//! CLI-facing Prototype 1 module. Intervention records, branch registries, and
//! scheduler nodes all need to preserve graph provenance, and those layers
//! should not depend on `crate::cli` just to name artifacts and patches. The
//! first wiring points are:
//!
//! - [`crate::intervention::spec::InterventionSynthesisInput`] and
//!   [`crate::intervention::spec::InterventionApplyOutput`]
//! - [`crate::intervention::branch_registry::record_synthesized_branches`]
//! - [`crate::intervention::scheduler::register_treatment_evaluation_node`]
//!
//! Today many live paths still carry only text-file surface identities or leave
//! runtime coordinates absent. That is deliberate: these types make the
//! provenance slots explicit without pretending the current dirty-worktree
//! prototype already has whole-artifact durability.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Durable identity for one concrete runtime instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct RuntimeId(pub Uuid);

impl RuntimeId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RuntimeId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Durable identity for a recoverable artifact state.
///
/// The backing string is intentionally backend-neutral. It may later contain a
/// git commit id, git tree id, content digest, artifact-manifest id, or another
/// stable backend reference. Dirty worktrees should not be assigned an
/// `ArtifactId` until they have a recoverable identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ArtifactId(String);

impl ArtifactId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ArtifactId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

/// Durable identity for one generated or composed patch.
///
/// A `PatchId` identifies the patch record, not merely a candidate branch name.
/// A composed patch and an LLM-resolved merge patch should receive their own
/// ids even when they reuse input patch content.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct PatchId(String);

impl PatchId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for PatchId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

/// Target operated over by a Runtime.
///
/// The simple patch-generation case targets one Artifact. Patch composition
/// and merge-resolution operations target sets of patches or artifacts while
/// preserving the base Artifact used to interpret the inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub(crate) enum OperationTarget {
    /// One Runtime operates over one durable Artifact surface.
    Artifact { artifact_id: ArtifactId },
    /// One Runtime or deterministic composer operates over patches with a
    /// shared base Artifact.
    PatchSet {
        base_artifact_id: ArtifactId,
        patch_ids: Vec<PatchId>,
    },
    /// One Runtime or merge strategy operates over derived Artifacts, with an
    /// optional common base when it is known.
    ArtifactSet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_artifact_id: Option<ArtifactId>,
        artifact_ids: Vec<ArtifactId>,
    },
}

/// Runtime plus target coordinate for one generative or compositional action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Coordinate {
    pub runtime_id: RuntimeId,
    pub target: OperationTarget,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[test]
    fn operation_target_artifact_serializes_with_named_identity() {
        let target = OperationTarget::Artifact {
            artifact_id: ArtifactId::new("git-tree:abc123"),
        };

        let json = serde_json::to_value(&target).expect("serialize target");

        assert_eq!(
            json,
            serde_json::json!({
                "kind": "artifact",
                "artifact_id": "git-tree:abc123",
            })
        );
    }

    #[test]
    fn operation_target_patch_set_preserves_base_and_inputs() {
        let target = OperationTarget::PatchSet {
            base_artifact_id: ArtifactId::new("artifact:A1"),
            patch_ids: vec![PatchId::new("patch:P1"), PatchId::new("patch:P2")],
        };

        let json = serde_json::to_value(&target).expect("serialize patch set");

        assert_eq!(
            json,
            serde_json::json!({
                "kind": "patch_set",
                "base_artifact_id": "artifact:A1",
                "patch_ids": ["patch:P1", "patch:P2"],
            })
        );
    }

    #[test]
    fn coordinate_binds_runtime_to_operation_target() {
        let runtime_id = RuntimeId(Uuid::nil());
        let coordinate = Coordinate {
            runtime_id,
            target: OperationTarget::ArtifactSet {
                base_artifact_id: None,
                artifact_ids: vec![
                    ArtifactId::new("artifact:A2"),
                    ArtifactId::new("artifact:A3"),
                ],
            },
        };

        let json = serde_json::to_value(&coordinate).expect("serialize coordinate");

        assert_eq!(json["runtime_id"], serde_json::json!(Uuid::nil()));
        assert_eq!(json["target"]["kind"], serde_json::json!("artifact_set"));
        assert_eq!(
            json["target"]["artifact_ids"],
            serde_json::json!(["artifact:A2", "artifact:A3"])
        );
        assert!(json["target"].get("base_artifact_id").is_none());
    }
}
