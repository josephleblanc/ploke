use std::collections::BTreeMap;

use ploke_records::ids::{ArtifactId, Coordinate, EntryId, OperationTarget, PatchId, RuntimeId};

use super::evidence::EvidenceId;

/// Operations are runtime-target actions, distinct from History admission.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OperationIndex {
    pub operations: BTreeMap<OperationKey, OperationNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationKey {
    /// Operation facts admitted through one History entry.
    HistoryEntry { entry_id: EntryId },
    /// Operation facts observed before or outside History admission.
    RuntimeTarget {
        runtime_id: RuntimeId,
        target: OperationTargetKey,
    },
}

impl OperationKey {
    pub fn from_coordinate(coordinate: &Coordinate) -> Self {
        Self::RuntimeTarget {
            runtime_id: coordinate.runtime_id.clone(),
            target: OperationTargetKey::from(&coordinate.target),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationTargetKey {
    Artifact {
        artifact_id: ArtifactId,
    },
    PatchSet {
        base_artifact_id: ArtifactId,
        patch_ids: Vec<PatchId>,
    },
    ArtifactSet {
        base_artifact_id: Option<ArtifactId>,
        artifact_ids: Vec<ArtifactId>,
    },
}

impl From<&OperationTarget> for OperationTargetKey {
    fn from(target: &OperationTarget) -> Self {
        match target {
            OperationTarget::Artifact { artifact_id } => Self::Artifact {
                artifact_id: artifact_id.clone(),
            },
            OperationTarget::PatchSet {
                base_artifact_id,
                patch_ids,
            } => Self::PatchSet {
                base_artifact_id: base_artifact_id.clone(),
                patch_ids: patch_ids.clone(),
            },
            OperationTarget::ArtifactSet {
                base_artifact_id,
                artifact_ids,
            } => Self::ArtifactSet {
                base_artifact_id: base_artifact_id.clone(),
                artifact_ids: artifact_ids.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationNode {
    pub key: OperationKey,
    pub coordinate: Option<Coordinate>,
    pub evidence: Vec<EvidenceId>,
}
