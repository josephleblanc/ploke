use std::collections::BTreeMap;

use ploke_records::ids::Coordinate;
use ploke_records::invocation::InvocationRecord;
use ploke_records::scheduler::{NodeRecord, RunnerRequestRecord, RunnerResultRecord};

use crate::RunAttemptEvidence;
use crate::graph::{EvidenceKind, EvidenceLocator, EvidenceSubject};

use super::Builder;

impl Builder {
    pub(super) fn ingest_run_attempts(&mut self, evidence: &RunAttemptEvidence) {
        for (path, request) in &evidence.runner_requests {
            self.observe_runner_request_metadata(path, request);
        }

        for (path, result) in &evidence.runner_results {
            self.observe_runner_result_metadata(path, result);
        }

        for (path, invocation) in &evidence.invocations {
            self.observe_invocation_metadata(path, invocation);
        }
    }

    pub(super) fn ingest_attempt_runner_results(
        &mut self,
        results: &BTreeMap<String, RunnerResultRecord>,
    ) {
        for (path, result) in results {
            self.observe_attempt_runner_result_metadata(path, result);
        }
    }

    fn observe_invocation_metadata(&mut self, path: &str, invocation: &InvocationRecord) {
        self.observe_runtime(&invocation.runtime_id);
        let runtime_evidence_id = self.attach_located_evidence(
            EvidenceSubject::Runtime(invocation.runtime_id.clone()),
            EvidenceKind::CandidatePayload,
            invocation_source_locators(path, invocation),
        );
        self.attach_to_runtime(&invocation.runtime_id, runtime_evidence_id);

        if let Some(node) = invocation.node.as_ref() {
            self.observe_node_artifacts(node);
            if let Some(target) = node.operation_target.as_ref() {
                let coordinate = Coordinate {
                    runtime_id: invocation.runtime_id.clone(),
                    target: target.clone(),
                };
                self.attach_to_operation_coordinate(&coordinate, runtime_evidence_id);
            }
        }

        if let Some(request) = invocation.request.as_ref() {
            self.observe_runner_request_metadata(path, request);
            if let Some(target) = request.operation_target.as_ref() {
                let coordinate = Coordinate {
                    runtime_id: invocation.runtime_id.clone(),
                    target: target.clone(),
                };
                self.attach_to_operation_coordinate(&coordinate, runtime_evidence_id);
            }
        }
    }

    fn observe_runner_request_metadata(&mut self, path: &str, request: &RunnerRequestRecord) {
        if let Some(base_artifact_id) = request.base_artifact_id.as_ref() {
            self.observe_artifact_id(base_artifact_id);
        }
        if let Some(derived_artifact_id) = request.derived_artifact_id.as_ref() {
            self.observe_artifact_id(derived_artifact_id);
        }
        if let Some(target) = request.operation_target.as_ref() {
            self.observe_operation_target(target);
        }

        let evidence_id = self.attach_located_evidence(
            EvidenceSubject::Branch(request.branch_id.0.clone()),
            EvidenceKind::CandidatePayload,
            runner_request_locators(path, request),
        );
        self.attach_to_branch(request.branch_id.0.as_str(), evidence_id);
        if let Some(base_artifact_id) = request.base_artifact_id.as_ref() {
            self.attach_to_artifact_id(base_artifact_id, evidence_id);
        }
        if let Some(derived_artifact_id) = request.derived_artifact_id.as_ref() {
            self.attach_to_artifact_id(derived_artifact_id, evidence_id);
        }
    }

    fn observe_runner_result_metadata(&mut self, path: &str, result: &RunnerResultRecord) {
        let evidence_id = self.attach_located_evidence(
            EvidenceSubject::Branch(result.branch_id.0.clone()),
            EvidenceKind::CandidateEvaluation,
            runner_result_locators(path, result),
        );
        self.attach_to_branch(result.branch_id.0.as_str(), evidence_id);
    }

    fn observe_attempt_runner_result_metadata(&mut self, path: &str, result: &RunnerResultRecord) {
        let evidence_id = self.attach_located_evidence(
            EvidenceSubject::Branch(result.branch_id.0.clone()),
            EvidenceKind::CandidateEvaluation,
            attempt_runner_result_locators(path, result),
        );
        self.attach_to_branch(result.branch_id.0.as_str(), evidence_id);
    }

    fn observe_node_artifacts(&mut self, node: &NodeRecord) {
        if let Some(base_artifact_id) = node.base_artifact_id.as_ref() {
            self.observe_artifact_id(base_artifact_id);
        }
        if let Some(derived_artifact_id) = node.derived_artifact_id.as_ref() {
            self.observe_artifact_id(derived_artifact_id);
        }
        if let Some(target) = node.operation_target.as_ref() {
            self.observe_operation_target(target);
        }
    }
}

fn runner_request_locators(path: &str, request: &RunnerRequestRecord) -> Vec<EvidenceLocator> {
    node_scoped_locators(path, request.node_id.0.as_str(), "passive_runner_request")
}

fn runner_result_locators(path: &str, result: &RunnerResultRecord) -> Vec<EvidenceLocator> {
    let mut locators =
        node_scoped_locators(path, result.node_id.0.as_str(), "passive_runner_result");
    if let Some(path) = result.evaluation_artifact_path.as_ref() {
        locators.push(EvidenceLocator::EvaluationArtifact { path: path.clone() });
    }
    locators
}

fn attempt_runner_result_locators(path: &str, result: &RunnerResultRecord) -> Vec<EvidenceLocator> {
    let mut locators = node_scoped_locators(
        path,
        result.node_id.0.as_str(),
        "passive_attempt_runner_result",
    );
    if let Some(path) = result.evaluation_artifact_path.as_ref() {
        locators.push(EvidenceLocator::EvaluationArtifact { path: path.clone() });
    }
    locators
}

fn invocation_source_locators(path: &str, invocation: &InvocationRecord) -> Vec<EvidenceLocator> {
    node_scoped_locators(path, invocation.node_id.as_str(), "passive_invocation")
}

fn node_scoped_locators(
    path: &str,
    node_id: &str,
    source_name: &'static str,
) -> Vec<EvidenceLocator> {
    let mut locators = vec![
        EvidenceLocator::LoadedSummary { name: source_name },
        EvidenceLocator::SchedulerNode {
            node_id: node_id.to_owned(),
        },
    ];

    if path.ends_with("evaluation.json") {
        locators.push(EvidenceLocator::EvaluationArtifact { path: path.into() });
    }

    locators
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use ploke_records::ids::{
        ArtifactId, BranchId, CampaignId, CandidateId, InstanceId, OperationTarget,
        OperationTarget::Artifact, RuntimeId, SchedulerNodeId, SourceStateId,
    };
    use ploke_records::invocation::{InvocationRecord, Role};
    use ploke_records::scheduler::{
        NodeRecord, NodeStatusRecord, RunnerRequestRecord, RunnerResultRecord,
    };

    use crate::graph::{
        ArtifactKey, EvidenceKind, EvidenceLocator, EvidenceSubject, OperationKey,
        OperationTargetKey,
    };
    use crate::{PassiveEvidence, RunAttemptEvidence, RunAttemptSummary};

    use super::Builder;

    #[test]
    fn passive_run_attempts_populate_operation_metadata_without_history_authority() {
        let runtime_id = RuntimeId("runtime:child".to_owned());
        let mut passive = PassiveEvidence::default();
        passive.run_attempts = Some(RunAttemptEvidence {
            summary: RunAttemptSummary {
                runner_request_file_count: 1,
                runner_request_parsed_count: 1,
                runner_result_file_count: 1,
                runner_result_parsed_count: 1,
                invocation_file_count: 1,
                invocation_parsed_count: 1,
                child_invocation_count: 1,
                successor_invocation_count: 0,
            },
            runner_requests: BTreeMap::from([(
                "nodes/node-1/runner-request.json".to_owned(),
                runner_request(),
            )]),
            runner_results: BTreeMap::from([(
                "nodes/node-1/runner-result.json".to_owned(),
                runner_result(),
            )]),
            invocations: BTreeMap::from([(
                "nodes/node-1/invocations/child.json".to_owned(),
                invocation(runtime_id.clone()),
            )]),
        });
        let mut builder = Builder::default();

        builder.ingest_passive_evidence(&passive);
        let graph = builder.finish();

        assert!(graph.history.blocks.is_empty());
        assert!(graph.authority.epochs_by_lineage.is_empty());
        assert!(graph.runtimes.runtimes.contains_key(&runtime_id));
        assert!(
            graph
                .artifacts
                .artifacts
                .contains_key(&ArtifactKey::PassiveId {
                    value: "artifact-before".to_owned()
                })
        );
        assert!(
            graph
                .artifacts
                .artifacts
                .contains_key(&ArtifactKey::PassiveId {
                    value: "artifact-after".to_owned()
                })
        );
        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::CandidateEvaluation
                && evidence.subject == EvidenceSubject::Branch("branch-1".to_owned())
        }));
        let operation_key = OperationKey::RuntimeTarget {
            runtime_id: runtime_id.clone(),
            target: OperationTargetKey::Artifact {
                artifact_id: ArtifactId("artifact-before".to_owned()),
            },
        };
        assert!(graph.operations.operations.contains_key(&operation_key));

        let operation = graph
            .operations
            .operations
            .get(&operation_key)
            .expect("operation");
        assert!(operation.evidence.iter().any(|id| {
            graph.evidence.attachments[id]
                .locators
                .iter()
                .any(|locator| {
                    locator
                        == &EvidenceLocator::LoadedSummary {
                            name: "passive_invocation",
                        }
                })
        }));
    }

    #[test]
    fn passive_attempt_runner_results_attach_branch_evidence_without_history_authority() {
        let mut passive = PassiveEvidence::default();
        passive.attempt_runner_results = BTreeMap::from([(
            "nodes/node-1/results/runtime-1.json".to_owned(),
            runner_result(),
        )]);
        let mut builder = Builder::default();

        builder.ingest_passive_evidence(&passive);
        let graph = builder.finish();

        assert!(graph.history.blocks.is_empty());
        assert!(graph.authority.epochs_by_lineage.is_empty());
        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::CandidateEvaluation
                && evidence.subject == EvidenceSubject::Branch("branch-1".to_owned())
                && evidence.locators.iter().any(|locator| {
                    locator
                        == &EvidenceLocator::LoadedSummary {
                            name: "passive_attempt_runner_result",
                        }
                })
                && evidence.locators.iter().any(|locator| {
                    locator
                        == &EvidenceLocator::EvaluationArtifact {
                            path: PathBuf::from("nodes/node-1/evaluation.json"),
                        }
                })
        }));
    }

    fn invocation(runtime_id: RuntimeId) -> InvocationRecord {
        InvocationRecord {
            schema_version: "prototype1-invocation.v1".to_owned(),
            role: Role::Child,
            campaign_id: "campaign-1".to_owned(),
            node_id: "node-1".to_owned(),
            runtime_id,
            journal_path: PathBuf::from("transition-journal.jsonl"),
            channel_root: None,
            node: Some(node_record()),
            request: Some(runner_request()),
            resolved: None,
            active_parent_root: None,
            created_at: "2026-05-11T00:00:00Z".to_owned(),
        }
    }

    fn runner_request() -> RunnerRequestRecord {
        RunnerRequestRecord {
            schema_version: "prototype1-treatment-node.v1".to_owned(),
            campaign_id: CampaignId("campaign-1".to_owned()),
            node_id: SchedulerNodeId("node-1".to_owned()),
            generation: 1,
            instance_id: InstanceId("instance-1".to_owned()),
            source_state_id: SourceStateId("source-1".to_owned()),
            operation_target: Some(operation_target()),
            base_artifact_id: Some(ArtifactId("artifact-before".to_owned())),
            patch_id: None,
            derived_artifact_id: Some(ArtifactId("artifact-after".to_owned())),
            branch_id: BranchId("branch-1".to_owned()),
            target_relpath: PathBuf::from("src/lib.rs"),
            workspace_root: PathBuf::from("worktree"),
            binary_path: PathBuf::from("target/debug/ploke"),
            stop_on_error: true,
            runner_args: Vec::new(),
        }
    }

    fn runner_result() -> RunnerResultRecord {
        RunnerResultRecord {
            schema_version: "prototype1-treatment-node.v1".to_owned(),
            campaign_id: CampaignId("campaign-1".to_owned()),
            node_id: SchedulerNodeId("node-1".to_owned()),
            generation: 1,
            branch_id: BranchId("branch-1".to_owned()),
            status: NodeStatusRecord::Succeeded,
            disposition: ploke_records::scheduler::RunnerDispositionRecord::Succeeded,
            treatment_campaign_id: None,
            evaluation_artifact_path: Some(PathBuf::from("nodes/node-1/evaluation.json")),
            detail: None,
            exit_code: Some(0),
            stdout_excerpt: None,
            stderr_excerpt: None,
            recorded_at: "2026-05-11T00:01:00Z".to_owned(),
        }
    }

    fn node_record() -> NodeRecord {
        NodeRecord {
            schema_version: "prototype1-treatment-node.v1".to_owned(),
            node_id: SchedulerNodeId("node-1".to_owned()),
            parent_node_id: None,
            generation: 1,
            instance_id: InstanceId("instance-1".to_owned()),
            source_state_id: SourceStateId("source-1".to_owned()),
            operation_target: Some(operation_target()),
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

    fn operation_target() -> OperationTarget {
        Artifact {
            artifact_id: ArtifactId("artifact-before".to_owned()),
        }
    }
}
