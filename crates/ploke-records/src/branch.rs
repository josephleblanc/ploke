//! Passive branch registry DTOs.
//!
//! The records in this module mirror the persisted Prototype 1 branch registry
//! shape. They do not select, mutate, restore, or evaluate branches.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::{ArtifactId, CampaignId, Coordinate, OperationTarget, PatchId};

/// Durable schema version used by current Prototype 1 branch registries.
pub const PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION: &str = "prototype1-branch-registry.v1";

/// Durable schema version used by append-only Prototype 1 branch logs.
pub const PROTOTYPE1_BRANCH_LOG_SCHEMA_VERSION: &str = "prototype1-branch-record.v1";

/// Branch-level disposition assigned by passive evaluation records.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Keep,
    Reject,
}

/// Lifecycle label for one treatment branch node in the registry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TreatmentBranchStatus {
    Synthesized,
    Selected,
    Applied,
    Restored,
    Dropped,
}

/// Summary of an evaluation comparing a baseline and treatment campaign.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreatmentBranchEvaluationSummary {
    pub baseline_campaign_id: CampaignId,
    pub treatment_campaign_id: CampaignId,
    pub compared_instances: usize,
    pub rejected_instances: usize,
    pub overall_disposition: Disposition,
    pub evaluated_at: String,
}

/// One append-only branch log record persisted in `prototype1/branches.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchLogRecord {
    pub schema_version: String,
    pub recorded_at: String,
    pub body: BranchLogBody,
}

/// Body of one append-only branch log record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BranchLogBody {
    RegistrySnapshot(Prototype1BranchRegistry),
    ParentComparison(BranchParentComparisonRecord),
}

/// Passive record of a parent comparison written after evaluating one branch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchParentComparisonRecord {
    pub campaign_id: CampaignId,
    pub instance_id: String,
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<String>,
    pub target_relpath: PathBuf,
    pub branch_id: String,
    pub candidate_id: String,
    pub summary: TreatmentBranchEvaluationSummary,
}

/// One candidate branch under an intervention source node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreatmentBranchNode {
    pub branch_id: String,
    pub candidate_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<PatchId>,
    pub branch_label: String,
    pub synthesized_spec_id: String,
    pub proposed_content: String,
    pub proposed_content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_target: Option<OperationTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_coordinate: Option<Coordinate>,
    pub status: TreatmentBranchStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_evaluation: Option<TreatmentBranchEvaluationSummary>,
}

/// Materialized branch payload carried by child invocation and handoff records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedTreatmentBranch {
    pub instance_id: String,
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<String>,
    pub target_relpath: PathBuf,
    pub source_content: String,
    pub source_content_hash: String,
    pub selected_branch_id: Option<String>,
    pub branch: TreatmentBranchNode,
}

/// One source state and target file with candidate treatment branches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSourceNode {
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_target: Option<OperationTarget>,
    pub instance_id: String,
    pub target_relpath: PathBuf,
    pub source_content: String,
    pub source_content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_branch_id: Option<String>,
    #[serde(default)]
    pub branches: Vec<TreatmentBranchNode>,
}

/// Active branch projection for a target file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveInterventionTarget {
    pub target_relpath: PathBuf,
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_patch_id: Option<PatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_apply_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_derived_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_operation_target: Option<OperationTarget>,
}

/// Full passive branch registry record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Prototype1BranchRegistry {
    pub schema_version: String,
    pub campaign_id: CampaignId,
    pub updated_at: String,
    #[serde(default)]
    pub source_nodes: Vec<InterventionSourceNode>,
    #[serde(default)]
    pub active_targets: Vec<ActiveInterventionTarget>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn roundtrips_representative_branch_registry() {
        let json = serde_json::json!({
            "schema_version": PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION,
            "campaign_id": "campaign-1",
            "updated_at": "2026-05-08T12:00:00Z",
            "source_nodes": [{
                "source_state_id": "source-1",
                "parent_branch_id": "branch-parent",
                "source_artifact_id": "artifact-source",
                "operation_target": {
                    "kind": "artifact",
                    "artifact_id": "artifact-source"
                },
                "instance_id": "instance-1",
                "target_relpath": "AGENTS.md",
                "source_content": "old",
                "source_content_hash": "hash-old",
                "selected_branch_id": "branch-1",
                "branches": [{
                    "branch_id": "branch-1",
                    "candidate_id": "candidate-1",
                    "patch_id": "patch-1",
                    "branch_label": "candidate 1",
                    "synthesized_spec_id": "spec-1",
                    "proposed_content": "new",
                    "proposed_content_hash": "hash-new",
                    "generation_target": {
                        "kind": "patch_set",
                        "base_artifact_id": "artifact-source",
                        "patch_ids": ["patch-1"]
                    },
                    "generation_coordinate": {
                        "runtime_id": "11111111-1111-1111-1111-111111111111",
                        "target": {
                            "kind": "artifact",
                            "artifact_id": "artifact-source"
                        }
                    },
                    "status": "applied",
                    "apply_id": "apply-1",
                    "applied_content_hash": "hash-applied",
                    "derived_artifact_id": "artifact-derived",
                    "latest_evaluation": {
                        "baseline_campaign_id": "baseline-1",
                        "treatment_campaign_id": "treatment-1",
                        "compared_instances": 3,
                        "rejected_instances": 0,
                        "overall_disposition": "keep",
                        "evaluated_at": "2026-05-08T13:00:00Z"
                    }
                }]
            }],
            "active_targets": [{
                "target_relpath": "AGENTS.md",
                "source_state_id": "source-1",
                "source_artifact_id": "artifact-source",
                "active_branch_id": "branch-1",
                "active_patch_id": "patch-1",
                "active_apply_id": "apply-1",
                "active_derived_artifact_id": "artifact-derived",
                "active_operation_target": {
                    "kind": "artifact",
                    "artifact_id": "artifact-derived"
                }
            }]
        });

        let record: Prototype1BranchRegistry = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    fn registry_defaults_missing_collections() {
        let json = serde_json::json!({
            "schema_version": PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION,
            "campaign_id": "campaign-1",
            "updated_at": "2026-05-08T12:00:00Z"
        });

        let record: Prototype1BranchRegistry = serde_json::from_value(json).unwrap();
        assert!(record.source_nodes.is_empty());
        assert!(record.active_targets.is_empty());
    }

    #[test]
    fn resolved_treatment_branch_preserves_null_selected_branch_id() {
        let json = serde_json::json!({
            "instance_id": "instance-1",
            "source_state_id": "source-1",
            "target_relpath": "src/lib.rs",
            "source_content": "fn main() {}",
            "source_content_hash": "sha256:source",
            "selected_branch_id": null,
            "branch": {
                "branch_id": "branch-child",
                "candidate_id": "candidate-1",
                "patch_id": "patch:attempt-1",
                "branch_label": "candidate-1",
                "synthesized_spec_id": "spec-1",
                "proposed_content": "fn main() { println!(\"hi\"); }",
                "proposed_content_hash": "sha256:proposed",
                "status": "synthesized"
            }
        });

        let record: ResolvedTreatmentBranch = serde_json::from_value(json.clone()).unwrap();
        assert!(record.selected_branch_id.is_none());
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    fn branch_log_parent_comparison_roundtrips() {
        let json = serde_json::json!({
            "schema_version": PROTOTYPE1_BRANCH_LOG_SCHEMA_VERSION,
            "recorded_at": "2026-05-11T10:34:56Z",
            "body": {
                "kind": "parent_comparison",
                "campaign_id": "campaign-1",
                "instance_id": "instance-1",
                "source_state_id": "source-1",
                "parent_branch_id": "branch-parent",
                "target_relpath": "crates/ploke-llm/src/lib.rs",
                "branch_id": "branch-1",
                "candidate_id": "candidate-1",
                "summary": {
                    "baseline_campaign_id": "campaign-1",
                    "treatment_campaign_id": "campaign-1-treatment-branch-1",
                    "compared_instances": 1,
                    "rejected_instances": 0,
                    "overall_disposition": "keep",
                    "evaluated_at": "2026-05-11T10:34:55Z"
                }
            }
        });

        let record: BranchLogRecord = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), json);
    }

    #[test]
    #[ignore]
    fn real_campaign_branch_registry_roundtrips_selection_and_evaluation_surface() {
        let path = real_run_root().join("branches.json");
        let original = read_json_value(&path);
        let record: Prototype1BranchRegistry =
            serde_json::from_value(original.clone()).expect("deserialize real branch registry");

        assert_eq!(
            serde_json::to_value(&record).expect("serialize real branch registry"),
            original
        );
        assert_eq!(
            record.campaign_id,
            CampaignId::from("p1-edit-surface-history-long-20260508-1")
        );
        assert_eq!(record.source_nodes.len(), 11);
        assert_eq!(
            record
                .source_nodes
                .iter()
                .map(|source| source.branches.len())
                .sum::<usize>(),
            36
        );

        let source = record
            .source_nodes
            .iter()
            .find(|source| source.source_state_id == "branch-d176496f54e6e755")
            .expect("generation source with selected successor branch");
        assert_eq!(
            source.selected_branch_id.as_deref(),
            Some("branch-eeba26463d120530")
        );

        let selected = source
            .branches
            .iter()
            .find(|branch| branch.branch_id == "branch-eeba26463d120530")
            .expect("selected branch node");
        assert_eq!(selected.candidate_id, "candidate-1");
        assert_eq!(selected.status, TreatmentBranchStatus::Selected);

        let evaluation = selected
            .latest_evaluation
            .as_ref()
            .expect("selected branch evaluation");
        assert_eq!(
            evaluation.baseline_campaign_id,
            CampaignId::from("p1-edit-surface-history-long-20260508-1")
        );
        assert_eq!(evaluation.compared_instances, 1);
        assert_eq!(evaluation.rejected_instances, 0);
        assert_eq!(evaluation.overall_disposition, Disposition::Keep);
    }

    #[test]
    #[ignore]
    fn print_real_campaign_branch_registry_round_trip_states() {
        let path = real_run_root().join("branches.json");
        let original = read_json_value(&path);

        println!(
            "before deserialize:\n{}",
            serde_json::to_string_pretty(&branch_registry_probe_from_value(&original))
                .expect("format original probe")
        );

        let record: Prototype1BranchRegistry =
            serde_json::from_value(original.clone()).expect("deserialize real branch registry");
        println!(
            "after deserialize:\n{}",
            serde_json::to_string_pretty(&branch_registry_probe_from_record(&record))
                .expect("format record probe")
        );

        let serialized = serde_json::to_value(&record).expect("serialize real branch registry");
        println!(
            "after serialize again:\n{}",
            serde_json::to_string_pretty(&branch_registry_probe_from_value(&serialized))
                .expect("format serialized probe")
        );

        assert_eq!(serialized, original);
    }

    fn branch_registry_probe_from_value(value: &serde_json::Value) -> serde_json::Value {
        let source_nodes = value
            .get("source_nodes")
            .and_then(serde_json::Value::as_array)
            .expect("source_nodes array");
        let source = source_nodes
            .iter()
            .find(|source| {
                source
                    .get("source_state_id")
                    .and_then(serde_json::Value::as_str)
                    == Some("branch-d176496f54e6e755")
            })
            .expect("selected source");
        let selected_branch_id = source
            .get("selected_branch_id")
            .and_then(serde_json::Value::as_str)
            .expect("selected_branch_id");
        let selected = source
            .get("branches")
            .and_then(serde_json::Value::as_array)
            .expect("branches array")
            .iter()
            .find(|branch| {
                branch.get("branch_id").and_then(serde_json::Value::as_str)
                    == Some(selected_branch_id)
            })
            .expect("selected branch");

        serde_json::json!({
            "campaign_id": value.get("campaign_id"),
            "source_node_count": source_nodes.len(),
            "branch_count": source_nodes
                .iter()
                .map(|source| {
                    source
                        .get("branches")
                        .and_then(serde_json::Value::as_array)
                        .map_or(0, Vec::len)
                })
                .sum::<usize>(),
            "selected_source": {
                "source_state_id": source.get("source_state_id"),
                "selected_branch_id": source.get("selected_branch_id"),
                "selected_branch": {
                    "branch_id": selected.get("branch_id"),
                    "candidate_id": selected.get("candidate_id"),
                    "status": selected.get("status"),
                    "latest_evaluation": selected.get("latest_evaluation"),
                }
            }
        })
    }

    fn branch_registry_probe_from_record(record: &Prototype1BranchRegistry) -> serde_json::Value {
        let source = record
            .source_nodes
            .iter()
            .find(|source| source.source_state_id == "branch-d176496f54e6e755")
            .expect("selected source");
        let selected_branch_id = source
            .selected_branch_id
            .as_deref()
            .expect("selected_branch_id");
        let selected = source
            .branches
            .iter()
            .find(|branch| branch.branch_id == selected_branch_id)
            .expect("selected branch");

        serde_json::json!({
            "campaign_id": record.campaign_id,
            "source_node_count": record.source_nodes.len(),
            "branch_count": record
                .source_nodes
                .iter()
                .map(|source| source.branches.len())
                .sum::<usize>(),
            "selected_source": {
                "source_state_id": source.source_state_id,
                "selected_branch_id": source.selected_branch_id,
                "selected_branch": {
                    "branch_id": selected.branch_id,
                    "candidate_id": selected.candidate_id,
                    "status": selected.status,
                    "latest_evaluation": selected.latest_evaluation,
                }
            }
        })
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
