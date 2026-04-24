use serde::{Deserialize, Serialize};

use crate::record::{
    RunRecord, SubmissionArtifactState, ToolExecutionRecord, ToolResult, TurnOutcome,
};

const EDIT_TOOL_NAMES: [&str; 4] = [
    "apply_code_edit",
    "insert_rust_item",
    "create_file",
    "non_semantic_patch",
];

const RAW_PATCH_TOOL_NAME: &str = "non_semantic_patch";
const REPAIR_LOOP_STREAK_THRESHOLD: usize = 3;
const PARTIAL_PATCH_ERROR_TEXT: &str = "Patch applied partially";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyState {
    No,
    Partial,
    Applied,
}

impl PatchApplyState {
    pub fn as_str(self) -> &'static str {
        match self {
            PatchApplyState::No => "no",
            PatchApplyState::Partial => "partial",
            PatchApplyState::Applied => "applied",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalRunMetrics {
    pub tool_calls_total: usize,
    pub tool_calls_failed: usize,
    pub patch_attempted: bool,
    pub patch_apply_state: PatchApplyState,
    /// Recorded benchmark submission artifact state from the packaging phase.
    ///
    /// This is the concrete packaging/output fact, separate from `nonempty_valid_patch`.
    /// Keep both: the submission artifact may be absent or empty even when the run looked
    /// operationally coherent, and legacy records may not have packaging persisted at all.
    pub submission_artifact_state: SubmissionArtifactState,
    pub partial_patch_failures: usize,
    pub same_file_patch_retry_count: usize,
    pub same_file_patch_max_streak: usize,
    pub aborted: bool,
    pub aborted_repair_loop: bool,
    /// Conservative pre-oracle proxy for "usable candidate fix exists".
    ///
    /// Today this means the run reached a fully applied edit state without a partial-apply
    /// terminal condition. It does *not* yet prove that a nonempty submission patch artifact was
    /// written to disk or that the exported patch text is well-formed.
    ///
    /// Keep this distinction explicit until the metric is tightened against recorded submission or
    /// diff artifacts.
    pub nonempty_valid_patch: bool,
    /// Prototype-1 eligibility gate for sending a candidate fix to oracle adjudication.
    ///
    /// Unlike `convergence`, this requires a concrete nonempty submission artifact. Keep the
    /// distinction explicit: workflow health can improve without yet producing an oracle-usable
    /// candidate patch.
    pub convergence: bool,
    pub oracle_eligible: bool,
}

impl OperationalRunMetrics {
    pub fn from_record(record: &RunRecord) -> Self {
        let tool_calls = record.tool_calls();
        let tool_calls_total = tool_calls.len();
        let tool_calls_failed = tool_calls
            .iter()
            .filter(|call| matches!(call.result, ToolResult::Failed(_)))
            .count();

        let patch_attempted = tool_calls
            .iter()
            .any(|call| is_edit_tool(call.request.tool.as_str()));
        let partial_patch_failures = tool_calls
            .iter()
            .filter(|call| is_partial_patch_failure(call))
            .count();

        let raw_patch_targets = flatten_raw_patch_targets(&tool_calls);
        let same_file_patch_retry_count = raw_patch_retry_count(&raw_patch_targets);
        let same_file_patch_max_streak = raw_patch_max_streak(&raw_patch_targets);

        let patch_apply_state =
            derive_patch_apply_state(record, &tool_calls, partial_patch_failures);
        let submission_artifact_state = derive_submission_artifact_state(record);
        let aborted = record
            .conversations()
            .last()
            .map(|turn| {
                matches!(
                    turn.outcome,
                    TurnOutcome::Error { .. } | TurnOutcome::Timeout { .. }
                )
            })
            .unwrap_or(false);

        // Conservative for prototype 1: treat a fully applied edit state as the best available
        // proxy for "nonempty valid patch". This intentionally under-specifies the stronger claim
        // we eventually want, which should be tied to recorded submission/diff artifacts.
        let nonempty_valid_patch = patch_apply_state == PatchApplyState::Applied;
        let aborted_repair_loop = aborted
            && same_file_patch_max_streak >= REPAIR_LOOP_STREAK_THRESHOLD
            && !nonempty_valid_patch;
        let convergence = !aborted
            && patch_apply_state == PatchApplyState::Applied
            && nonempty_valid_patch
            && !aborted_repair_loop;
        let oracle_eligible =
            convergence && submission_artifact_state == SubmissionArtifactState::Nonempty;

        Self {
            tool_calls_total,
            tool_calls_failed,
            patch_attempted,
            patch_apply_state,
            submission_artifact_state,
            partial_patch_failures,
            same_file_patch_retry_count,
            same_file_patch_max_streak,
            aborted,
            aborted_repair_loop,
            nonempty_valid_patch,
            convergence,
            oracle_eligible,
        }
    }
}

fn derive_submission_artifact_state(record: &RunRecord) -> SubmissionArtifactState {
    record
        .phases
        .packaging
        .as_ref()
        .map(|phase| phase.submission_artifact_state)
        .unwrap_or(SubmissionArtifactState::NotRecorded)
}

impl RunRecord {
    pub fn operational_metrics(&self) -> OperationalRunMetrics {
        OperationalRunMetrics::from_record(self)
    }
}

fn derive_patch_apply_state(
    record: &RunRecord,
    tool_calls: &[ToolExecutionRecord],
    partial_patch_failures: usize,
) -> PatchApplyState {
    if let Some(last_turn) = record.conversations().last() {
        if let Some(artifact) = &last_turn.agent_turn_artifact {
            if artifact.patch_artifact.applied {
                if artifact.patch_artifact.all_proposals_applied {
                    return PatchApplyState::Applied;
                }
                return PatchApplyState::Partial;
            }
        }
    }

    if partial_patch_failures > 0 {
        return PatchApplyState::Partial;
    }

    if tool_calls.iter().any(|call| applied_edit_count(call) > 0) {
        PatchApplyState::Applied
    } else {
        PatchApplyState::No
    }
}

fn is_edit_tool(tool_name: &str) -> bool {
    EDIT_TOOL_NAMES.contains(&tool_name)
}

fn is_partial_patch_failure(call: &ToolExecutionRecord) -> bool {
    if !is_edit_tool(call.request.tool.as_str()) {
        return false;
    }

    match &call.result {
        ToolResult::Failed(failed) => failed.error.contains(PARTIAL_PATCH_ERROR_TEXT),
        ToolResult::Completed(completed) => {
            ui_payload_field(completed.ui_payload.as_ref(), "partial")
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
        }
    }
}

fn applied_edit_count(call: &ToolExecutionRecord) -> usize {
    match &call.result {
        ToolResult::Completed(completed) => {
            ui_payload_field(completed.ui_payload.as_ref(), "applied")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0)
        }
        ToolResult::Failed(_) => 0,
    }
}

fn ui_payload_field<'a>(
    payload: Option<&'a ploke_tui::tools::ToolUiPayload>,
    name: &str,
) -> Option<&'a str> {
    payload?
        .fields
        .iter()
        .find(|field| field.name.as_ref() == name)
        .map(|field| field.value.as_ref())
}

fn flatten_raw_patch_targets(tool_calls: &[ToolExecutionRecord]) -> Vec<String> {
    tool_calls
        .iter()
        .filter(|call| call.request.tool == RAW_PATCH_TOOL_NAME)
        .flat_map(|call| raw_patch_target_files(&call.request.arguments))
        .collect()
}

fn raw_patch_target_files(arguments: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(arguments) else {
        return Vec::new();
    };
    value
        .get("patches")
        .and_then(|patches| patches.as_array())
        .into_iter()
        .flatten()
        .filter_map(|patch| patch.get("file").and_then(|file| file.as_str()))
        .map(ToString::to_string)
        .collect()
}

fn raw_patch_retry_count(targets: &[String]) -> usize {
    use std::collections::BTreeMap;

    let mut counts = BTreeMap::<&str, usize>::new();
    for target in targets {
        *counts.entry(target.as_str()).or_insert(0) += 1;
    }
    counts.values().map(|count| count.saturating_sub(1)).sum()
}

fn raw_patch_max_streak(targets: &[String]) -> usize {
    let mut max_streak = 0;
    let mut current_streak = 0;
    let mut last_target: Option<&str> = None;

    for target in targets {
        if last_target == Some(target.as_str()) {
            current_streak += 1;
        } else {
            current_streak = 1;
            last_target = Some(target.as_str());
        }
        max_streak = max_streak.max(current_streak);
    }

    max_streak
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ploke_core::ArcStr;
    use ploke_core::tool_types::ToolName;
    use ploke_tui::tools::ToolUiPayload;
    use uuid::Uuid;

    use super::{OperationalRunMetrics, PatchApplyState};
    use crate::record::{
        AgentMetadata, BenchmarkMetadata, PackagingPhase, RunMetadata, RunPhases, RunRecord,
        RuntimeMetadata, SubmissionArtifactState, ToolExecutionRecord, ToolResult, TurnOutcome,
        TurnRecord,
    };
    use crate::runner::{RunArm, ToolCompletedRecord, ToolFailedRecord};
    use crate::spec::{EvalBudget, IssueInput};

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
                runtime: RuntimeMetadata::default(),
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

    #[test]
    fn content_only_run_is_not_oracle_eligible() {
        let mut record = base_record();
        record
            .phases
            .agent_turns
            .push(turn_record(TurnOutcome::Content, Vec::new()));

        let metrics = OperationalRunMetrics::from_record(&record);

        assert_eq!(metrics.tool_calls_total, 0);
        assert!(!metrics.patch_attempted);
        assert_eq!(metrics.patch_apply_state, PatchApplyState::No);
        assert_eq!(
            metrics.submission_artifact_state,
            SubmissionArtifactState::NotRecorded
        );
        assert!(!metrics.nonempty_valid_patch);
        assert!(!metrics.convergence);
        assert!(!metrics.oracle_eligible);
    }

    #[test]
    fn applied_edit_run_with_nonempty_submission_is_oracle_eligible() {
        let mut record = base_record();
        record.phases.packaging = Some(PackagingPhase {
            started_at: "2026-04-23T00:00:02Z".to_string(),
            ended_at: "2026-04-23T00:00:03Z".to_string(),
            submission_artifact_state: SubmissionArtifactState::Nonempty,
            msb_submission_path: Some(PathBuf::from("/tmp/repo/multi-swe-bench-submission.jsonl")),
        });
        record.phases.agent_turns.push(turn_record(
            TurnOutcome::ToolCalls { count: 1 },
            vec![completed_edit_call(
                ToolName::ApplyCodeEdit,
                serde_json::json!({
                    "edits": [{
                        "file": "src/lib.rs",
                        "canon": "crate::lib::helper",
                        "node_type": "function",
                        "code": "pub fn helper() {}"
                    }]
                }),
                1,
                false,
            )],
        ));

        let metrics = OperationalRunMetrics::from_record(&record);

        assert!(metrics.patch_attempted);
        assert_eq!(metrics.patch_apply_state, PatchApplyState::Applied);
        assert_eq!(
            metrics.submission_artifact_state,
            SubmissionArtifactState::Nonempty
        );
        assert!(!metrics.aborted);
        assert!(metrics.nonempty_valid_patch);
        assert!(metrics.convergence);
        assert!(metrics.oracle_eligible);
    }

    #[test]
    fn repeated_same_file_raw_patch_loop_is_detected_as_aborted_repair_loop() {
        let mut record = base_record();
        record.phases.agent_turns.push(turn_record(
            TurnOutcome::Error {
                message: "Request summary: [aborted] error_id=1".to_string(),
            },
            vec![
                completed_edit_call(
                    ToolName::NsPatch,
                    serde_json::json!({
                        "patches": [{
                            "file": "src/bytes_mut.rs",
                            "diff": "--- a/src/bytes_mut.rs\n+++ b/src/bytes_mut.rs\n",
                            "reasoning": "first"
                        }]
                    }),
                    1,
                    false,
                ),
                completed_edit_call(
                    ToolName::NsPatch,
                    serde_json::json!({
                        "patches": [{
                            "file": "src/bytes_mut.rs",
                            "diff": "--- a/src/bytes_mut.rs\n+++ b/src/bytes_mut.rs\n",
                            "reasoning": "second"
                        }]
                    }),
                    0,
                    true,
                ),
                failed_edit_call(
                    ToolName::NsPatch,
                    serde_json::json!({
                        "patches": [{
                            "file": "src/bytes_mut.rs",
                            "diff": "--- a/src/bytes_mut.rs\n+++ b/src/bytes_mut.rs\n",
                            "reasoning": "third"
                        }]
                    }),
                    "failed to patch src/bytes_mut.rs: IO error: NsPatchError: Patch applied partially. See report for details.",
                ),
                failed_edit_call(
                    ToolName::NsPatch,
                    serde_json::json!({
                        "patches": [{
                            "file": "src/bytes_mut.rs",
                            "diff": "--- a/src/bytes_mut.rs\n+++ b/src/bytes_mut.rs\n",
                            "reasoning": "fourth"
                        }]
                    }),
                    "Request summary: [aborted] code=REPAIR_BUDGET_EXHAUSTED",
                ),
            ],
        ));

        let metrics = OperationalRunMetrics::from_record(&record);

        assert_eq!(metrics.tool_calls_total, 4);
        assert_eq!(metrics.partial_patch_failures, 2);
        assert_eq!(metrics.same_file_patch_retry_count, 3);
        assert_eq!(metrics.same_file_patch_max_streak, 4);
        assert_eq!(metrics.patch_apply_state, PatchApplyState::Partial);
        assert!(metrics.aborted);
        assert!(metrics.aborted_repair_loop);
        assert!(!metrics.nonempty_valid_patch);
        assert!(!metrics.convergence);
        assert!(!metrics.oracle_eligible);
    }

    #[test]
    fn empty_submission_artifact_state_is_tracked_separately_from_patch_proxy() {
        let mut record = base_record();
        record.phases.packaging = Some(PackagingPhase {
            started_at: "2026-04-23T00:00:02Z".to_string(),
            ended_at: "2026-04-23T00:00:03Z".to_string(),
            submission_artifact_state: SubmissionArtifactState::Empty,
            msb_submission_path: Some(PathBuf::from("/tmp/repo/multi-swe-bench-submission.jsonl")),
        });
        record.phases.agent_turns.push(turn_record(
            TurnOutcome::ToolCalls { count: 1 },
            vec![completed_edit_call(
                ToolName::ApplyCodeEdit,
                serde_json::json!({
                    "edits": [{
                        "file": "src/lib.rs",
                        "canon": "crate::lib::helper",
                        "node_type": "function",
                        "code": "pub fn helper() {}"
                    }]
                }),
                1,
                false,
            )],
        ));

        let metrics = OperationalRunMetrics::from_record(&record);

        assert_eq!(metrics.patch_apply_state, PatchApplyState::Applied);
        assert_eq!(
            metrics.submission_artifact_state,
            SubmissionArtifactState::Empty
        );
        assert!(metrics.nonempty_valid_patch);
        assert!(metrics.convergence);
        assert!(!metrics.oracle_eligible);
    }
}
