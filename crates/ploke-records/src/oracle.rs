//! Passive MBE oracle evidence DTOs.
//!
//! These records mirror MBE report/evaluation shapes persisted or embedded by
//! `ploke-eval`. They do not run the oracle, validate campaign policy, or make
//! successor-selection decisions.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Resolved,
    Unresolved,
    EmptyPatch,
    Incomplete,
    Error,
    NotSubmitted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub report_path: PathBuf,
    pub instance_id: String,
    pub report_id: String,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evaluation {
    pub evidence: Evidence,
    pub instance_report_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_report: Option<InstanceReport>,
    pub diagnostic: Diagnostic,
    pub usable_for_selection: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnostic {
    Resolved,
    UnresolvedTestsRan,
    FixCompileFailed,
    MissingFixResults,
    InvalidInstanceReport,
    MissingInstanceReport,
    EmptyPatch,
    Incomplete,
    Error,
    NotSubmitted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceReport {
    pub org: String,
    pub repo: String,
    pub number: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_msg: Option<String>,
    #[serde(default)]
    pub fixed_tests: BTreeMap<String, TestTransition>,
    #[serde(default)]
    pub p2p_tests: BTreeMap<String, TestTransition>,
    #[serde(default)]
    pub f2p_tests: BTreeMap<String, TestTransition>,
    #[serde(default)]
    pub s2p_tests: BTreeMap<String, TestTransition>,
    #[serde(default)]
    pub n2p_tests: BTreeMap<String, TestTransition>,
    pub run_result: StageResult,
    pub test_patch_result: StageResult,
    pub fix_patch_result: StageResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageResult {
    pub passed_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    #[serde(default)]
    pub passed_tests: BTreeSet<String>,
    #[serde(default)]
    pub failed_tests: BTreeSet<String>,
    #[serde(default)]
    pub skipped_tests: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestTransition {
    pub run: TestStatus,
    pub test: TestStatus,
    pub fix: TestStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestStatus {
    #[serde(rename = "PASS")]
    Pass,
    #[serde(rename = "FAIL")]
    Fail,
    #[serde(rename = "SKIP")]
    Skip,
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "FAILED")]
    Failed,
    #[serde(rename = "PASSED")]
    Passed,
    #[serde(rename = "SKIPPED")]
    Skipped,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "XFAIL")]
    Xfail,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_evaluation_json_roundtrips() {
        let evaluation = sample_evaluation();

        let json = serde_json::to_string_pretty(&evaluation).expect("serialize oracle");
        let decoded: Evaluation = serde_json::from_str(&json).expect("deserialize oracle");

        assert_eq!(decoded, evaluation);
    }

    #[test]
    fn oracle_evaluation_embeds_in_passive_records() {
        let evaluation = sample_evaluation();
        let selection = crate::selection::RunComparison {
            instance_id: evaluation.evidence.instance_id.clone(),
            parent_metrics: None,
            child_metrics: None,
            oracle_evaluation: Some(evaluation.clone()),
            status: "compared".to_string(),
        };
        let selection_json = serde_json::to_string(&selection).expect("serialize selection");
        let decoded_selection: crate::selection::RunComparison =
            serde_json::from_str(&selection_json).expect("deserialize selection");
        assert_eq!(decoded_selection, selection);

        let instance = crate::evaluation::InstanceComparison {
            instance_id: evaluation.evidence.instance_id.clone(),
            baseline_registration_path: None,
            treatment_registration_path: None,
            baseline_record_path: None,
            treatment_record_path: None,
            baseline_metrics: None,
            treatment_metrics: None,
            oracle_evaluation: Some(evaluation.clone()),
            evaluation: None,
            status: "compared".to_string(),
        };
        let instance_json = serde_json::to_string(&instance).expect("serialize evaluation");
        let decoded_instance: crate::evaluation::InstanceComparison =
            serde_json::from_str(&instance_json).expect("deserialize evaluation");
        assert_eq!(decoded_instance, instance);

        let compared = crate::history::ComparedRunEvidenceRecord {
            instance_id: Some(evaluation.evidence.instance_id.clone()),
            status: Some("compared".to_string()),
            baseline_citation: None,
            treatment_citation: None,
            baseline_metrics: None,
            treatment_metrics: None,
            oracle_evaluation: Some(evaluation),
            baseline_protocol: None,
            treatment_protocol: None,
            diagnostics: Vec::new(),
            baseline_run: None,
            treatment_run: None,
        };
        let compared_json = serde_json::to_string(&compared).expect("serialize history");
        let decoded_compared: crate::history::ComparedRunEvidenceRecord =
            serde_json::from_str(&compared_json).expect("deserialize history");
        assert_eq!(decoded_compared, compared);
    }

    fn sample_evaluation() -> Evaluation {
        Evaluation {
            evidence: Evidence {
                report_path: PathBuf::from("/tmp/mbe/final_report.json"),
                instance_id: "BurntSushi__ripgrep-2209".to_string(),
                report_id: "BurntSushi/ripgrep:pr-2209".to_string(),
                verdict: Verdict::Resolved,
            },
            instance_report_path: PathBuf::from(
                "/tmp/mbe/workdir/BurntSushi/ripgrep/evals/pr-2209/report.json",
            ),
            instance_report: Some(InstanceReport {
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number: 2209,
                valid: Some(true),
                error_msg: None,
                fixed_tests: BTreeMap::from([(
                    "test_a".to_string(),
                    TestTransition {
                        run: TestStatus::Fail,
                        test: TestStatus::Fail,
                        fix: TestStatus::Pass,
                    },
                )]),
                p2p_tests: BTreeMap::new(),
                f2p_tests: BTreeMap::new(),
                s2p_tests: BTreeMap::new(),
                n2p_tests: BTreeMap::new(),
                run_result: stage(0, 1, 0),
                test_patch_result: stage(0, 1, 0),
                fix_patch_result: stage(1, 0, 0),
            }),
            diagnostic: Diagnostic::Resolved,
            usable_for_selection: true,
        }
    }

    fn stage(passed: usize, failed: usize, skipped: usize) -> StageResult {
        StageResult {
            passed_count: passed,
            failed_count: failed,
            skipped_count: skipped,
            passed_tests: (0..passed).map(|index| format!("passed-{index}")).collect(),
            failed_tests: (0..failed).map(|index| format!("failed-{index}")).collect(),
            skipped_tests: (0..skipped)
                .map(|index| format!("skipped-{index}"))
                .collect(),
        }
    }
}
