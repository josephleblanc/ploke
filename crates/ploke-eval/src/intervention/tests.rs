use std::fs;
use std::path::PathBuf;

use ploke_core::ArcStr;
use ploke_core::tool_types::ToolName;
use ploke_protocol::tool_calls::segment::{IntentLabel, SegmentStatus};
use ploke_tui::tools::ToolUiPayload;
use tempfile::TempDir;
use uuid::Uuid;

use super::{
    ArtifactEdit, InterventionExecutionInput, InterventionSpec, InterventionSpecError,
    InterventionSynthesisInput, IssueDetectionInput, IssueSelectionBasis, ValidationPolicy,
    detect_issue_cases, execute_tool_text_intervention, select_primary_issue,
    synthesize_intervention,
};
use crate::protocol::protocol_aggregate::{
    ProtocolAggregate, ProtocolArtifactRef, ProtocolBranchAssessment, ProtocolCallReviewRow,
    ProtocolCoverage, ProtocolDerivedMetrics, ProtocolReviewSignals, ProtocolRunIdentity,
    ProtocolSegmentBasis, ProtocolSegmentationAnchor, ProtocolSegmentationCoverage,
};
use crate::record::{
    AgentMetadata, BenchmarkMetadata, RunMetadata, RunPhases, RunRecord, ToolExecutionRecord,
    ToolResult, TurnOutcome, TurnRecord,
};
use crate::runner::{RunArm, ToolCompletedRecord, ToolFailedRecord};
use crate::spec::{EvalBudget, IssueInput};

fn seed_repo() -> TempDir {
    let temp = TempDir::new().expect("tempdir");
    let tool_text_dir = temp.path().join("crates/ploke-core/tool_text");
    fs::create_dir_all(&tool_text_dir).expect("tool_text_dir");
    fs::write(
        tool_text_dir.join("non_semantic_patch.md"),
        "Header\n\nOriginal body.\n",
    )
    .expect("seed tool text");
    temp
}

fn base_record() -> RunRecord {
    RunRecord {
        schema_version: crate::record::RUN_RECORD_SCHEMA_VERSION.to_string(),
        manifest_id: "case-1".to_string(),
        metadata: RunMetadata {
            run_arm: RunArm::structured_current_policy_treatment(),
            benchmark: BenchmarkMetadata {
                instance_id: "case-1".to_string(),
                repo_root: PathBuf::from("/tmp/repo"),
                base_sha: Some("abc123".to_string()),
                issue: Some(IssueInput {
                    title: Some("issue".to_string()),
                    body: Some("body".to_string()),
                    body_path: None,
                }),
            },
            agent: AgentMetadata::default(),
            runtime: crate::record::RuntimeMetadata::default(),
            budget: EvalBudget::default(),
        },
        phases: RunPhases::default(),
        db_time_travel_index: Vec::new(),
        conversation: Vec::new(),
        timing: None,
    }
}

fn turn_record(outcome: TurnOutcome, tool_calls: Vec<ToolExecutionRecord>) -> TurnRecord {
    TurnRecord {
        turn_number: 1,
        started_at: "2026-04-23T00:00:00Z".to_string(),
        ended_at: "2026-04-23T00:00:01Z".to_string(),
        db_timestamp_micros: 0,
        issue_prompt: "fix it".to_string(),
        llm_request: None,
        llm_response: None,
        tool_calls,
        outcome,
        agent_turn_artifact: None,
    }
}

fn completed_edit_call(
    tool: ToolName,
    arguments: serde_json::Value,
    applied: usize,
    partial: bool,
) -> ToolExecutionRecord {
    let request_id = Uuid::new_v4();
    let call_id = ArcStr::from(format!("call-{}", Uuid::new_v4()));
    let ui_payload = ToolUiPayload::new(tool, call_id.clone(), "done")
        .with_request_id(request_id)
        .with_field("applied", applied.to_string())
        .with_field("partial", partial.to_string());

    ToolExecutionRecord {
        request: crate::runner::ToolRequestRecord {
            request_id: request_id.to_string(),
            parent_id: Uuid::new_v4().to_string(),
            call_id: call_id.to_string(),
            tool: tool.as_str().to_string(),
            arguments: serde_json::to_string(&arguments).expect("serialize args"),
        },
        result: ToolResult::Completed(ToolCompletedRecord {
            request_id: request_id.to_string(),
            parent_id: Uuid::new_v4().to_string(),
            call_id: call_id.to_string(),
            tool: tool.as_str().to_string(),
            content: "{}".to_string(),
            ui_payload: Some(ui_payload),
            latency_ms: 10,
        }),
        latency_ms: 10,
    }
}

fn failed_edit_call(
    tool: ToolName,
    arguments: serde_json::Value,
    error: &str,
) -> ToolExecutionRecord {
    let request_id = Uuid::new_v4();
    let call_id = ArcStr::from(format!("call-{}", Uuid::new_v4()));
    ToolExecutionRecord {
        request: crate::runner::ToolRequestRecord {
            request_id: request_id.to_string(),
            parent_id: Uuid::new_v4().to_string(),
            call_id: call_id.to_string(),
            tool: tool.as_str().to_string(),
            arguments: serde_json::to_string(&arguments).expect("serialize args"),
        },
        result: ToolResult::Failed(ToolFailedRecord {
            request_id: request_id.to_string(),
            parent_id: Uuid::new_v4().to_string(),
            call_id: call_id.to_string(),
            tool: Some(tool.as_str().to_string()),
            error: error.to_string(),
            ui_payload: None,
            latency_ms: 10,
        }),
        latency_ms: 10,
    }
}

fn raw_patch_args(file: &str) -> serde_json::Value {
    serde_json::json!({
        "patches": [
            {
                "file": file,
                "diff": "@@ -1,1 +1,1 @@"
            }
        ]
    })
}

fn protocol_aggregate_with_recovery_review(
    total_calls: usize,
    segment_start: usize,
    segment_end: usize,
    reviewed_call_index: usize,
) -> ProtocolAggregate {
    ProtocolAggregate {
        run: ProtocolRunIdentity {
            record_path: PathBuf::from("/tmp/run.json"),
            run_dir: PathBuf::from("/tmp/run"),
            run_id: "case-1".to_string(),
            subject_id: "case-1".to_string(),
        },
        coverage: ProtocolCoverage {
            scanned_artifact_count: 3,
            artifact_counts: Default::default(),
            total_calls_in_run: total_calls,
            total_segments_in_anchor: 1,
            reviewed_call_count: 1,
            reviewed_segment_count: 0,
            missing_call_indices: Vec::new(),
            missing_segment_indices: Vec::new(),
            skipped_segment_review_count: 0,
            segment_anchor_mismatch_count: 0,
        },
        segmentation: ProtocolSegmentationAnchor {
            artifact: ProtocolArtifactRef {
                path: PathBuf::from("/tmp/anchor.json"),
                created_at_ms: 1,
                procedure_name: "tool_call_intent_segmentation".to_string(),
            },
            coverage: ProtocolSegmentationCoverage {
                total_calls,
                labeled_calls: total_calls,
                ambiguous_calls: 0,
                labeled_segments: 1,
                ambiguous_segments: 0,
                uncovered_calls: 0,
            },
            segments: vec![ProtocolSegmentBasis {
                segment_index: 0,
                label: Some(IntentLabel::Recovery),
                status: SegmentStatus::Labeled,
                confidence: None,
                rationale: Some("recovery span".to_string()),
                start_index: segment_start,
                end_index: segment_end,
                turn_span: vec![1],
                call_indices: (segment_start..=segment_end).collect(),
                call_count: segment_end - segment_start + 1,
            }],
        },
        call_reviews: vec![ProtocolCallReviewRow {
            artifact: ProtocolArtifactRef {
                path: PathBuf::from("/tmp/review.json"),
                created_at_ms: 2,
                procedure_name: "tool_call_review".to_string(),
            },
            focal_call_index: reviewed_call_index,
            segment_index: Some(0),
            segment_label: Some("recovery".to_string()),
            segment_status: Some("labeled".to_string()),
            scope_call_indices: (segment_start..=segment_end).collect(),
            total_calls_in_run: total_calls,
            total_calls_in_scope: segment_end - segment_start + 1,
            turn_span: vec![1],
            overall: "recoverable_misstep".to_string(),
            overall_confidence: "medium".to_string(),
            usefulness: ProtocolBranchAssessment {
                verdict: "mixed".to_string(),
                confidence: "medium".to_string(),
                rationale: "test".to_string(),
            },
            redundancy: ProtocolBranchAssessment {
                verdict: "redundant".to_string(),
                confidence: "medium".to_string(),
                rationale: "test".to_string(),
            },
            recoverability: ProtocolBranchAssessment {
                verdict: "recoverable".to_string(),
                confidence: "high".to_string(),
                rationale: "test".to_string(),
            },
            signals: ProtocolReviewSignals {
                candidate_concerns: vec![
                    "same_file_raw_patch_retry".to_string(),
                    "consider_semantic_edit".to_string(),
                ],
                ..Default::default()
            },
        }],
        segment_reviews: Vec::new(),
        crosswalk: Vec::new(),
        derived_metrics: ProtocolDerivedMetrics {
            call_review_overall_counts: Default::default(),
            segment_review_overall_counts: Default::default(),
            call_review_confidence_counts: Default::default(),
            segment_review_confidence_counts: Default::default(),
            calls_with_segment_crosswalk: 1,
            calls_without_segment_crosswalk: 0,
            average_calls_per_anchor_segment: total_calls as f64,
        },
        skipped_segment_reviews: Vec::new(),
    }
}

#[test]
fn tool_text_adapter_materializes_stages_applies_and_validates_replace_whole_text() {
    let repo = seed_repo();
    let spec = InterventionSpec::ToolGuidanceMutation {
        spec_id: "p1".to_string(),
        evidence_basis: "test".to_string(),
        intended_effect: "tighten guidance".to_string(),
        tool: ToolName::NsPatch,
        validation_policy: ValidationPolicy {
            allowed_relpaths: vec![PathBuf::from(
                ToolName::NsPatch.description_artifact_relpath(),
            )],
            require_target_exists: true,
            require_nonempty_result: true,
            require_utf8: true,
            require_content_change: true,
            require_markers_after_apply: vec!["Updated guidance.".to_string()],
            require_cargo_check: false,
        },
        edit: ArtifactEdit::ReplaceWholeText {
            new_text: "Header\n\nUpdated guidance.\n".to_string(),
        },
    };
    let output = execute_tool_text_intervention(&InterventionExecutionInput {
        repo_root: repo.path().to_path_buf(),
        spec,
    })
    .expect("execute intervention");

    assert!(output.validation.ok);
    let written = fs::read_to_string(
        repo.path()
            .join("crates/ploke-core/tool_text/non_semantic_patch.md"),
    )
    .expect("read written");
    assert_eq!(written, "Header\n\nUpdated guidance.\n");
}

#[test]
fn policy_config_target_is_explicitly_unimplemented_for_first_adapter() {
    let repo = seed_repo();
    let spec = InterventionSpec::PolicyConfigMutation {
        spec_id: "p2".to_string(),
        evidence_basis: "test".to_string(),
        intended_effect: "toggle policy".to_string(),
        relpath: PathBuf::from("crates/ploke-eval/src/intervention_policy.rs"),
        validation_policy: ValidationPolicy::for_tool_description_target(ToolName::NsPatch),
        edit: ArtifactEdit::AppendText {
            text: "\npolicy".to_string(),
        },
    };

    let err = execute_tool_text_intervention(&InterventionExecutionInput {
        repo_root: repo.path().to_path_buf(),
        spec,
    })
    .expect_err("unimplemented");
    assert!(matches!(
        err,
        InterventionSpecError::Unimplemented("policy/config intervention adapter")
    ));
}

#[test]
fn validator_rejects_disallowed_target_paths() {
    let repo = seed_repo();
    let spec = InterventionSpec::ToolGuidanceMutation {
        spec_id: "p3".to_string(),
        evidence_basis: "test".to_string(),
        intended_effect: "tighten guidance".to_string(),
        tool: ToolName::NsPatch,
        validation_policy: ValidationPolicy {
            allowed_relpaths: vec![PathBuf::from(
                ToolName::ApplyCodeEdit.description_artifact_relpath(),
            )],
            require_target_exists: true,
            require_nonempty_result: true,
            require_utf8: true,
            require_content_change: true,
            require_markers_after_apply: Vec::new(),
            require_cargo_check: false,
        },
        edit: ArtifactEdit::AppendText {
            text: "Updated guidance.\n".to_string(),
        },
    };

    let err = execute_tool_text_intervention(&InterventionExecutionInput {
        repo_root: repo.path().to_path_buf(),
        spec,
    })
    .expect_err("disallowed target");

    assert!(matches!(err, InterventionSpecError::TargetNotAllowed(_)));
}

#[test]
fn detect_issue_cases_finds_reviewed_tool_case() {
    let mut record = base_record();
    record.phases.agent_turns.push(turn_record(
        TurnOutcome::Error {
            message: "aborted".to_string(),
        },
        vec![
            failed_edit_call(
                ToolName::ApplyCodeEdit,
                serde_json::json!({"file":"src/bytes_mut.rs"}),
                "ambiguous canonical target",
            ),
            failed_edit_call(
                ToolName::NsPatch,
                raw_patch_args("src/bytes_mut.rs"),
                "Patch applied partially. See report for details.",
            ),
            completed_edit_call(
                ToolName::NsPatch,
                raw_patch_args("src/bytes_mut.rs"),
                0,
                true,
            ),
            failed_edit_call(
                ToolName::NsPatch,
                raw_patch_args("src/bytes_mut.rs"),
                "Patch applied partially. See report for details.",
            ),
        ],
    ));

    let output = detect_issue_cases(&IssueDetectionInput::from_record(
        record,
        Some(protocol_aggregate_with_recovery_review(4, 1, 3, 1)),
    ));
    assert_eq!(output.cases.len(), 1);
    let issue = select_primary_issue(&output).expect("select primary issue");
    assert_eq!(
        issue.selection_basis,
        IssueSelectionBasis::ProtocolReviewedIssueCalls
    );
    assert_eq!(issue.target_tool, ToolName::NsPatch);
    assert_eq!(issue.evidence.reviewed_call_count, 1);
    assert_eq!(issue.evidence.reviewed_issue_call_count, 1);
    assert_eq!(issue.evidence.protocol.reviewed_segment_indices, vec![0]);
    assert!(
        issue
            .evidence
            .protocol
            .candidate_concerns
            .contains(&"same_file_raw_patch_retry".to_string())
    );
}

#[test]
fn detect_issue_cases_requires_protocol_review_evidence() {
    let mut record = base_record();
    record.phases.agent_turns.push(turn_record(
        TurnOutcome::Error {
            message: "aborted".to_string(),
        },
        vec![
            failed_edit_call(
                ToolName::ApplyCodeEdit,
                serde_json::json!({"file":"src/lib.rs"}),
                "ambiguous canonical target",
            ),
            failed_edit_call(
                ToolName::NsPatch,
                raw_patch_args("src/lib.rs"),
                "Patch applied partially. See report for details.",
            ),
            failed_edit_call(
                ToolName::NsPatch,
                raw_patch_args("src/lib.rs"),
                "Patch applied partially. See report for details.",
            ),
        ],
    ));

    let output = detect_issue_cases(&IssueDetectionInput::from_record(record, None));
    assert!(output.cases.is_empty());
}

#[test]
fn synthesized_reviewed_tool_target_executes_against_tool_text_surface() {
    let repo = seed_repo();
    let mut record = base_record();
    record.phases.agent_turns.push(turn_record(
        TurnOutcome::Error {
            message: "aborted".to_string(),
        },
        vec![
            failed_edit_call(
                ToolName::ApplyCodeEdit,
                serde_json::json!({"file":"src/bytes_mut.rs"}),
                "ambiguous canonical target",
            ),
            failed_edit_call(
                ToolName::NsPatch,
                raw_patch_args("src/bytes_mut.rs"),
                "Patch applied partially. See report for details.",
            ),
            completed_edit_call(
                ToolName::NsPatch,
                raw_patch_args("src/bytes_mut.rs"),
                0,
                true,
            ),
            failed_edit_call(
                ToolName::NsPatch,
                raw_patch_args("src/bytes_mut.rs"),
                "Patch applied partially. See report for details.",
            ),
        ],
    ));

    let detection = detect_issue_cases(&IssueDetectionInput::from_record(
        record,
        Some(protocol_aggregate_with_recovery_review(4, 1, 3, 1)),
    ));
    let issue = select_primary_issue(&detection).expect("select primary issue");
    let synthesized = synthesize_intervention(&InterventionSynthesisInput { issue })
        .expect("synthesize intervention");
    let output = execute_tool_text_intervention(&InterventionExecutionInput {
        repo_root: repo.path().to_path_buf(),
        spec: synthesized.selected_spec,
    })
    .expect("execute synthesized intervention");

    assert!(output.validation.ok);
    let written = fs::read_to_string(
        repo.path()
            .join("crates/ploke-core/tool_text/non_semantic_patch.md"),
    )
    .expect("read written");
    assert!(written.contains("Protocol review targeting note for `non_semantic_patch`."));
}
