use std::collections::BTreeMap;

use ploke_records::ids::RuntimeId;

use super::evidence::EvidenceId;

/// Runtime identities observed in actors, environments, and selected successors.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeIndex {
    pub runtimes: BTreeMap<RuntimeId, RuntimeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeNode {
    pub runtime_id: RuntimeId,
    pub evidence: Vec<EvidenceId>,
}
