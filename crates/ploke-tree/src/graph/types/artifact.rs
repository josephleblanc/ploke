use std::collections::BTreeMap;

use ploke_records::history::{ArtifactRefRecord, TreeKeyHashRecord};
use ploke_records::ids::ArtifactId;

use super::evidence::EvidenceId;

/// Artifact identities observed in History and selection payloads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArtifactIndex {
    pub artifacts: BTreeMap<ArtifactKey, ArtifactNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactNode {
    pub key: ArtifactKey,
    pub identity: ArtifactIdentity,
    pub ids: ArtifactIds,
    pub evidence: Vec<EvidenceId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactIds {
    pub artifact_ids: Vec<ArtifactId>,
    pub artifact_refs: Vec<ArtifactRefRecord>,
    pub tree_keys: Vec<TreeKeyHashRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactKey {
    HistoryRef { value: String },
    PassiveId { value: String },
}

impl ArtifactKey {
    pub(crate) fn from_history_ref(artifact: &ArtifactRefRecord) -> Self {
        Self::HistoryRef {
            value: artifact.value.clone(),
        }
    }

    pub(crate) fn from_passive_id(artifact: &ArtifactId) -> Self {
        Self::PassiveId {
            value: artifact.0.clone(),
        }
    }

    pub fn entity_key(&self) -> &str {
        match self {
            Self::HistoryRef { value } | Self::PassiveId { value } => {
                value.strip_prefix("artifact:").unwrap_or(value)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactIdentity {
    HistoryRef(ArtifactRefRecord),
    PassiveId(ArtifactId),
}

impl ArtifactNode {
    pub fn entity_key(&self) -> &str {
        self.key.entity_key()
    }

    pub fn artifact_ids(&self) -> &[ArtifactId] {
        self.ids.artifact_ids.as_slice()
    }

    pub fn artifact_refs(&self) -> &[ArtifactRefRecord] {
        self.ids.artifact_refs.as_slice()
    }

    pub fn tree_keys(&self) -> &[TreeKeyHashRecord] {
        self.ids.tree_keys.as_slice()
    }
}

impl ArtifactIds {
    pub(crate) fn record_artifact_id(&mut self, artifact_id: ArtifactId) {
        if !self.artifact_ids.contains(&artifact_id) {
            self.artifact_ids.push(artifact_id);
        }
    }

    pub(crate) fn record_artifact_ref(&mut self, artifact_ref: ArtifactRefRecord) {
        if !self.artifact_refs.contains(&artifact_ref) {
            self.artifact_refs.push(artifact_ref);
        }
    }

    pub(crate) fn record_tree_key(&mut self, tree_key: TreeKeyHashRecord) {
        if !self.tree_keys.contains(&tree_key) {
            self.tree_keys.push(tree_key);
        }
    }
}
