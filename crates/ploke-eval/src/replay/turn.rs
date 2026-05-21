//! Resolve agent-turn playback cursors into replay anchors.
//!
//! `ploke-tree` owns the typed playback coordinate: artifact kind, artifact
//! path, and event index inside `agent-turn-trace.json` or
//! `agent-turn-summary.json`. This module is the eval-side adapter that turns
//! that coordinate into the minimum replay anchor needed by a tool-loop probe:
//! the recorded prompt, selected model label, event metadata, and the
//! `assistant_message_id` used to load `llm-full-responses.jsonl`.
//!
//! Keep this module as a resolver over typed records. It should not execute
//! TUI events, synthesize tool results, or parse persisted JSON through
//! `serde_json::Value`; replay execution starts only after a response tape has
//! been installed into the normal `ploke-tui` session path.

use std::path::Path;

use ploke_llm::manager::ResponseIndex;
use ploke_tree::{
    AgentTurnRecordSet, FsRunStore, TurnArtifactKind, TurnCursor, TurnEventKind,
    turn_event_step_at, turn_event_steps_from_artifact,
};
use serde::{Deserialize, Serialize};

use crate::{replay::llm::LoadedResponseTape, spec::PrepareError};

/// Owned replay anchor for one event in a persisted agent-turn artifact.
///
/// The anchor is intentionally owned because it crosses from a borrowed graph
/// projection into async TUI execution. It is still not a replacement record
/// type: it is a short-lived run parameter derived from
/// `ploke-tree::TurnEventStepRef` and the response-tape sidecar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurnAnchor {
    pub cursor: TurnCursor,
    pub task_id: String,
    pub selected_model: String,
    pub issue_prompt: String,
    pub event_kind: TurnEventKind,
    pub tool_name: Option<String>,
    pub call_id: Option<String>,
    pub assistant_message_id: String,
}

impl TurnAnchor {
    pub fn assistant_message_id(&self) -> &str {
        self.assistant_message_id.as_str()
    }
}

/// Which provider-response prefix should be installed for a replay probe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplayPrefixSelector {
    /// Install every recorded response for the resolved assistant message.
    FullTape,
    /// Install responses from index 0 through the selected provider response.
    ThroughResponseIndex { response_index: usize },
    /// Install enough provider responses to replay through this agent-turn event.
    ThroughEvent { cursor: TurnCursor },
}

/// What the session should do after the selected recorded prefix is exhausted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayTail {
    /// Use a recorded-only tape. Exhaustion stops before any live provider call.
    Stop,
    /// Switch from recorded provider responses to the normal live provider path.
    Live,
}

/// Resolved replay prefix installed into the TUI session path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedReplayPrefix {
    pub anchor: TurnAnchor,
    pub selector: ReplayPrefixSelector,
    pub tail: ReplayTail,
    pub total_records: usize,
    pub installed_records: usize,
    pub through_response_index: Option<usize>,
}

pub fn resolve_turn_anchor(
    run_dir: &Path,
    cursor: &TurnCursor,
) -> Result<TurnAnchor, PrepareError> {
    let records = load_agent_turn_records(run_dir, "resolve_turn_anchor")?;
    let step = turn_event_step_at(&records, cursor).ok_or_else(|| PrepareError::DatabaseSetup {
        phase: "resolve_turn_anchor",
        detail: format!(
            "agent-turn cursor {:?} did not resolve in '{}'",
            cursor,
            run_dir.display()
        ),
    })?;
    let response = step
        .response_tape_ref()
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "resolve_turn_anchor",
            detail: format!(
                "agent-turn cursor {:?} resolved without an assistant_message_id",
                cursor
            ),
        })?;

    Ok(TurnAnchor {
        cursor: step.cursor(),
        task_id: step.task_id.to_owned(),
        selected_model: step.selected_model.to_owned(),
        issue_prompt: step.issue_prompt.to_owned(),
        event_kind: step.kind(),
        tool_name: step.tool_name().map(ToOwned::to_owned),
        call_id: step.call_id().map(ToOwned::to_owned),
        assistant_message_id: response.assistant_message_id.to_owned(),
    })
}

pub fn resolve_replay_prefix_at(
    run_dir: &Path,
    cursor: &TurnCursor,
    selector: ReplayPrefixSelector,
    tail: ReplayTail,
) -> Result<(ResolvedReplayPrefix, LoadedResponseTape), PrepareError> {
    // Install by resolved assistant message rather than by event shape. A
    // historical failure may begin at a tool request, a repair response, or a
    // later turn event, but the replayable provider prefix is keyed by the
    // assistant message sidecar written by the original session.
    let anchor_cursor = match &selector {
        ReplayPrefixSelector::ThroughEvent { cursor } => cursor,
        _ => cursor,
    };
    let anchor = resolve_turn_anchor(run_dir, anchor_cursor)?;
    let full = LoadedResponseTape::load(run_dir, anchor.assistant_message_id())?;
    let total_records = full.record_count();
    let through_response_index = match &selector {
        ReplayPrefixSelector::FullTape => full.last_response_index(),
        ReplayPrefixSelector::ThroughResponseIndex { response_index } => {
            Some(ResponseIndex::new(*response_index))
        }
        ReplayPrefixSelector::ThroughEvent { cursor } => {
            response_index_through_event(run_dir, cursor, &full)?
        }
    };
    let installed = match through_response_index {
        Some(response_index) => full.prefix_through_response_index(response_index)?,
        None => full.empty_prefix(),
    };
    let installed_records = installed.record_count();

    Ok((
        ResolvedReplayPrefix {
            anchor,
            selector,
            tail,
            total_records,
            installed_records,
            through_response_index: through_response_index.map(ResponseIndex::get),
        },
        installed,
    ))
}

pub fn install_replay_prefix_at(
    run_dir: &Path,
    cursor: &TurnCursor,
    selector: ReplayPrefixSelector,
    tail: ReplayTail,
) -> Result<(ResolvedReplayPrefix, LoadedResponseTape), PrepareError> {
    let (prefix, loaded) = resolve_replay_prefix_at(run_dir, cursor, selector, tail)?;
    match tail {
        ReplayTail::Stop => {
            ploke_tui::llm::install_recorded_response_tape(
                loaded.clone().into_recorded_response_tape(),
            );
        }
        ReplayTail::Live => {
            ploke_tui::llm::install_recorded_response_prefix_then_live(
                loaded.clone().into_recorded_response_tape(),
            );
        }
    }
    Ok((prefix, loaded))
}

pub fn install_prefix_then_live_at(
    run_dir: &Path,
    cursor: &TurnCursor,
) -> Result<(TurnAnchor, LoadedResponseTape), PrepareError> {
    let (prefix, loaded) = install_replay_prefix_at(
        run_dir,
        cursor,
        ReplayPrefixSelector::FullTape,
        ReplayTail::Live,
    )?;
    Ok((prefix.anchor, loaded))
}

fn response_index_through_event(
    run_dir: &Path,
    cursor: &TurnCursor,
    tape: &LoadedResponseTape,
) -> Result<Option<ResponseIndex>, PrepareError> {
    let records = load_agent_turn_records(run_dir, "resolve_replay_event_prefix")?;
    let (artifact_path, record) = match cursor.artifact_kind {
        TurnArtifactKind::Trace => records.traces.get_key_value(cursor.artifact_path.as_str()),
        TurnArtifactKind::Summary => records
            .summaries
            .get_key_value(cursor.artifact_path.as_str()),
    }
    .ok_or_else(|| PrepareError::DatabaseSetup {
        phase: "resolve_replay_event_prefix",
        detail: format!(
            "agent-turn artifact {:?}:{} did not resolve in '{}'",
            cursor.artifact_kind,
            cursor.artifact_path,
            run_dir.display()
        ),
    })?;
    let steps = turn_event_steps_from_artifact(cursor.artifact_kind, artifact_path, record);
    let target = steps
        .iter()
        .find(|step| step.event_index == cursor.event_index)
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "resolve_replay_event_prefix",
            detail: format!(
                "agent-turn cursor {:?} did not resolve in '{}'",
                cursor,
                run_dir.display()
            ),
        })?;

    if target.kind() == TurnEventKind::TurnFinished {
        return Ok(tape.last_response_index());
    }

    let target_call_id = target.call_id().map(ToOwned::to_owned);
    let mut selected = None;
    let mut target_call_seen = target_call_id.is_none();
    for step in steps
        .iter()
        .filter(|step| step.event_index <= cursor.event_index)
    {
        if step.kind() == TurnEventKind::TurnFinished {
            selected = tape.last_response_index();
            continue;
        }

        let Some(call_id) = step.call_id() else {
            continue;
        };
        let response_index = tape.response_index_for_tool_call(call_id)?;
        if let Some(response_index) = response_index {
            selected = max_response_index(selected, response_index);
            if target_call_id.as_deref() == Some(call_id) {
                target_call_seen = true;
            }
        }
    }

    if !target_call_seen {
        return Err(PrepareError::DatabaseSetup {
            phase: "resolve_replay_event_prefix",
            detail: format!(
                "agent-turn event {:?} references tool call '{}' but no recorded provider response contains that call id",
                cursor,
                target_call_id.unwrap_or_default()
            ),
        });
    }

    Ok(selected)
}

fn load_agent_turn_records(
    run_dir: &Path,
    phase: &'static str,
) -> Result<AgentTurnRecordSet, PrepareError> {
    FsRunStore::new(run_dir)
        .load_agent_turn_records()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase,
            detail: format!(
                "failed to load agent-turn records from '{}': {source}",
                run_dir.display()
            ),
        })
}

fn max_response_index(
    current: Option<ResponseIndex>,
    candidate: ResponseIndex,
) -> Option<ResponseIndex> {
    Some(match current {
        Some(current) => current.max(candidate),
        None => candidate,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ploke_records::llm_response::FULL_RESPONSE_TRACE_FILE;
    use ploke_tree::TurnArtifactKind;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn resolve_turn_anchor_loads_agent_turn_response_tape_identity() {
        let root = tempfile::tempdir().expect("tempdir");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa).to_string();
        fs::write(
            root.path().join("agent-turn-trace.json"),
            agent_turn_json(&assistant).to_string(),
        )
        .expect("write trace");
        fs::write(
            root.path().join(FULL_RESPONSE_TRACE_FILE),
            format!("{}\n", response_line(&assistant, 0)),
        )
        .expect("write response trace");

        let cursor = TurnCursor {
            artifact_kind: TurnArtifactKind::Trace,
            artifact_path: "agent-turn-trace.json".to_owned(),
            event_index: 0,
        };

        let anchor = resolve_turn_anchor(root.path(), &cursor).expect("resolve anchor");

        assert_eq!(anchor.cursor, cursor);
        assert_eq!(anchor.task_id, "trace-task");
        assert_eq!(anchor.selected_model, "openai/gpt-5");
        assert_eq!(anchor.issue_prompt, "Fix the bug.");
        assert_eq!(anchor.event_kind, TurnEventKind::ToolRequested);
        assert_eq!(anchor.tool_name.as_deref(), Some("read_file"));
        assert_eq!(anchor.call_id.as_deref(), Some("call-1"));
        assert_eq!(anchor.assistant_message_id(), assistant);
    }

    #[test]
    fn resolve_replay_prefix_through_response_index_slices_tape() {
        let root = tempfile::tempdir().expect("tempdir");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa).to_string();
        fs::write(
            root.path().join("agent-turn-trace.json"),
            agent_turn_json(&assistant).to_string(),
        )
        .expect("write trace");
        fs::write(
            root.path().join(FULL_RESPONSE_TRACE_FILE),
            format!(
                "{}\n{}\n",
                response_line(&assistant, 0),
                response_line(&assistant, 1)
            ),
        )
        .expect("write response trace");

        let cursor = TurnCursor {
            artifact_kind: TurnArtifactKind::Trace,
            artifact_path: "agent-turn-trace.json".to_owned(),
            event_index: 0,
        };

        let (prefix, loaded) = resolve_replay_prefix_at(
            root.path(),
            &cursor,
            ReplayPrefixSelector::ThroughResponseIndex { response_index: 0 },
            ReplayTail::Stop,
        )
        .expect("resolve replay prefix");

        assert_eq!(prefix.total_records, 2);
        assert_eq!(prefix.installed_records, 1);
        assert_eq!(prefix.through_response_index, Some(0));
        assert_eq!(loaded.records().len(), 1);
    }

    #[test]
    fn resolve_replay_prefix_through_event_maps_tool_call_to_response() {
        let root = tempfile::tempdir().expect("tempdir");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa).to_string();
        fs::write(
            root.path().join("agent-turn-trace.json"),
            agent_turn_json(&assistant).to_string(),
        )
        .expect("write trace");
        fs::write(
            root.path().join(FULL_RESPONSE_TRACE_FILE),
            format!("{}\n", tool_response_line(&assistant)),
        )
        .expect("write response trace");

        let cursor = TurnCursor {
            artifact_kind: TurnArtifactKind::Trace,
            artifact_path: "agent-turn-trace.json".to_owned(),
            event_index: 0,
        };

        let (prefix, loaded) = resolve_replay_prefix_at(
            root.path(),
            &cursor,
            ReplayPrefixSelector::ThroughEvent {
                cursor: cursor.clone(),
            },
            ReplayTail::Live,
        )
        .expect("resolve replay prefix");

        assert_eq!(prefix.total_records, 1);
        assert_eq!(prefix.installed_records, 1);
        assert_eq!(prefix.through_response_index, Some(0));
        assert_eq!(loaded.records().len(), 1);
    }

    #[test]
    fn install_prefix_then_live_at_uses_resolved_assistant_message() {
        let _guard = RecordedTapeGuard;
        ploke_tui::llm::clear_recorded_response_tape();
        let root = tempfile::tempdir().expect("tempdir");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa).to_string();
        fs::write(
            root.path().join("agent-turn-trace.json"),
            agent_turn_json(&assistant).to_string(),
        )
        .expect("write trace");
        fs::write(
            root.path().join(FULL_RESPONSE_TRACE_FILE),
            format!("{}\n", response_line(&assistant, 0)),
        )
        .expect("write response trace");

        let cursor = TurnCursor {
            artifact_kind: TurnArtifactKind::Trace,
            artifact_path: "agent-turn-trace.json".to_owned(),
            event_index: 0,
        };

        let (anchor, loaded) =
            install_prefix_then_live_at(root.path(), &cursor).expect("install tape");

        assert_eq!(anchor.assistant_message_id(), assistant);
        assert_eq!(loaded.assistant_message_id().to_string(), assistant);
        assert_eq!(loaded.records().len(), 1);
    }

    struct RecordedTapeGuard;

    impl Drop for RecordedTapeGuard {
        fn drop(&mut self) {
            ploke_tui::llm::clear_recorded_response_tape();
        }
    }

    fn agent_turn_json(assistant_message_id: &str) -> serde_json::Value {
        serde_json::json!({
            "task_id": "trace-task",
            "selected_model": "openai/gpt-5",
            "issue_prompt": "Fix the bug.",
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

    fn tool_response_line(assistant_message_id: &str) -> String {
        serde_json::json!({
            "assistant_message_id": assistant_message_id,
            "response_index": 0,
            "response": {
                "id": "response-0",
                "choices": [{
                    "index": 0,
                    "finish_reason": "tool_calls",
                    "message": {
                        "role": "assistant",
                        "tool_calls": [{
                            "id": "call-1",
                            "type": "function",
                            "function": {
                                "name": "read_file",
                                "arguments": "{\"file\":\"src/lib.rs\"}"
                            }
                        }]
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
