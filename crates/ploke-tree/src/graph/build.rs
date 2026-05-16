#[path = "build/branch.rs"]
mod branch;
#[path = "build/channel.rs"]
mod channel;
#[path = "build/handoff.rs"]
mod handoff;
#[path = "build/history.rs"]
mod history;
#[path = "build/journal.rs"]
mod journal;
#[path = "build/passive.rs"]
mod passive;
#[path = "build/run_attempts.rs"]
mod run_attempts;
#[path = "build/scheduler.rs"]
mod scheduler;
#[path = "build/selection.rs"]
mod selection;

use std::collections::BTreeMap;

use ploke_records::history::{ActorRefRecord, ArtifactRefRecord, TreeKeyHashRecord};
use ploke_records::ids::{ArtifactId, Coordinate, OperationTarget, RuntimeId};
use ploke_records::scheduler::NodeRecord;

use super::*;
use crate::RunRecordSet;

impl Graph {
    /// Assemble a graph from typed records already loaded by `ploke-tree`.
    pub fn from_records(records: &RunRecordSet) -> Self {
        let mut builder = Builder::default();
        builder.ingest_history(&records.history_blocks);
        builder.ingest_scheduler_records(&records.forest_input);
        builder.ingest_transition_journal(&records.transition_journal);
        builder.ingest_passive_evidence(&records.forest_input.passive_evidence);
        let mut graph = builder.finish();
        graph.forest = Some(crate::RunForest::from_records(records.forest_input.clone()));
        graph
    }
}

#[derive(Default)]
struct Builder {
    graph: Graph,
    next_evidence_id: u64,
    branch_by_node_id: BTreeMap<String, String>,
    artifact_keys_by_entity: BTreeMap<String, Vec<ArtifactKey>>,
    artifact_ids_by_entity: BTreeMap<String, ArtifactIds>,
}

impl Builder {
    fn finish(mut self) -> Graph {
        self.normalize_history_order();
        self.graph
    }

    fn observe_actor_ref(&mut self, actor: &ActorRefRecord) {
        if let ActorRefRecord::Runtime(runtime_id) = actor {
            self.observe_runtime(runtime_id);
        }
    }

    fn observe_runtime(&mut self, runtime_id: &RuntimeId) {
        self.graph
            .runtimes
            .runtimes
            .entry(runtime_id.clone())
            .or_insert_with(|| RuntimeNode {
                runtime_id: runtime_id.clone(),
                evidence: Vec::new(),
            });
    }

    fn observe_artifact_ref(&mut self, artifact: &ArtifactRefRecord) {
        let key = artifact_ref_key(artifact);
        self.graph
            .artifacts
            .artifacts
            .entry(key.clone())
            .or_insert_with(|| ArtifactNode {
                key,
                identity: ArtifactIdentity::HistoryRef(artifact.clone()),
                ids: ArtifactIds::default(),
                evidence: Vec::new(),
            });
        let entity_key = artifact_entity_key(artifact);
        self.register_artifact_entity_key(&entity_key, artifact_ref_key(artifact));
        self.artifact_ids_by_entity
            .entry(entity_key.clone())
            .or_default()
            .record_artifact_ref(artifact.clone());
        self.sync_artifact_entity_ids(&entity_key);
    }

    fn observe_artifact_id(&mut self, artifact: &ArtifactId) {
        let key = ArtifactKey::from_passive_id(artifact);
        self.graph
            .artifacts
            .artifacts
            .entry(key.clone())
            .or_insert_with(|| ArtifactNode {
                key,
                identity: ArtifactIdentity::PassiveId(artifact.clone()),
                ids: ArtifactIds::default(),
                evidence: Vec::new(),
            });
        let entity_key = artifact
            .0
            .strip_prefix("artifact:")
            .unwrap_or(artifact.0.as_str())
            .to_owned();
        self.register_artifact_entity_key(&entity_key, ArtifactKey::from_passive_id(artifact));
        self.artifact_ids_by_entity
            .entry(entity_key.clone())
            .or_default()
            .record_artifact_id(artifact.clone());
        self.sync_artifact_entity_ids(&entity_key);
    }

    fn observe_artifact_tree_key(
        &mut self,
        artifact: &ArtifactRefRecord,
        tree_key: &TreeKeyHashRecord,
    ) {
        self.observe_artifact_ref(artifact);
        let entity_key = artifact_entity_key(artifact);
        self.artifact_ids_by_entity
            .entry(entity_key.clone())
            .or_default()
            .record_tree_key(tree_key.clone());
        self.sync_artifact_entity_ids(&entity_key);
    }

    fn attach_to_branch(&mut self, branch_id: &str, evidence_id: EvidenceId) {
        for branch in &mut self.graph.candidates.branches {
            if branch.branch_id == branch_id {
                branch.evidence.push(evidence_id);
            }
        }
        for candidate in &mut self.graph.candidates.candidates {
            if candidate.branch_id.as_deref() == Some(branch_id) {
                candidate.evidence.push(evidence_id);
            }
        }
    }

    fn observe_node_branch(&mut self, node_id: &str, branch_id: &str) {
        self.branch_by_node_id
            .entry(node_id.to_owned())
            .or_insert_with(|| branch_id.to_owned());
    }

    fn observe_scheduler_branch(&mut self, node: &NodeRecord) {
        if let Some(branch) = self
            .graph
            .candidates
            .branches
            .iter_mut()
            .find(|branch| branch.branch_id == node.branch_id.0)
        {
            if branch.candidate_id.is_none() {
                branch.candidate_id = Some(node.candidate_id.clone());
            }
            if branch.source_state_id.is_none() {
                branch.source_state_id = Some(node.source_state_id.0.clone());
            }
            if branch.parent_branch_id.is_none() {
                branch.parent_branch_id = node.parent_branch_id.as_ref().map(|id| id.0.clone());
            }
            if branch.base_artifact_id.is_none() {
                branch.base_artifact_id = node.base_artifact_id.clone();
            }
            if branch.derived_artifact_id.is_none() {
                branch.derived_artifact_id = node.derived_artifact_id.clone();
            }
            if branch.patch_id.is_none() {
                branch.patch_id = node.patch_id.clone();
            }
        }
    }

    fn attach_to_node_branch(&mut self, node_id: &str, evidence_id: EvidenceId) {
        if let Some(branch_id) = self.branch_by_node_id.get(node_id).cloned() {
            self.attach_to_branch(&branch_id, evidence_id);
        }
    }

    fn attach_to_runtime(&mut self, runtime_id: &RuntimeId, evidence_id: EvidenceId) {
        self.observe_runtime(runtime_id);
        if let Some(runtime) = self.graph.runtimes.runtimes.get_mut(runtime_id) {
            runtime.evidence.push(evidence_id);
        }
    }

    fn attach_to_artifact_id(&mut self, artifact_id: &ArtifactId, evidence_id: EvidenceId) {
        self.observe_artifact_id(artifact_id);
        let key = ArtifactKey::from_passive_id(artifact_id);
        if let Some(artifact) = self.graph.artifacts.artifacts.get_mut(&key) {
            artifact.evidence.push(evidence_id);
        }
    }

    fn observe_operation_coordinate(&mut self, coordinate: &Coordinate) {
        self.observe_runtime(&coordinate.runtime_id);
        self.observe_operation_target(&coordinate.target);

        let key = OperationKey::from_coordinate(coordinate);
        self.graph
            .operations
            .operations
            .entry(key.clone())
            .and_modify(|operation| {
                if operation.coordinate.is_none() {
                    operation.coordinate = Some(coordinate.clone());
                }
            })
            .or_insert_with(|| OperationNode {
                key,
                coordinate: Some(coordinate.clone()),
                evidence: Vec::new(),
            });
    }

    fn attach_to_operation_coordinate(&mut self, coordinate: &Coordinate, evidence_id: EvidenceId) {
        self.observe_operation_coordinate(coordinate);
        let key = OperationKey::from_coordinate(coordinate);
        if let Some(operation) = self.graph.operations.operations.get_mut(&key) {
            operation.evidence.push(evidence_id);
        }
    }

    fn observe_operation_target(&mut self, target: &OperationTarget) {
        match target {
            OperationTarget::Artifact { artifact_id } => {
                self.observe_artifact_id(artifact_id);
            }
            OperationTarget::PatchSet {
                base_artifact_id, ..
            } => {
                self.observe_artifact_id(base_artifact_id);
            }
            OperationTarget::ArtifactSet {
                base_artifact_id,
                artifact_ids,
            } => {
                if let Some(base_artifact_id) = base_artifact_id {
                    self.observe_artifact_id(base_artifact_id);
                }
                for artifact_id in artifact_ids {
                    self.observe_artifact_id(artifact_id);
                }
            }
        }
    }

    fn attach_evidence(
        &mut self,
        subject: EvidenceSubject,
        kind: EvidenceKind,
        refs: Vec<ploke_records::history::EvidenceRefRecord>,
    ) -> EvidenceId {
        self.attach_located_evidence(
            subject,
            kind,
            refs.into_iter().map(EvidenceLocator::HistoryRef).collect(),
        )
    }

    fn attach_located_evidence(
        &mut self,
        subject: EvidenceSubject,
        kind: EvidenceKind,
        locators: Vec<EvidenceLocator>,
    ) -> EvidenceId {
        let id = EvidenceId(self.next_evidence_id);
        self.next_evidence_id += 1;
        self.graph.evidence.attachments.insert(
            id,
            EvidenceAttachment {
                id,
                subject,
                kind,
                locators,
            },
        );
        id
    }

    fn warn(&mut self, kind: GraphWarningKind, detail: String) {
        self.graph.warnings.push(GraphWarning { kind, detail });
    }

    fn register_artifact_entity_key(&mut self, entity_key: &str, artifact_key: ArtifactKey) {
        let artifact_keys = self
            .artifact_keys_by_entity
            .entry(entity_key.to_owned())
            .or_default();
        if !artifact_keys.contains(&artifact_key) {
            artifact_keys.push(artifact_key);
        }
    }

    fn sync_artifact_entity_ids(&mut self, entity_key: &str) {
        let Some(artifact_ids) = self.artifact_ids_by_entity.get(entity_key).cloned() else {
            return;
        };
        let Some(artifact_keys) = self.artifact_keys_by_entity.get(entity_key).cloned() else {
            return;
        };
        for artifact_key in artifact_keys {
            if let Some(node) = self.graph.artifacts.artifacts.get_mut(&artifact_key) {
                node.ids = artifact_ids.clone();
            }
        }
    }
}

fn artifact_ref_key(artifact: &ArtifactRefRecord) -> ArtifactKey {
    ArtifactKey::from_history_ref(artifact)
}

fn artifact_entity_key(artifact: &ArtifactRefRecord) -> String {
    artifact.graph_entity_key().to_owned()
}

#[cfg(test)]
mod tests {
    use ploke_records::history::{ArtifactRefRecord, TreeKeyHashRecord};
    use ploke_records::ids::{ArtifactId, HistoryHash};

    use super::Builder;
    use crate::graph::ArtifactKey;

    #[test]
    fn artifact_nodes_share_reconciled_ids_by_entity_key() {
        let mut builder = Builder::default();
        let history_ref = ArtifactRefRecord::from_artifact_id(ArtifactId("after".to_owned()));
        let passive_id = ArtifactId("after".to_owned());
        let tree_key = TreeKeyHashRecord {
            hash: HistoryHash("tree:after".to_owned()),
        };

        builder.observe_artifact_ref(&history_ref);
        builder.observe_artifact_id(&passive_id);
        builder.observe_artifact_tree_key(&history_ref, &tree_key);

        let history_node = builder
            .graph
            .artifacts
            .artifacts
            .get(&ArtifactKey::HistoryRef {
                id: history_ref.id().0.clone(),
            })
            .expect("history artifact node");
        let passive_node = builder
            .graph
            .artifacts
            .artifacts
            .get(&ArtifactKey::PassiveId {
                value: passive_id.0.clone(),
            })
            .expect("passive artifact node");

        assert_eq!(history_node.entity_key(), "after");
        assert_eq!(passive_node.entity_key(), "after");
        assert_eq!(history_node.artifact_ids(), std::slice::from_ref(&passive_id));
        assert_eq!(passive_node.artifact_ids(), std::slice::from_ref(&passive_id));
        assert_eq!(history_node.artifact_refs(), std::slice::from_ref(&history_ref));
        assert_eq!(passive_node.artifact_refs(), std::slice::from_ref(&history_ref));
        assert_eq!(history_node.tree_keys(), std::slice::from_ref(&tree_key));
        assert_eq!(passive_node.tree_keys(), &[tree_key]);
    }
}
