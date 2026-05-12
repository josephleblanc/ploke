use std::collections::BTreeMap;

use ploke_records::scheduler::NodeRecord;

use crate::RunForestInput;
use crate::graph::{EvidenceKind, EvidenceLocator, EvidenceSubject};

use super::Builder;

impl Builder {
    pub(super) fn ingest_scheduler_records(&mut self, input: &RunForestInput) {
        self.attach_located_evidence(
            EvidenceSubject::SchedulerCampaign(input.scheduler.campaign_id.to_string()),
            EvidenceKind::SchedulerStateSummary,
            vec![EvidenceLocator::SchedulerState],
        );

        let nodes = merged_nodes(&input.scheduler.nodes, &input.node_records);
        let mut branch_by_node_id = BTreeMap::new();
        for node in nodes.values() {
            branch_by_node_id.insert(node.node_id.to_string(), node.branch_id.to_string());
            self.ingest_scheduler_node(node);
        }

        if let Some(parent) = input.parent_identity.as_ref() {
            self.ingest_parent_identity(parent);
        }

        for ready in &input.successor_ready {
            let evidence_id = self.ingest_successor_ready(ready);
            if let Some(branch_id) = branch_by_node_id.get(&ready.node_id) {
                self.attach_to_branch(branch_id, evidence_id);
            }
        }

        for completion in &input.successor_completion {
            let evidence_id = self.ingest_successor_completion(completion);
            if let Some(branch_id) = branch_by_node_id.get(&completion.node_id) {
                self.attach_to_branch(branch_id, evidence_id);
            }
        }
    }

    fn ingest_scheduler_node(&mut self, node: &NodeRecord) {
        self.observe_node_branch(node.node_id.as_str(), node.branch_id.as_str());
        self.observe_scheduler_branch(node);
        let evidence_id = self.attach_located_evidence(
            EvidenceSubject::SchedulerNode(node.node_id.to_string()),
            EvidenceKind::SchedulerNodeRecord,
            vec![EvidenceLocator::SchedulerNode {
                node_id: node.node_id.to_string(),
            }],
        );

        self.attach_to_branch(node.branch_id.as_str(), evidence_id);
        if let Some(base_artifact_id) = node.base_artifact_id.as_ref() {
            self.attach_to_artifact_id(base_artifact_id, evidence_id);
        }
        if let Some(derived_artifact_id) = node.derived_artifact_id.as_ref() {
            self.attach_to_artifact_id(derived_artifact_id, evidence_id);
        }
    }
}

fn merged_nodes<'a>(
    scheduler_nodes: &'a [NodeRecord],
    node_records: &'a [NodeRecord],
) -> BTreeMap<String, &'a NodeRecord> {
    let mut nodes = BTreeMap::new();
    for node in scheduler_nodes {
        nodes.insert(node.node_id.to_string(), node);
    }
    for node in node_records {
        nodes.insert(node.node_id.to_string(), node);
    }
    nodes
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ploke_records::identity::ParentIdentityRecord;
    use ploke_records::ids::{
        ArtifactId, BranchId, CampaignId, CandidateId, InstanceId, RuntimeId, SchedulerNodeId,
        SourceStateId,
    };
    use ploke_records::invocation::{
        SuccessorCompletionRecord, SuccessorCompletionStatus, SuccessorReadyRecord,
    };
    use ploke_records::scheduler::{NodeRecord, NodeStatusRecord, SchedulerStateRecord};

    use crate::graph::{ArtifactKey, EvidenceKind, EvidenceSubject, Graph};
    use crate::{RunForestInput, RunRecordSet, TransitionJournal};

    #[test]
    fn graph_reaches_scheduler_handoff_and_successor_records() {
        let runtime_id = RuntimeId("runtime-1".to_owned());
        let records = RunRecordSet {
            forest_input: RunForestInput {
                scheduler: SchedulerStateRecord {
                    schema_version: "prototype1-scheduler.v1".to_owned(),
                    campaign_id: CampaignId("campaign-1".to_owned()),
                    updated_at: "2026-05-11T00:00:00Z".to_owned(),
                    policy: Default::default(),
                    frontier_node_ids: Vec::new(),
                    completed_node_ids: vec![SchedulerNodeId("node-1".to_owned())],
                    failed_node_ids: Vec::new(),
                    last_continuation_decision: None,
                    nodes: vec![node_record()],
                },
                node_records: Vec::new(),
                parent_identity: Some(ParentIdentityRecord {
                    schema_version: "prototype1-parent-identity.v1".to_owned(),
                    campaign_id: "campaign-1".to_owned(),
                    parent_id: "parent-1".to_owned(),
                    node_id: "node-1".to_owned(),
                    generation: 1,
                    instance_id: Some("instance-1".to_owned()),
                    previous_parent_id: None,
                    parent_node_id: None,
                    branch_id: "branch-1".to_owned(),
                    artifact_branch: Some("prototype1-parent-1".to_owned()),
                    created_at: "2026-05-11T00:01:00Z".to_owned(),
                }),
                successor_ready: vec![SuccessorReadyRecord {
                    schema_version: "prototype1-successor-ready.v1".to_owned(),
                    campaign_id: "campaign-1".to_owned(),
                    node_id: "node-1".to_owned(),
                    runtime_id: runtime_id.clone(),
                    pid: 42,
                    recorded_at: "2026-05-11T00:02:00Z".to_owned(),
                }],
                successor_completion: vec![SuccessorCompletionRecord {
                    schema_version: "prototype1-successor-completion.v1".to_owned(),
                    campaign_id: "campaign-1".to_owned(),
                    node_id: "node-1".to_owned(),
                    runtime_id: runtime_id.clone(),
                    status: SuccessorCompletionStatus::Succeeded,
                    trace_path: None,
                    detail: None,
                    recorded_at: "2026-05-11T00:03:00Z".to_owned(),
                }],
                passive_evidence: Default::default(),
            },
            history_blocks: Vec::new(),
            transition_journal: TransitionJournal::default(),
        };

        let graph = Graph::from_records(&records);

        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::SchedulerStateSummary
                && evidence.subject == EvidenceSubject::SchedulerCampaign("campaign-1".to_owned())
        }));
        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::SchedulerNodeRecord
                && evidence.subject == EvidenceSubject::SchedulerNode("node-1".to_owned())
        }));
        assert!(
            graph
                .evidence
                .attachments
                .values()
                .any(|evidence| evidence.kind == EvidenceKind::ParentIdentity)
        );

        let runtime = graph
            .runtimes
            .runtimes
            .get(&runtime_id)
            .expect("successor runtime is graph-reachable");
        assert!(
            runtime
                .evidence
                .iter()
                .any(|id| { graph.evidence.attachments[id].kind == EvidenceKind::SuccessorReady })
        );
        assert!(runtime.evidence.iter().any(|id| {
            graph.evidence.attachments[id].kind == EvidenceKind::SuccessorCompletion
        }));

        let derived_artifact = graph
            .artifacts
            .artifacts
            .get(&ArtifactKey::PassiveId {
                value: "artifact-after".to_owned(),
            })
            .expect("derived artifact is graph-reachable");
        assert!(derived_artifact.iter_evidence_kind(&graph, EvidenceKind::SchedulerNodeRecord));
    }

    fn node_record() -> NodeRecord {
        NodeRecord {
            schema_version: "prototype1-treatment-node.v1".to_owned(),
            node_id: SchedulerNodeId("node-1".to_owned()),
            parent_node_id: None,
            generation: 1,
            instance_id: InstanceId("instance-1".to_owned()),
            source_state_id: SourceStateId("source-1".to_owned()),
            operation_target: None,
            base_artifact_id: Some(ArtifactId("artifact-before".to_owned())),
            patch_id: None,
            derived_artifact_id: Some(ArtifactId("artifact-after".to_owned())),
            parent_branch_id: None,
            branch_id: BranchId("branch-1".to_owned()),
            candidate_id: CandidateId("candidate-1".to_owned()),
            target_relpath: PathBuf::from("src/lib.rs"),
            node_dir: PathBuf::from("nodes/node-1"),
            workspace_root: PathBuf::from("worktree"),
            binary_path: PathBuf::from("target/debug/ploke"),
            runner_request_path: PathBuf::from("runner-request.json"),
            runner_result_path: PathBuf::from("runner-result.json"),
            status: NodeStatusRecord::Succeeded,
            created_at: "2026-05-11T00:00:00Z".to_owned(),
            updated_at: "2026-05-11T00:01:00Z".to_owned(),
        }
    }

    trait ArtifactEvidenceExt {
        fn iter_evidence_kind(&self, graph: &Graph, kind: EvidenceKind) -> bool;
    }

    impl ArtifactEvidenceExt for crate::graph::ArtifactNode {
        fn iter_evidence_kind(&self, graph: &Graph, kind: EvidenceKind) -> bool {
            self.evidence
                .iter()
                .any(|id| graph.evidence.attachments[id].kind == kind)
        }
    }
}
