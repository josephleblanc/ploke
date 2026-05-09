//! Passive Prototype 1 evaluation artifact DTOs.
//!
//! These records mirror persisted files under `prototype1/evaluations/*.json`.
//! They do not evaluate branches, select successors, or enforce authority.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::branch::Disposition;
use crate::value::JsonRecordValue;

/// One persisted Prototype 1 branch evaluation artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artifact {
    pub baseline_campaign_id: String,
    pub branch_id: String,
    pub treatment_campaign_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_procedure_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluator_identity: Option<Evaluator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_set_identity: Option<EvalSet>,
    pub branch_registry_path: PathBuf,
    pub evaluation_artifact_path: PathBuf,
    pub treatment_campaign_manifest: PathBuf,
    pub treatment_closure_state_path: PathBuf,
    pub overall_disposition: Disposition,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub compared_instances: Vec<InstanceComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Evaluator {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalSet {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_family: Option<String>,
    #[serde(default)]
    pub dataset_sources: Vec<DatasetSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_policy: Option<JsonRecordValue>,
    #[serde(default)]
    pub instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatasetSource {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceComparison {
    pub instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_registration_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_registration_path: Option<PathBuf>,
    #[serde(default)]
    pub baseline_record_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_record_path: Option<PathBuf>,
    #[serde(default)]
    pub baseline_metrics: Option<RunMetrics>,
    #[serde(default)]
    pub treatment_metrics: Option<RunMetrics>,
    #[serde(default)]
    pub evaluation: Option<Outcome>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunMetrics {
    pub tool_calls_total: u64,
    pub tool_calls_failed: u64,
    pub patch_attempted: bool,
    pub patch_apply_state: String,
    pub submission_artifact_state: String,
    pub partial_patch_failures: u64,
    pub same_file_patch_retry_count: u64,
    pub same_file_patch_max_streak: u64,
    pub aborted: bool,
    pub aborted_repair_loop: bool,
    pub nonempty_valid_patch: bool,
    pub convergence: bool,
    pub oracle_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Outcome {
    pub disposition: Disposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    #[ignore]
    fn real_branch_116821_roundtrips_value_and_preserves_metrics_surface() {
        let path = real_run_root().join("evaluations/branch-116821c1239b4022.json");
        let original = read_json_value(&path);
        let record: Artifact =
            serde_json::from_value(original.clone()).expect("deserialize branch-116821 report");

        assert_eq!(
            serde_json::to_value(&record).expect("serialize branch-116821 report"),
            original
        );
        assert_eq!(record.overall_disposition, Disposition::Keep);
        assert_eq!(record.reasons, Vec::<String>::new());
        assert_eq!(record.compared_instances.len(), 1);
        let compared = record
            .compared_instances
            .first()
            .expect("one compared instance");
        assert_eq!(compared.status, "compared");
        let baseline_metrics = compared
            .baseline_metrics
            .as_ref()
            .expect("baseline metrics present");
        assert_eq!(baseline_metrics.tool_calls_total, 12);
        assert_eq!(baseline_metrics.patch_apply_state, "no");
        let treatment_metrics = compared
            .treatment_metrics
            .as_ref()
            .expect("treatment metrics present");
        assert_eq!(treatment_metrics.tool_calls_total, 17);
        assert!(treatment_metrics.aborted);
        let evaluation = compared
            .evaluation
            .as_ref()
            .expect("evaluation present");
        assert_eq!(evaluation.disposition, Disposition::Keep);
    }

    #[test]
    #[ignore]
    fn real_branch_01187c_roundtrips_value_and_preserves_reject_surface() {
        let path = real_run_root().join("evaluations/branch-01187cd17226d1a4.json");
        let original = read_json_value(&path);
        let record: Artifact =
            serde_json::from_value(original.clone()).expect("deserialize branch-01187 report");

        assert_eq!(
            serde_json::to_value(&record).expect("serialize branch-01187 report"),
            original
        );
        assert_eq!(record.overall_disposition, Disposition::Reject);
        assert_eq!(record.reasons.len(), 1);
        assert!(
            record.reasons[0].contains("baseline arm does not have a complete record"),
            "expected baseline-missing reason, got: {}",
            record.reasons[0]
        );
        assert_eq!(record.compared_instances.len(), 1);
        let compared = record
            .compared_instances
            .first()
            .expect("one compared instance");
        assert_eq!(compared.status, "missing_baseline_record");
        assert!(compared.baseline_metrics.is_none());
        assert!(compared.evaluation.is_none());
        let treatment_metrics = compared
            .treatment_metrics
            .as_ref()
            .expect("treatment metrics present");
        assert_eq!(treatment_metrics.tool_calls_failed, 3);
        assert_eq!(treatment_metrics.submission_artifact_state, "empty");
    }

    #[test]
    #[ignore]
    fn print_real_evaluation_probe() {
        let path = real_run_root().join("evaluations/branch-116821c1239b4022.json");
        let value = read_json_value(&path);
        let report: Artifact =
            serde_json::from_value(value).expect("deserialize report");
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "branch_id": report.branch_id,
                "overall_disposition": report.overall_disposition,
                "reasons": report.reasons,
                "compared_instances": report.compared_instances.iter().map(|row| {
                    serde_json::json!({
                        "instance_id": row.instance_id,
                        "status": row.status,
                        "has_baseline_metrics": row.baseline_metrics.is_some(),
                        "has_treatment_metrics": row.treatment_metrics.is_some(),
                        "has_evaluation": row.evaluation.is_some(),
                    })
                }).collect::<Vec<_>>(),
            }))
            .expect("probe to json")
        );
    }

    #[test]
    #[ignore]
    fn real_run_all_evaluation_artifacts_roundtrip_and_match_expected_counts() {
        let evaluations_dir = real_run_root().join("evaluations");
        let mut paths = fs::read_dir(&evaluations_dir)
            .unwrap_or_else(|err| {
                panic!(
                    "read evaluations directory {} (set PLOKE_RECORDS_REAL_RUN_ROOT to override): {err}",
                    evaluations_dir.display()
                )
            })
            .map(|entry| entry.expect("read evaluation entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();

        let mut keep_count = 0usize;
        let mut reject_count = 0usize;
        let mut nested_reason_files = 0usize;

        for path in &paths {
            let original = read_json_value(path);
            let record: Artifact = serde_json::from_value(original.clone())
                .unwrap_or_else(|err| panic!("deserialize evaluation {}: {err}", path.display()));
            let serialized = serde_json::to_value(&record)
                .unwrap_or_else(|err| panic!("serialize evaluation {}: {err}", path.display()));
            assert_eq!(
                serialized,
                original,
                "round-trip mismatch for {}",
                path.display()
            );

            match record.overall_disposition {
                Disposition::Keep => keep_count += 1,
                Disposition::Reject => reject_count += 1,
            }

            if record.compared_instances.iter().any(|instance| {
                instance
                    .evaluation
                    .as_ref()
                    .map(|evaluation| !evaluation.reasons.is_empty())
                    .unwrap_or(false)
            }) {
                nested_reason_files += 1;
            }
        }

        assert_eq!(paths.len(), 36, "unexpected evaluation file count");
        assert_eq!(keep_count, 20, "unexpected keep disposition count");
        assert_eq!(reject_count, 16, "unexpected reject disposition count");
        assert_eq!(
            nested_reason_files, 6,
            "unexpected files-with-nested-evaluation-reasons count"
        );
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
                "read real-run evaluation {} (set PLOKE_RECORDS_REAL_RUN_ROOT to override): {err}",
                path.display()
            )
        });
        serde_json::from_str(&text).expect("parse real-run JSON value")
    }
}
