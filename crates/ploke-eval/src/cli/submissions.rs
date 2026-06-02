use std::fs;
use std::path::PathBuf;

use crate::closure::{ClosureClass, ClosureInstanceRow, ClosureState};
use crate::run_history::{RunDirPreference, preferred_run_dir_for_instance};
use crate::runner::MultiSweBenchSubmissionRecord;
use crate::spec::PrepareError;

pub(super) fn default_campaign_submission_export_path(
    campaign_id: &str,
    nonempty_only: bool,
) -> Result<PathBuf, PrepareError> {
    let file_name = if nonempty_only {
        "multi-swe-bench-submission.nonempty.jsonl"
    } else {
        "multi-swe-bench-submission.jsonl"
    };
    Ok(crate::layout::campaigns_dir()?
        .join(campaign_id)
        .join(file_name))
}

pub(super) fn collect_campaign_submission_records(
    state: &ClosureState,
    nonempty_only: bool,
) -> Result<Vec<MultiSweBenchSubmissionRecord>, PrepareError> {
    let mut records = Vec::new();
    for row in &state.instances {
        if row.eval_status != ClosureClass::Complete {
            continue;
        }
        let record = load_submission_record_for_row(state, row)?;
        if nonempty_only && record.fix_patch.trim().is_empty() {
            continue;
        }
        records.push(record);
    }
    Ok(records)
}

pub(super) fn count_campaign_empty_patch_rows(state: &ClosureState) -> Result<usize, PrepareError> {
    let mut count = 0;
    for row in &state.instances {
        if row.eval_status != ClosureClass::Complete {
            continue;
        }
        let record = load_submission_record_for_row(state, row)?;
        if record.fix_patch.trim().is_empty() {
            count += 1;
        }
    }
    Ok(count)
}

pub(super) fn load_submission_record_for_row(
    state: &ClosureState,
    row: &ClosureInstanceRow,
) -> Result<MultiSweBenchSubmissionRecord, PrepareError> {
    let path = if let Some(path) = row.artifacts.msb_submission.as_ref() {
        path.clone()
    } else {
        let instance_root = state.config.instances_root.join(&row.instance_id);
        let run_dir = preferred_run_dir_for_instance(
            &state.config.instances_root,
            &row.instance_id,
            RunDirPreference::PreferTreatmentWithSubmission,
        )?
        .unwrap_or(instance_root);
        run_dir.join("multi-swe-bench-submission.jsonl")
    };
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(text.trim()).map_err(|source| PrepareError::ParseManifest { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target_registry::BenchmarkFamily;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn sample_closure_state_for_submission_export(
        instances_root: PathBuf,
    ) -> crate::closure::ClosureState {
        crate::closure::ClosureState {
            schema_version: crate::closure::CLOSURE_STATE_SCHEMA_VERSION.to_string(),
            campaign_id: "campaign-1".to_string(),
            updated_at: "2026-04-17T00:00:00Z".to_string(),
            config: crate::closure::ClosureConfig {
                benchmark_family: BenchmarkFamily::MultiSweBenchRust,
                model_id: Some("x-ai/grok-4-fast".to_string()),
                provider_slug: Some("xai".to_string()),
                registry_path: None,
                dataset_sources: Vec::new(),
                required_procedures: Vec::new(),
                instances_root,
                batches_root: PathBuf::from("/tmp/batches"),
                framework: crate::spec::FrameworkConfig::default(),
            },
            registry: crate::closure::RegistryClosureSummary {
                expected_total: 3,
                mapped_total: 3,
                missing_total: 0,
                ambiguous_total: 0,
                status: ClosureClass::Complete,
            },
            eval: crate::closure::EvalClosureSummary {
                expected_total: 3,
                complete_total: 2,
                failed_total: 1,
                missing_total: 0,
                partial_total: 0,
                in_progress_total: 0,
                status: ClosureClass::Partial,
                last_transition_at: None,
            },
            protocol: crate::closure::ProtocolClosureSummary {
                expected_total: 0,
                full_total: 0,
                partial_total: 0,
                failed_total: 0,
                missing_total: 0,
                incompatible_total: 0,
                ineligible_total: 0,
                in_progress_total: 0,
                status: ClosureClass::Complete,
                required_procedures: Vec::new(),
                status_by_procedure: BTreeMap::new(),
                last_transition_at: None,
            },
            instances: vec![
                crate::closure::ClosureInstanceRow {
                    instance_id: "org__repo-1".to_string(),
                    dataset_label: "org__repo".to_string(),
                    repo_family: "org__repo".to_string(),
                    registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                    eval_status: ClosureClass::Complete,
                    protocol_status: ClosureClass::Complete,
                    eval_failure: None,
                    protocol_failure: None,
                    artifacts: crate::closure::ClosureArtifactRefs::default(),
                    protocol_procedures: BTreeMap::new(),
                    protocol_counts: None,
                    last_event_at: None,
                },
                crate::closure::ClosureInstanceRow {
                    instance_id: "org__repo-2".to_string(),
                    dataset_label: "org__repo".to_string(),
                    repo_family: "org__repo".to_string(),
                    registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                    eval_status: ClosureClass::Complete,
                    protocol_status: ClosureClass::Complete,
                    eval_failure: None,
                    protocol_failure: None,
                    artifacts: crate::closure::ClosureArtifactRefs::default(),
                    protocol_procedures: BTreeMap::new(),
                    protocol_counts: None,
                    last_event_at: None,
                },
                crate::closure::ClosureInstanceRow {
                    instance_id: "org__repo-3".to_string(),
                    dataset_label: "org__repo".to_string(),
                    repo_family: "org__repo".to_string(),
                    registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                    eval_status: ClosureClass::Failed,
                    protocol_status: ClosureClass::Ineligible,
                    eval_failure: Some("failed".to_string()),
                    protocol_failure: None,
                    artifacts: crate::closure::ClosureArtifactRefs::default(),
                    protocol_procedures: BTreeMap::new(),
                    protocol_counts: None,
                    last_event_at: None,
                },
            ],
        }
    }

    #[test]
    fn collect_campaign_submission_records_filters_empty_patches_only_when_requested() {
        let tmp = tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let instances_root = eval_home.join("instances");
        fs::create_dir_all(&instances_root).expect("instances root");

        let run_a_submission = eval_home.join("run-a-submission.jsonl");
        let run_b_submission = eval_home.join("run-b-submission.jsonl");
        fs::write(
            &run_a_submission,
            serde_json::to_string(&MultiSweBenchSubmissionRecord {
                org: "org".to_string(),
                repo: "repo".to_string(),
                number: 1,
                fix_patch: "diff --git a/src/lib.rs b/src/lib.rs\n".to_string(),
            })
            .expect("json"),
        )
        .expect("write submission 1");
        fs::write(
            &run_b_submission,
            serde_json::to_string(&MultiSweBenchSubmissionRecord {
                org: "org".to_string(),
                repo: "repo".to_string(),
                number: 2,
                fix_patch: String::new(),
            })
            .expect("json"),
        )
        .expect("write submission 2");

        let mut state = sample_closure_state_for_submission_export(instances_root);
        state.instances[0].artifacts.msb_submission = Some(run_a_submission);
        state.instances[1].artifacts.msb_submission = Some(run_b_submission);

        let all_records =
            collect_campaign_submission_records(&state, false).expect("all records export");
        let nonempty_records =
            collect_campaign_submission_records(&state, true).expect("nonempty export");

        assert_eq!(all_records.len(), 2);
        assert_eq!(nonempty_records.len(), 1);
        assert_eq!(nonempty_records[0].number, 1);
        assert_eq!(
            count_campaign_empty_patch_rows(&state).expect("empty rows"),
            1
        );
    }

    #[test]
    fn collect_campaign_submission_records_prefers_treatment_submission_over_newer_control_run() {
        let tmp = tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let instances_root = eval_home.join("instances");
        fs::create_dir_all(&instances_root).expect("instances root");

        let submission_path = eval_home.join("treatment-submission.jsonl");
        fs::write(
            &submission_path,
            serde_json::to_string(&MultiSweBenchSubmissionRecord {
                org: "org".to_string(),
                repo: "repo".to_string(),
                number: 1,
                fix_patch: "diff --git a/src/lib.rs b/src/lib.rs\n".to_string(),
            })
            .expect("json"),
        )
        .expect("write treatment submission");

        let mut state = sample_closure_state_for_submission_export(instances_root);
        state.instances.truncate(1);
        state.instances[0].artifacts.msb_submission = Some(submission_path);

        let records =
            collect_campaign_submission_records(&state, false).expect("all records export");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].number, 1);
        assert_eq!(
            records[0].fix_patch,
            "diff --git a/src/lib.rs b/src/lib.rs\n"
        );
    }

    #[test]
    fn default_campaign_submission_export_path_uses_campaign_directory() {
        let path =
            default_campaign_submission_export_path("campaign-1", false).expect("default path");
        assert!(path.ends_with("campaign-1/multi-swe-bench-submission.jsonl"));

        let nonempty_path =
            default_campaign_submission_export_path("campaign-1", true).expect("nonempty path");
        assert!(nonempty_path.ends_with("campaign-1/multi-swe-bench-submission.nonempty.jsonl"));
    }
}

