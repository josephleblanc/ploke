//! Self-edit replay probes over broad-harness headless TUI evidence.
//!
//! Broad self-edit attempts currently persist compact headless-TUI evidence
//! rather than the full `agent-turn-trace.json` plus `llm-full-responses.jsonl`
//! shape used by benchmark eval runs. This module bridges that older evidence
//! into the same useful replay surface without replaying tool results: it
//! rebuilds assistant provider responses from the historical tool-request
//! events, installs those responses into `session.rs`, and lets the current TUI
//! tools execute against the requested workspace.

use std::{
    fs,
    path::{Path, PathBuf},
};

use ploke_llm::manager::{RecordedResponse, RecordedResponseTape};
use ploke_records::{
    agent_turn::ToolRequestRecord, llm_response::RawFullResponseRecord,
    tool_contracts::ToolArgumentsJson,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    cli::prototype1_state::edit_surface::{
        harness_request::PublishedBroadHarnessRequest,
        tui_adapter::{self, ModelSelection},
    },
    replay::turn::ReplayTail,
    spec::PrepareError,
};

#[derive(Debug, Clone)]
pub(crate) struct SelfEditProbeRequest {
    pub(crate) request_path: PathBuf,
    pub(crate) result_path: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) event_index: usize,
    pub(crate) through_event: bool,
    pub(crate) tail: ReplayTail,
    pub(crate) budget: tui_adapter::Budget,
    pub(crate) model: Option<ModelSelection>,
}

impl SelfEditProbeRequest {
    pub(crate) async fn run(self) -> Result<SelfEditProbeRun, PrepareError> {
        run_self_edit_probe(self).await
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SelfEditProbeRun {
    pub(crate) request_path: PathBuf,
    pub(crate) result_path: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) selected_events: usize,
    pub(crate) selected_tool_requests: usize,
    pub(crate) installed_records: usize,
    pub(crate) terminal: String,
    pub(crate) historical_failures: Vec<SelfEditFailure>,
    pub(crate) observed: tui_adapter::evidence::Summary,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SelfEditFailure {
    pub(crate) event_index: usize,
    pub(crate) call_id: String,
    pub(crate) preview: String,
}

fn run_prepare_error(phase: &'static str, detail: impl Into<String>) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase,
        detail: detail.into(),
    }
}

async fn run_self_edit_probe(
    request: SelfEditProbeRequest,
) -> Result<SelfEditProbeRun, PrepareError> {
    let published = load_published_request(&request.request_path)?;
    let historical = load_historical_summary(&request.result_path)?;
    let selected_events = selected_events(&historical, request.event_index, request.through_event);
    let historical_requests = tool_requests_from_events(&selected_events)?;
    let installed_records = match request.tail {
        ReplayTail::Stop => historical_requests.len() + 1,
        ReplayTail::Live | ReplayTail::LiveStep => historical_requests.len(),
    };
    let tape = recorded_tool_request_tape(&historical_requests, request.tail)?;
    install_tape(tape, request.tail);

    let prompt = fs::read_to_string(published.prompt_path()).map_err(|source| {
        PrepareError::ReadManifest {
            path: published.prompt_path().to_path_buf(),
            source,
        }
    })?;
    let run = tui_adapter::run_headless_with_model(
        &request.workspace,
        &prompt,
        request.budget,
        published.request().edit_policy,
        &published.request().evidence_roots,
        request.model,
    )
    .await
    .map_err(|source| run_prepare_error("self_edit_replay_headless", source.to_string()))?;

    let terminal = run
        .terminal()
        .map(terminal_label)
        .unwrap_or_else(|| "unknown".to_string());
    let observed = run.evidence();

    Ok(SelfEditProbeRun {
        request_path: request.request_path,
        result_path: request.result_path,
        workspace: request.workspace,
        selected_events: selected_events.len(),
        selected_tool_requests: historical_requests.len(),
        installed_records,
        terminal,
        historical_failures: historical_failures(&historical, request.event_index),
        observed,
    })
}

fn load_published_request(path: &Path) -> Result<PublishedBroadHarnessRequest, PrepareError> {
    let json = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str::<PublishedBroadHarnessRequest>(&json).map_err(|source| {
        run_prepare_error(
            "self_edit_replay_request",
            format!(
                "parse published broad-harness request {}: {source}",
                path.display()
            ),
        )
    })
}

fn load_historical_summary(path: &Path) -> Result<tui_adapter::evidence::Summary, PrepareError> {
    let json = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str::<tui_adapter::evidence::Summary>(&json).map_err(|source| {
        run_prepare_error(
            "self_edit_replay_result",
            format!("parse headless TUI result {}: {source}", path.display()),
        )
    })
}

fn selected_events(
    summary: &tui_adapter::evidence::Summary,
    event_index: usize,
    through_event: bool,
) -> Vec<tui_adapter::evidence::Event> {
    let end = if through_event {
        event_index.saturating_add(1)
    } else {
        summary.events.len()
    };
    summary.events.iter().take(end).cloned().collect()
}

fn tool_requests_from_events(
    events: &[tui_adapter::evidence::Event],
) -> Result<Vec<ToolRequestRecord>, PrepareError> {
    let mut requests = Vec::new();
    for event in events {
        let tui_adapter::evidence::Event::ToolRequest {
            request_id,
            parent_id,
            call_id,
            tool,
            arguments,
        } = event
        else {
            continue;
        };
        if arguments.chars != arguments.preview.chars().count() {
            return Err(run_prepare_error(
                "self_edit_replay_tool_args",
                format!(
                    "historical arguments for tool {tool} call {call_id} are truncated; cannot replay as provider output"
                ),
            ));
        }
        requests.push(ToolRequestRecord {
            request_id: request_id.clone(),
            parent_id: parent_id.clone(),
            call_id: call_id.clone(),
            tool: tool.clone(),
            arguments: ToolArgumentsJson::from(arguments.preview.clone()),
        });
    }
    Ok(requests)
}

fn recorded_tool_request_tape(
    requests: &[ToolRequestRecord],
    tail: ReplayTail,
) -> Result<RecordedResponseTape, PrepareError> {
    let assistant_id = Uuid::new_v4();
    let mut records = Vec::new();
    for (index, request) in requests.iter().enumerate() {
        records.push(tool_response_record(assistant_id, index, request)?);
    }
    if tail == ReplayTail::Stop {
        records.push(stop_response_record(assistant_id, requests.len())?);
    }
    Ok(RecordedResponseTape::new(
        records
            .into_iter()
            .map(RawFullResponseRecord::into_recorded_response)
            .collect(),
    ))
}

fn install_tape(tape: RecordedResponseTape, tail: ReplayTail) {
    match tail {
        ReplayTail::Stop => ploke_tui::llm::install_recorded_response_tape(tape),
        ReplayTail::Live => ploke_tui::llm::install_recorded_response_prefix_then_live(tape),
        ReplayTail::LiveStep => {
            ploke_tui::llm::install_recorded_response_prefix_then_live_steps(tape, 1)
        }
    }
}

fn tool_response_record(
    assistant_id: Uuid,
    response_index: usize,
    request: &ToolRequestRecord,
) -> Result<RawFullResponseRecord, PrepareError> {
    let response = serde_json::from_value(serde_json::json!({
        "id": format!("self-edit-replay-tool-{response_index}"),
        "choices": [{
            "index": 0,
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "tool_calls": [{
                    "id": request.call_id,
                    "type": "function",
                    "function": {
                        "name": request.tool,
                        "arguments": request.arguments.as_str(),
                    }
                }]
            }
        }],
        "created": response_index,
        "model": "self-edit/replay",
        "object": "chat.completion"
    }))
    .map_err(|source| {
        run_prepare_error(
            "self_edit_replay_response",
            format!("build recorded response {response_index}: {source}"),
        )
    })?;
    Ok(RawFullResponseRecord {
        assistant_message_id: assistant_id,
        recorded_response: RecordedResponse::new(response_index, response),
    })
}

fn stop_response_record(
    assistant_id: Uuid,
    response_index: usize,
) -> Result<RawFullResponseRecord, PrepareError> {
    let response = serde_json::from_value(serde_json::json!({
        "id": format!("self-edit-replay-stop-{response_index}"),
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {
                "role": "assistant",
                "content": "done"
            }
        }],
        "created": response_index,
        "model": "self-edit/replay",
        "object": "chat.completion"
    }))
    .map_err(|source| {
        run_prepare_error(
            "self_edit_replay_response",
            format!("build recorded stop response {response_index}: {source}"),
        )
    })?;
    Ok(RawFullResponseRecord {
        assistant_message_id: assistant_id,
        recorded_response: RecordedResponse::new(response_index, response),
    })
}

fn historical_failures(
    summary: &tui_adapter::evidence::Summary,
    event_index: usize,
) -> Vec<SelfEditFailure> {
    summary
        .events
        .iter()
        .take(event_index.saturating_add(1))
        .enumerate()
        .filter_map(|(event_index, event)| {
            let tui_adapter::evidence::Event::ToolFailed { call_id, error } = event else {
                return None;
            };
            Some(SelfEditFailure {
                event_index,
                call_id: call_id.clone(),
                preview: error.preview.clone(),
            })
        })
        .collect()
}

fn terminal_label(terminal: &tui_adapter::HeadlessTerminal) -> String {
    match terminal {
        tui_adapter::HeadlessTerminal::Applied { changed_paths, .. } => {
            format!("applied changed_paths={}", changed_paths.len())
        }
        tui_adapter::HeadlessTerminal::Exhausted { attempts, .. } => {
            format!("exhausted attempts={attempts}")
        }
        tui_adapter::HeadlessTerminal::CompletedWithoutEdit { outcome, .. } => {
            format!("completed_without_edit outcome={outcome}")
        }
        tui_adapter::HeadlessTerminal::ToolFailed { .. } => "tool_failed".to_string(),
        tui_adapter::HeadlessTerminal::NoEdit => "no_edit".to_string(),
        tui_adapter::HeadlessTerminal::ContextUnavailable { reason } => {
            format!("context_unavailable reason={}", truncate_chars(reason, 160))
        }
        tui_adapter::HeadlessTerminal::ProviderUnavailable { reason } => {
            format!(
                "provider_unavailable reason={}",
                truncate_chars(reason, 160)
            )
        }
        tui_adapter::HeadlessTerminal::TimedOut { secs } => format!("timed_out secs={secs}"),
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let mut truncated = text.chars().take(keep).collect::<String>();
    truncated.push_str("...");
    truncated
}
