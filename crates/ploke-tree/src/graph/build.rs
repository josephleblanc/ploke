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

use ploke_records::history::{ActorRefRecord, ArtifactRefRecord};
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
        builder.finish()
    }
}

#[derive(Default)]
struct Builder {
    graph: Graph,
    next_evidence_id: u64,
    branch_by_node_id: BTreeMap<String, String>,
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
                evidence: Vec::new(),
            });
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
                evidence: Vec::new(),
            });
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
}

fn artifact_ref_key(artifact: &ArtifactRefRecord) -> ArtifactKey {
    ArtifactKey::from_history_ref(artifact)
}
