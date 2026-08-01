//! Passive child-plan message records.
//!
//! These DTOs mirror `prototype1/messages/child-plan/<parent-node-id>.json`.
//! They describe a parent-owned message-box body; deserializing them does not
//! receive the message, validate parent authority, stage children, or advance
//! any Parent state.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::branch::ResolvedTreatmentBranch;
use crate::history::{SurfaceAttemptRecord, SurfaceEvidenceRecord};
use crate::ids::SchedulerNodeId;
use crate::record::{Record, RecordFamily, RecordFormat};
use crate::scheduler::{NodeRecord, RunnerRequestRecord};

pub const CHILD_PLAN_SCHEMA_V1: &str = "prototype1-child-plan.v1";

/// Body persisted in one parent-owned child-plan message box.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildPlanRecord {
    /// Concrete message-box path serialized by the eval-side `At<ChildPlanFile>`.
    pub message: PathBuf,
    pub parent_node_id: SchedulerNodeId,
    pub child_generation: u32,
    pub children: Vec<ChildPlanChildRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejected_surface_attempts: Vec<SurfaceAttemptRecord>,
}

/// One planned child bundle carried by a child-plan message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildPlanChildRecord {
    pub node: NodeRecord,
    pub request: RunnerRequestRecord,
    pub resolved: ResolvedTreatmentBranch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<SurfaceEvidenceRecord>,
}

impl Record for ChildPlanRecord {
    const FAMILY: RecordFamily = RecordFamily::ChildPlan;
    const SCHEMA: &'static str = CHILD_PLAN_SCHEMA_V1;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn representative_child_plan_json() -> serde_json::Value {
        json!({
            "message": "/tmp/run/prototype1/messages/child-plan/parent-a.json",
            "parent_node_id": "parent-a",
            "child_generation": 1,
            "children": [{
                "node": {
                    "schema_version": "prototype1-treatment-node.v1",
                    "node_id": "child-1",
                    "parent_node_id": "parent-a",
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
                    "parent_branch_id": "branch-parent",
                    "branch_id": "branch-child",
                    "candidate_id": "candidate-1",
                    "target_relpath": "src/lib.rs",
                    "node_dir": "/tmp/run/prototype1/nodes/child-1",
                    "workspace_root": "/tmp/worktrees/child-1",
                    "binary_path": "/tmp/worktrees/child-1/target/debug/ploke",
                    "runner_request_path": "/tmp/run/prototype1/nodes/child-1/runner-request.json",
                    "runner_result_path": "/tmp/run/prototype1/nodes/child-1/runner-result.json",
                    "status": "planned",
                    "created_at": "2026-05-11T12:00:00Z",
                    "updated_at": "2026-05-11T12:00:00Z"
                },
                "request": {
                    "schema_version": "prototype1-runner-request.v1",
                    "campaign_id": "campaign-1",
                    "node_id": "child-1",
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
                    "branch_id": "branch-child",
                    "target_relpath": "src/lib.rs",
                    "workspace_root": "/tmp/worktrees/child-1",
                    "binary_path": "/tmp/worktrees/child-1/target/debug/ploke",
                    "stop_on_error": false,
                    "runner_args": ["prototype1", "runner"]
                },
                "resolved": {
                    "instance_id": "instance-1",
                    "source_state_id": "source-1",
                    "parent_branch_id": "branch-parent",
                    "target_relpath": "src/lib.rs",
                    "source_content": "fn main() {}",
                    "source_content_hash": "sha256:source",
                    "selected_branch_id": "branch-child",
                    "branch": {
                        "branch_id": "branch-child",
                        "candidate_id": "candidate-1",
                        "patch_id": "patch:attempt-1",
                        "branch_label": "candidate-1",
                        "synthesized_spec_id": "spec-1",
                        "proposed_content": "fn main() { println!(\"hi\"); }",
                        "proposed_content_hash": "sha256:proposed",
                        "generation_target": {
                            "kind": "artifact",
                            "artifact_id": "git-tree:source"
                        },
                        "generation_coordinate": {
                            "runtime_id": "runtime-1",
                            "target": {
                                "kind": "artifact",
                                "artifact_id": "git-tree:source"
                            }
                        },
                        "status": "synthesized",
                        "apply_id": "apply-1",
                        "applied_content_hash": "sha256:applied",
                        "derived_artifact_id": "git-commit:derived"
                    }
                },
                "surface": {
                    "schema_version": 2,
                    "producer_id": "prototype1:tui-edit-surface:deterministic-v1",
                    "proposal_id": "proposal-accepted",
                    "run_id": "run-accepted",
                    "policy": "workspace_except_ploke_eval",
                    "target_relpath": "src/lib.rs",
                    "base": {
                        "artifact_id": "git-tree:source",
                        "hash": "sha256:source"
                    },
                    "after": {
                        "artifact_id": "git-commit:derived",
                        "hash": "sha256:applied"
                    },
                    "patch_id": "patch:attempt-1",
                    "source_content_hash": "sha256:source",
                    "proposed_content_hash": "sha256:proposed",
                    "proposal_producer": {
                        "kind": "non_router"
                    },
                    "generator_surface": {
                        "projection_id": "tui-generator-projection",
                        "projection_hash": "sha256:projection",
                        "bounds_digest": "sha256:bounds",
                        "source_kind": "named",
                        "source_id": "tui-edit-surface",
                        "source_version": "deterministic-v1"
                    },
                    "touches": [{
                        "target_relpath": "src/lib.rs",
                        "target_name": "direct-splice:eof-comment",
                        "span_relpath": "src/lib.rs",
                        "start": 12,
                        "end": 12,
                        "base_hash": "sha256:source",
                        "replacement": " println!(\"hi\");",
                        "replacement_hash": "sha256:replacement"
                    }],
                    "touches_digest": "sha256:touches",
                    "delta_id": "surface-delta:sha256:delta",
                    "delta_digest": "sha256:delta",
                    "check_status": "checked",
                    "apply_status": "applied"
                }
            }],
            "rejected_surface_attempts": [{
                "schema_version": 1,
                "producer_id": "prototype1:tui-edit-surface:deterministic-v1",
                "proposal_id": "proposal-rejected",
                "run_id": "run-1",
                "policy": "workspace_except_ploke_eval",
                "target_relpath": "src/lib.rs",
                "outcome": {
                    "kind": "rejected",
                    "reason": "duplicate proposal"
                }
            }]
        })
    }

    #[test]
    fn child_plan_record_roundtrips_representative_json() {
        let json = representative_child_plan_json();
        let record: ChildPlanRecord =
            serde_json::from_value(json.clone()).expect("deserialize child plan");

        assert_eq!(record.parent_node_id.as_str(), "parent-a");
        assert_eq!(record.children[0].node.node_id.as_str(), "child-1");
        assert!(record.children[0].surface.is_some());
        assert_eq!(record.rejected_surface_attempts.len(), 1);
        assert_eq!(
            serde_json::to_value(&record).expect("serialize child plan"),
            json
        );
    }
}
