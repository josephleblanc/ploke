//! Passive scheduler record mirrors.
//!
//! These DTOs mirror the persisted Prototype 1 scheduler JSON shape. They are
//! intentionally inert: scheduling policy, node registration, runner argv
//! construction, continuation decisions, filesystem mutation, and History/Crown
//! authority remain in `ploke-eval`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::{
    ArtifactId, BranchId, CampaignId, CandidateId, InstanceId, OperationTarget, PatchId,
    SchedulerNodeId, SourceStateId,
};
use crate::record::{Record, RecordFamily, RecordFormat};

pub const SCHEDULER_STATE_SCHEMA_V1: &str = "prototype1-scheduler.v1";
pub const TREATMENT_NODE_SCHEMA_V1: &str = "prototype1-treatment-node.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchPolicyRecord {
    pub max_generations: u32,
    pub max_total_nodes: u32,
    #[serde(default)]
    pub child_budget: ChildBudgetRecord,
    #[serde(default)]
    pub child_schedule_mode: ChildScheduleModeRecord,
    pub stop_on_first_keep: bool,
    pub require_keep_for_continuation: bool,
    #[serde(default = "default_explore_from_rejected")]
    pub explore_from_rejected: bool,
}

impl Default for SearchPolicyRecord {
    fn default() -> Self {
        Self {
            max_generations: 1,
            max_total_nodes: 32,
            child_budget: ChildBudgetRecord::default(),
            child_schedule_mode: ChildScheduleModeRecord::default(),
            stop_on_first_keep: false,
            require_keep_for_continuation: true,
            explore_from_rejected: true,
        }
    }
}

fn default_explore_from_rejected() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildBudgetRecord {
    pub min: u32,
    pub max: u32,
}

impl Default for ChildBudgetRecord {
    fn default() -> Self {
        Self { min: 2, max: 6 }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ChildScheduleModeRecord {
    #[default]
    #[serde(alias = "full_batch")]
    FullBatch,
    #[serde(alias = "adaptive_batch")]
    AdaptiveBatch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationDispositionRecord {
    ContinueReady,
    ContinueExploreFromRejected,
    ContinueHistoricalTraversal,
    StopMaxGenerations,
    StopMaxTotalNodes,
    StopNoSelectedBranch,
    StopOnFirstKeepSatisfied,
    StopSelectedBranchRejected,
    StopHistoricalSelection,
    StopNonDirectChildSelection,
    StopHistoricalTraversalCycle,
    StopHistoricalTraversalBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContinuationDecisionRecord {
    pub disposition: ContinuationDispositionRecord,
    pub selected_next_branch_id: Option<BranchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_branch_disposition: Option<String>,
    pub next_generation: u32,
    pub total_nodes_after_continue: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatusRecord {
    Planned,
    WorkspaceStaged,
    BinaryBuilt,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerDispositionRecord {
    Succeeded,
    CompileFailed,
    TreatmentFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRecord {
    pub schema_version: String,
    pub node_id: SchedulerNodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node_id: Option<SchedulerNodeId>,
    pub generation: u32,
    pub instance_id: InstanceId,
    pub source_state_id: SourceStateId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_target: Option<OperationTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<PatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<BranchId>,
    pub branch_id: BranchId,
    pub candidate_id: CandidateId,
    pub target_relpath: PathBuf,
    pub node_dir: PathBuf,
    pub workspace_root: PathBuf,
    pub binary_path: PathBuf,
    pub runner_request_path: PathBuf,
    pub runner_result_path: PathBuf,
    pub status: NodeStatusRecord,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerRequestRecord {
    pub schema_version: String,
    pub campaign_id: CampaignId,
    pub node_id: SchedulerNodeId,
    pub generation: u32,
    pub instance_id: InstanceId,
    pub source_state_id: SourceStateId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_target: Option<OperationTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<PatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_artifact_id: Option<ArtifactId>,
    pub branch_id: BranchId,
    pub target_relpath: PathBuf,
    pub workspace_root: PathBuf,
    pub binary_path: PathBuf,
    #[serde(default)]
    pub stop_on_error: bool,
    pub runner_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerResultRecord {
    pub schema_version: String,
    pub campaign_id: CampaignId,
    pub node_id: SchedulerNodeId,
    pub generation: u32,
    pub branch_id: BranchId,
    pub status: NodeStatusRecord,
    pub disposition: RunnerDispositionRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_campaign_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_artifact_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_excerpt: Option<String>,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulerStateRecord {
    pub schema_version: String,
    pub campaign_id: CampaignId,
    pub updated_at: String,
    #[serde(default)]
    pub policy: SearchPolicyRecord,
    #[serde(default)]
    pub frontier_node_ids: Vec<SchedulerNodeId>,
    #[serde(default)]
    pub completed_node_ids: Vec<SchedulerNodeId>,
    #[serde(default)]
    pub failed_node_ids: Vec<SchedulerNodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_continuation_decision: Option<ContinuationDecisionRecord>,
    #[serde(default)]
    pub nodes: Vec<NodeRecord>,
}

impl Record for SchedulerStateRecord {
    const FAMILY: RecordFamily = RecordFamily::SchedulerState;
    const SCHEMA: &'static str = SCHEDULER_STATE_SCHEMA_V1;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

impl Record for NodeRecord {
    const FAMILY: RecordFamily = RecordFamily::SchedulerNode;
    const SCHEMA: &'static str = TREATMENT_NODE_SCHEMA_V1;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

impl Record for RunnerRequestRecord {
    const FAMILY: RecordFamily = RecordFamily::RunnerRequest;
    const SCHEMA: &'static str = TREATMENT_NODE_SCHEMA_V1;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

impl Record for RunnerResultRecord {
    const FAMILY: RecordFamily = RecordFamily::RunnerResult;
    const SCHEMA: &'static str = TREATMENT_NODE_SCHEMA_V1;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    fn representative_scheduler_json() -> serde_json::Value {
        serde_json::json!({
            "schema_version": "prototype1-scheduler.v1",
            "campaign_id": "campaign-1",
            "updated_at": "2026-05-08T12:00:00Z",
            "policy": {
                "max_generations": 3,
                "max_total_nodes": 16,
                "child_budget": {
                    "min": 2,
                    "max": 4
                },
                "child_schedule_mode": "adaptive-batch",
                "stop_on_first_keep": false,
                "require_keep_for_continuation": true,
                "explore_from_rejected": true
            },
            "frontier_node_ids": ["node-1"],
            "completed_node_ids": ["node-0"],
            "failed_node_ids": [],
            "last_continuation_decision": {
                "disposition": "continue_explore_from_rejected",
                "selected_next_branch_id": "branch-1",
                "selected_branch_disposition": "reject",
                "next_generation": 2,
                "total_nodes_after_continue": 2
            },
            "nodes": [{
                "schema_version": "prototype1-treatment-node.v1",
                "node_id": "node-1",
                "parent_node_id": "node-0",
                "generation": 1,
                "instance_id": "instance-1",
                "source_state_id": "source-1",
                "operation_target": {
                    "kind": "artifact",
                    "artifact_id": "git-tree:source"
                },
                "base_artifact_id": "git-tree:source",
                "patch_id": "patch:attempt-1",
                "derived_artifact_id": "git-commit:derived",
                "parent_branch_id": "branch-0",
                "branch_id": "branch-1",
                "candidate_id": "candidate-1",
                "target_relpath": "crates/ploke-core/tool_text/request_code_context.md",
                "node_dir": "/tmp/prototype1/nodes/node-1",
                "workspace_root": "/tmp/worktrees/node-1",
                "binary_path": "/tmp/worktrees/node-1/target/debug/ploke",
                "runner_request_path": "/tmp/prototype1/nodes/node-1/runner-request.json",
                "runner_result_path": "/tmp/prototype1/nodes/node-1/runner-result.json",
                "status": "running",
                "created_at": "2026-05-08T12:00:00Z",
                "updated_at": "2026-05-08T12:01:00Z"
            }]
        })
    }

    #[test]
    fn scheduler_state_record_roundtrips_representative_json() {
        let json = representative_scheduler_json();

        let record: SchedulerStateRecord =
            serde_json::from_value(json.clone()).expect("deserialize scheduler record");
        assert_eq!(record.campaign_id.as_str(), "campaign-1");
        assert_eq!(record.frontier_node_ids[0].as_str(), "node-1");
        assert_eq!(
            record.policy.child_schedule_mode,
            ChildScheduleModeRecord::AdaptiveBatch
        );
        assert_eq!(record.nodes[0].branch_id.as_str(), "branch-1");
        assert_eq!(record.nodes[0].status, NodeStatusRecord::Running);

        let serialized = serde_json::to_value(&record).expect("serialize scheduler record");
        assert_eq!(serialized, json);
    }

    #[test]
    fn scheduler_policy_accepts_legacy_missing_defaults_and_aliases() {
        let json = serde_json::json!({
            "max_generations": 2,
            "max_total_nodes": 8,
            "child_budget": {
                "min": 1,
                "max": 3
            },
            "stop_on_first_keep": false,
            "require_keep_for_continuation": true
        });

        let policy: SearchPolicyRecord =
            serde_json::from_value(json).expect("deserialize policy defaults");
        assert_eq!(
            policy.child_schedule_mode,
            ChildScheduleModeRecord::FullBatch
        );
        assert!(policy.explore_from_rejected);

        let alias: ChildScheduleModeRecord =
            serde_json::from_str("\"full_batch\"").expect("deserialize old full_batch alias");
        assert_eq!(alias, ChildScheduleModeRecord::FullBatch);
    }

    #[test]
    fn continuation_decision_accepts_newer_historical_variants() {
        let historical: ContinuationDecisionRecord = serde_json::from_value(serde_json::json!({
            "disposition": "continue_historical_traversal",
            "selected_next_branch_id": "branch-history",
            "selected_branch_disposition": "keep",
            "next_generation": 2,
            "total_nodes_after_continue": 7
        }))
        .expect("deserialize historical traversal continuation");
        assert_eq!(
            historical.disposition,
            ContinuationDispositionRecord::ContinueHistoricalTraversal
        );

        let stop: ContinuationDecisionRecord = serde_json::from_value(serde_json::json!({
            "disposition": "stop_historical_traversal_budget",
            "selected_next_branch_id": null,
            "selected_branch_disposition": null,
            "next_generation": 2,
            "total_nodes_after_continue": 7
        }))
        .expect("deserialize historical traversal budget stop");
        assert_eq!(
            stop.disposition,
            ContinuationDispositionRecord::StopHistoricalTraversalBudget
        );
    }

    #[test]
    fn runner_request_and_result_records_roundtrip_representative_json() {
        let request_json = serde_json::json!({
            "schema_version": "prototype1-runner-request.v1",
            "campaign_id": "campaign-1",
            "node_id": "node-1",
            "generation": 1,
            "instance_id": "instance-1",
            "source_state_id": "source-1",
            "operation_target": {
                "kind": "patch_set",
                "base_artifact_id": "git-tree:source",
                "patch_ids": ["patch:attempt-1"]
            },
            "base_artifact_id": "git-tree:source",
            "patch_id": "patch:attempt-1",
            "derived_artifact_id": "git-commit:derived",
            "branch_id": "branch-1",
            "target_relpath": "crates/ploke-core/tool_text/request_code_context.md",
            "workspace_root": "/tmp/worktrees/node-1",
            "binary_path": "/tmp/worktrees/node-1/target/debug/ploke",
            "stop_on_error": true,
            "runner_args": ["prototype1", "runner"]
        });
        let request: RunnerRequestRecord =
            serde_json::from_value(request_json.clone()).expect("deserialize request");
        assert_eq!(
            serde_json::to_value(&request).expect("serialize request"),
            request_json
        );

        let result_json = serde_json::json!({
            "schema_version": "prototype1-runner-result.v1",
            "campaign_id": "campaign-1",
            "node_id": "node-1",
            "generation": 1,
            "branch_id": "branch-1",
            "status": "failed",
            "disposition": "compile_failed",
            "treatment_campaign_id": "treatment-1",
            "evaluation_artifact_path": "/tmp/eval/artifact.json",
            "detail": "compile failed",
            "exit_code": 101,
            "stdout_excerpt": "",
            "stderr_excerpt": "error[E0425]",
            "recorded_at": "2026-05-08T12:02:00Z"
        });
        let result: RunnerResultRecord =
            serde_json::from_value(result_json.clone()).expect("deserialize result");
        assert_eq!(
            serde_json::to_value(&result).expect("serialize result"),
            result_json
        );
    }

    #[test]
    #[ignore]
    fn real_campaign_scheduler_json_roundtrips() {
        let path = real_run_root().join("scheduler.json");
        let original = read_json_value(&path);
        let record: SchedulerStateRecord =
            serde_json::from_value(original.clone()).expect("deserialize real scheduler");

        assert_eq!(
            serde_json::to_value(&record).expect("serialize real scheduler"),
            original
        );
        assert!(!record.campaign_id.as_str().is_empty());
        assert!(!record.nodes.is_empty());
        assert!(
            record
                .nodes
                .iter()
                .all(|node| !node.node_id.as_str().is_empty())
        );
        assert!(
            record
                .frontier_node_ids
                .iter()
                .chain(record.completed_node_ids.iter())
                .chain(record.failed_node_ids.iter())
                .all(|id| record.nodes.iter().any(|node| node.node_id == *id))
        );
    }

    #[test]
    #[ignore]
    fn real_campaign_node_json_roundtrips() {
        let root = real_run_root();
        let scheduler: SchedulerStateRecord =
            serde_json::from_value(read_json_value(&root.join("scheduler.json")))
                .expect("deserialize scheduler");
        let node_id = scheduler
            .nodes
            .first()
            .expect("real run has at least one node")
            .node_id
            .as_str()
            .to_owned();
        let path = root.join("nodes").join(&node_id).join("node.json");
        let original = read_json_value(&path);
        let record: NodeRecord =
            serde_json::from_value(original.clone()).expect("deserialize real node");

        assert_eq!(
            serde_json::to_value(&record).expect("serialize real node"),
            original
        );
        assert_eq!(record.node_id.as_str(), node_id);
        let scheduler_node = scheduler
            .nodes
            .iter()
            .find(|node| node.node_id == record.node_id)
            .expect("node is present in scheduler");
        assert_eq!(scheduler_node.generation, record.generation);
        assert_eq!(scheduler_node.instance_id, record.instance_id);
        assert_eq!(scheduler_node.branch_id, record.branch_id);
        assert!(!record.source_state_id.as_str().is_empty());
    }

    fn real_run_root() -> PathBuf {
        std::env::var_os("PLOKE_RECORDS_REAL_RUN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    "/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1",
                )
            })
    }

    fn read_json_value(path: &Path) -> serde_json::Value {
        let text = fs::read_to_string(path).unwrap_or_else(|err| {
            panic!(
                "read real-run fixture {} (set PLOKE_RECORDS_REAL_RUN_ROOT to override): {err}",
                path.display()
            )
        });
        serde_json::from_str(&text).expect("parse real-run JSON value")
    }
}
