use std::collections::BTreeMap;

use ploke_records::history::ArtifactRefRecord;
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
    pub evidence: Vec<EvidenceId>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactIdentity {
    HistoryRef(ArtifactRefRecord),
    PassiveId(ArtifactId),
}
