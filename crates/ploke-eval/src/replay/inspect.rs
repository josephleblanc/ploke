//! Read-only replay inspection over typed agent-turn artifacts.
//!
//! This module is the library body behind `ploke-eval run replay inspect`.
//! It deliberately stops before TUI execution: the job here is to show which
//! event cursors are replayable, whether their provider-response sidecars are
//! contiguous enough for admission, and whether the recorded prompt appears to
//! point at stale workspace paths.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use ploke_records::llm_response::FULL_RESPONSE_TRACE_FILE;
use ploke_tree::{FsRunStore, TurnCursor, TurnEventKind, turn_event_steps_from_agent_turn_records};
use serde::Serialize;

use crate::{replay::llm::LoadedResponseTape, spec::PrepareError};

#[derive(Debug, Clone)]
pub(crate) struct ReplayInspectRequest {
    pub(crate) run_dir: PathBuf,
    pub(crate) workspace: Option<PathBuf>,
}

impl ReplayInspectRequest {
    pub(crate) fn inspect(self) -> Result<ReplayInspection, PrepareError> {
        inspect_replay(self)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ReplayInspection {
    pub(crate) run_dir: PathBuf,
    pub(crate) workspace: Option<PathBuf>,
    pub(crate) workspace_exists: Option<bool>,
    pub(crate) steps: Vec<ReplayStep>,
}

impl ReplayInspection {
    pub(crate) fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub(crate) fn signal_count(&self) -> usize {
        let mut distinct = BTreeSet::new();
        for signal in self.steps.iter().flat_map(|step| &step.quality_signals) {
            distinct.insert((format!("{:?}", signal.kind), signal.detail.as_str()));
        }
        distinct.len()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ReplayStep {
    pub(crate) cursor: TurnCursor,
    pub(crate) task_id: String,
    pub(crate) selected_model: String,
    pub(crate) event_kind: TurnEventKind,
    pub(crate) tool_name: Option<String>,
    pub(crate) call_id: Option<String>,
    pub(crate) assistant_message_id: Option<String>,
    pub(crate) tape: Option<TapeContinuity>,
    pub(crate) prompt_paths: Vec<PromptPathEvidence>,
    pub(crate) quality_signals: Vec<ReplayQualitySignal>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum TapeContinuity {
    Present {
        path: PathBuf,
        response_indices: Vec<usize>,
        missing_response_indices: Vec<usize>,
    },
    Unavailable {
        path: PathBuf,
        reason: String,
    },
}

impl TapeContinuity {
    fn inspect(run_dir: &Path, assistant_message_id: &str) -> Self {
        let path = run_dir.join(FULL_RESPONSE_TRACE_FILE);
        match LoadedResponseTape::load_for_inspection(run_dir, assistant_message_id) {
            Ok(tape) => {
                let response_indices = tape
                    .records()
                    .iter()
                    .map(|record| record.response_index().get())
                    .collect();
                let missing_response_indices = tape
                    .missing_response_indices()
                    .into_iter()
                    .map(|index| index.get())
                    .collect();
                Self::Present {
                    path: tape.path().to_path_buf(),
                    response_indices,
                    missing_response_indices,
                }
            }
            Err(error) => Self::Unavailable {
                path,
                reason: error.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct PromptPathEvidence {
    pub(crate) path: PathBuf,
    pub(crate) exists: bool,
    pub(crate) target_workspace: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ReplayQualitySignal {
    pub(crate) kind: ReplayQualitySignalKind,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReplayQualitySignalKind {
    TapeUnavailable,
    TapeGap,
    PromptPathUnreadable,
    PromptTargetMismatch,
}

fn inspect_replay(request: ReplayInspectRequest) -> Result<ReplayInspection, PrepareError> {
    if !request.run_dir.is_dir() {
        return Err(PrepareError::DatabaseSetup {
            phase: "replay_inspect_run_dir",
            detail: format!(
                "run directory '{}' does not exist",
                request.run_dir.display()
            ),
        });
    }

    let records = FsRunStore::new(&request.run_dir)
        .load_agent_turn_records()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "replay_inspect_records",
            detail: format!(
                "failed to load agent-turn records from '{}': {source}",
                request.run_dir.display()
            ),
        })?;
    let workspace_exists = request.workspace.as_ref().map(|path| path.is_dir());
    let workspace = request.workspace.as_deref();
    let mut tape_cache = BTreeMap::new();

    let mut steps = Vec::new();
    for step in turn_event_steps_from_agent_turn_records(&records) {
        let assistant_message_id = step
            .response_tape_ref()
            .map(|reference| reference.assistant_message_id.to_owned());
        let tape = assistant_message_id.as_ref().map(|assistant| {
            tape_cache
                .entry(assistant.clone())
                .or_insert_with(|| TapeContinuity::inspect(&request.run_dir, assistant))
                .clone()
        });
        let prompt_paths = prompt_path_evidence(step.issue_prompt, workspace);
        let quality_signals = quality_signals(&tape, &prompt_paths, workspace);

        steps.push(ReplayStep {
            cursor: step.cursor(),
            task_id: step.task_id.to_owned(),
            selected_model: step.selected_model.to_owned(),
            event_kind: step.kind(),
            tool_name: step.tool_name().map(ToOwned::to_owned),
            call_id: step.call_id().map(ToOwned::to_owned),
            assistant_message_id,
            tape,
            prompt_paths,
            quality_signals,
        });
    }

    Ok(ReplayInspection {
        run_dir: request.run_dir,
        workspace: request.workspace,
        workspace_exists,
        steps,
    })
}

fn quality_signals(
    tape: &Option<TapeContinuity>,
    prompt_paths: &[PromptPathEvidence],
    workspace: Option<&Path>,
) -> Vec<ReplayQualitySignal> {
    let mut signals = Vec::new();
    match tape {
        Some(TapeContinuity::Unavailable { reason, .. }) => signals.push(ReplayQualitySignal {
            kind: ReplayQualitySignalKind::TapeUnavailable,
            detail: reason.clone(),
        }),
        Some(TapeContinuity::Present {
            missing_response_indices,
            ..
        }) if !missing_response_indices.is_empty() => signals.push(ReplayQualitySignal {
            kind: ReplayQualitySignalKind::TapeGap,
            detail: format!("missing response_index values: {missing_response_indices:?}"),
        }),
        _ => {}
    }

    for path in prompt_paths.iter().filter(|path| !path.exists) {
        signals.push(ReplayQualitySignal {
            kind: ReplayQualitySignalKind::PromptPathUnreadable,
            detail: format!("prompt path '{}' is not readable", path.path.display()),
        });
    }

    if workspace.is_some()
        && !prompt_paths.is_empty()
        && !prompt_paths.iter().any(|path| path.target_workspace)
    {
        signals.push(ReplayQualitySignal {
            kind: ReplayQualitySignalKind::PromptTargetMismatch,
            detail: "prompt paths do not mention the selected target workspace".to_owned(),
        });
    }

    signals
}

fn prompt_path_evidence(text: &str, workspace: Option<&Path>) -> Vec<PromptPathEvidence> {
    extract_absolute_prompt_paths(text)
        .into_iter()
        .map(|path| {
            let target_workspace = workspace
                .map(|workspace| path == workspace || path.starts_with(workspace))
                .unwrap_or(false);
            PromptPathEvidence {
                exists: path.exists(),
                target_workspace,
                path,
            }
        })
        .collect()
}

fn extract_absolute_prompt_paths(text: &str) -> Vec<PathBuf> {
    let mut paths = BTreeSet::new();
    for raw in text.split_whitespace() {
        let token = trim_path_token(raw);
        if token.starts_with('/') && token.len() > 1 {
            paths.insert(PathBuf::from(token));
        }
    }
    paths.into_iter().collect()
}

fn trim_path_token(raw: &str) -> &str {
    raw.trim_matches(|c| {
        matches!(
            c,
            '"' | '\'' | '`' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
        )
    })
    .trim_end_matches('.')
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ploke_records::llm_response::FULL_RESPONSE_TRACE_FILE;
    use ploke_tree::TurnArtifactKind;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn replay_inspection_lists_steps_and_gapped_tape_without_scheduler() {
        let root = tempfile::tempdir().expect("tempdir");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa).to_string();
        let stale_path = root.path().join("old-workspace").join("src/lib.rs");
        let workspace = root.path().join("target-workspace");
        fs::create_dir(&workspace).expect("create workspace");
        fs::write(
            root.path().join("agent-turn-trace.json"),
            agent_turn_json(&assistant, &stale_path).to_string(),
        )
        .expect("write trace");
        fs::write(
            root.path().join(FULL_RESPONSE_TRACE_FILE),
            format!(
                "{}\n{}\n",
                response_line(&assistant, 0),
                response_line(&assistant, 2)
            ),
        )
        .expect("write response trace");

        let inspection = ReplayInspectRequest {
            run_dir: root.path().to_path_buf(),
            workspace: Some(workspace),
        }
        .inspect()
        .expect("inspect replay");

        assert_eq!(inspection.workspace_exists, Some(true));
        assert_eq!(inspection.step_count(), 2);
        let step = &inspection.steps[0];
        assert_eq!(step.cursor.artifact_kind, TurnArtifactKind::Trace);
        assert_eq!(step.cursor.event_index, 0);
        assert_eq!(step.event_kind, TurnEventKind::ToolRequested);
        assert_eq!(step.tool_name.as_deref(), Some("read_file"));
        assert_eq!(
            step.assistant_message_id.as_deref(),
            Some(assistant.as_str())
        );
        assert_eq!(step.prompt_paths.len(), 1);
        assert_eq!(step.prompt_paths[0].path, stale_path);
        assert!(!step.prompt_paths[0].exists);
        assert!(!step.prompt_paths[0].target_workspace);

        match step.tape.as_ref().expect("tape") {
            TapeContinuity::Present {
                response_indices,
                missing_response_indices,
                ..
            } => {
                assert_eq!(response_indices, &[0, 2]);
                assert_eq!(missing_response_indices, &[1]);
            }
            TapeContinuity::Unavailable { reason, .. } => {
                panic!("expected present tape, got unavailable: {reason}")
            }
        }
        assert!(
            step.quality_signals
                .iter()
                .any(|signal| { signal.kind == ReplayQualitySignalKind::TapeGap })
        );
        assert!(
            step.quality_signals
                .iter()
                .any(|signal| { signal.kind == ReplayQualitySignalKind::PromptPathUnreadable })
        );
        assert!(
            step.quality_signals
                .iter()
                .any(|signal| { signal.kind == ReplayQualitySignalKind::PromptTargetMismatch })
        );
    }

    fn agent_turn_json(assistant_message_id: &str, stale_path: &Path) -> serde_json::Value {
        serde_json::json!({
            "task_id": "trace-task",
            "selected_model": "openai/gpt-5",
            "issue_prompt": format!("Fix the bug in {}.", stale_path.display()),
            "user_message_id": "user-1",
            "events": [
                {"ToolRequested": {
                    "request_id": "req-1",
                    "parent_id": "parent-1",
                    "call_id": "call-1",
                    "tool": "read_file",
                    "arguments": "{\"file\":\"src/lib.rs\"}"
                }},
                {"TurnFinished": {
                    "session_id": "session-1",
                    "request_id": "req-1",
                    "parent_id": "parent-1",
                    "assistant_message_id": assistant_message_id,
                    "outcome": "completed",
                    "error_id": null,
                    "summary": "done",
                    "attempts": 1
                }}
            ],
            "prompt_debug": null,
            "terminal_record": {
                "session_id": "session-1",
                "request_id": "req-1",
                "parent_id": "parent-1",
                "assistant_message_id": assistant_message_id,
                "outcome": "completed",
                "error_id": null,
                "summary": "done",
                "attempts": 1
            },
            "final_assistant_message": {
                "id": assistant_message_id,
                "kind": "Assistant",
                "status": "Completed",
                "tool_call_id": null,
                "content_len": 4,
                "content_preview": "done"
            },
            "patch_artifact": {
                "edit_proposals": [],
                "create_proposals": [],
                "applied": false,
                "all_proposals_applied": false,
                "expected_file_changes": [],
                "any_expected_file_changed": false,
                "all_expected_files_changed": false
            },
            "llm_prompt": [{
                "role": "user",
                "content": "Fix the bug."
            }],
            "llm_response": "done"
        })
    }

    fn response_line(assistant_message_id: &str, response_index: usize) -> String {
        serde_json::json!({
            "assistant_message_id": assistant_message_id,
            "response_index": response_index,
            "response": {
                "id": format!("response-{response_index}"),
                "choices": [{
                    "index": 0,
                    "finish_reason": "stop",
                    "message": {
                        "role": "assistant",
                        "content": "done"
                    }
                }],
                "created": 0,
                "model": "test/model",
                "object": "chat.completion"
            }
        })
        .to_string()
    }
}
