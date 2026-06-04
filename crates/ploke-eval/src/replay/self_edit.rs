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
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use ploke_llm::manager::{RecordedResponse, RecordedResponseTape};
use ploke_llm::manager::{ResponseIndex, parse_chat_outcome};
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
    pub(crate) source: Source,
    pub(crate) workspace: PathBuf,
    pub(crate) event_index: usize,
    pub(crate) through_event: bool,
    pub(crate) through_response_index: Option<ResponseIndex>,
    pub(crate) tail: ReplayTail,
    pub(crate) budget: tui_adapter::Budget,
    pub(crate) model: Option<ModelSelection>,
}

impl SelfEditProbeRequest {
    pub(crate) async fn run(self) -> Result<SelfEditProbeRun, PrepareError> {
        run_self_edit_probe(self).await
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Source {
    HeadlessResult { path: PathBuf },
    RawFullResponse { path: PathBuf },
}

impl Source {
    fn path(&self) -> &Path {
        match self {
            Self::HeadlessResult { path } | Self::RawFullResponse { path } => path,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::HeadlessResult { .. } => "headless_result",
            Self::RawFullResponse { .. } => "raw_full_response",
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SelfEditProbeRun {
    pub(crate) request_path: PathBuf,
    pub(crate) source_kind: &'static str,
    pub(crate) source_path: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) selected_events: usize,
    pub(crate) selected_response_records: Option<usize>,
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
    let replay = Replay::load(
        &request.source,
        request.event_index,
        request.through_event,
        request.through_response_index,
    )?;
    let installed_records = replay.installed_records(request.tail);
    let tape = replay.tape(request.tail)?;
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
        source_kind: request.source.label(),
        source_path: request.source.path().to_path_buf(),
        workspace: request.workspace,
        selected_events: replay.selected_events,
        selected_response_records: replay.selected_response_records,
        selected_tool_requests: replay.selected_tool_requests,
        installed_records,
        terminal,
        historical_failures: replay.historical_failures,
        observed,
    })
}

#[derive(Debug)]
struct Replay {
    records: Vec<RawFullResponseRecord>,
    selected_events: usize,
    selected_response_records: Option<usize>,
    selected_tool_requests: usize,
    historical_failures: Vec<SelfEditFailure>,
}

impl Replay {
    fn load(
        source: &Source,
        event_index: usize,
        through_event: bool,
        through_response_index: Option<ResponseIndex>,
    ) -> Result<Self, PrepareError> {
        match source {
            Source::HeadlessResult { path } => {
                if let Some(response_index) = through_response_index {
                    return Err(run_prepare_error(
                        "self_edit_replay_source",
                        format!(
                            "--through-response-index {response_index} only applies to --raw-full-response"
                        ),
                    ));
                }
                let historical = load_historical_summary(path)?;
                let selected_events = selected_events(&historical, event_index, through_event);
                let historical_requests = tool_requests_from_events(&selected_events)?;
                let records = records_from_tool_requests(&historical_requests)?;
                Ok(Self {
                    records,
                    selected_events: selected_events.len(),
                    selected_response_records: None,
                    selected_tool_requests: historical_requests.len(),
                    historical_failures: historical_failures(&historical, event_index),
                })
            }
            Source::RawFullResponse { path } => {
                let records = load_raw_full_response_records(path)?;
                let records = select_raw_response_records(records, through_response_index)?;
                let selected_tool_requests = count_tool_request_records(&records)?;
                let selected_response_records = records.len();
                Ok(Self {
                    records,
                    selected_events: 0,
                    selected_response_records: Some(selected_response_records),
                    selected_tool_requests,
                    historical_failures: Vec::new(),
                })
            }
        }
    }

    fn installed_records(&self, tail: ReplayTail) -> usize {
        match tail {
            ReplayTail::Stop => self.records.len() + 1,
            ReplayTail::Live | ReplayTail::LiveStep => self.records.len(),
        }
    }

    fn tape(&self, tail: ReplayTail) -> Result<RecordedResponseTape, PrepareError> {
        let mut records = self.records.clone();
        if tail == ReplayTail::Stop {
            let next_index = records
                .last()
                .map(|record| record.response_index().get().saturating_add(1))
                .unwrap_or(0);
            let assistant_id = records
                .last()
                .map(|record| record.assistant_message_id)
                .unwrap_or_else(Uuid::new_v4);
            records.push(stop_response_record(assistant_id, next_index)?);
        }
        Ok(RecordedResponseTape::new(
            records
                .into_iter()
                .map(RawFullResponseRecord::into_recorded_response)
                .collect(),
        ))
    }
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

fn load_raw_full_response_records(path: &Path) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut records = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: RawFullResponseRecord = serde_json::from_str(trimmed).map_err(|source| {
            run_prepare_error(
                "self_edit_replay_raw_response",
                format!(
                    "parse raw provider sidecar {} line {}: {source}",
                    path.display(),
                    line_index + 1
                ),
            )
        })?;
        records.push(record);
    }

    if records.is_empty() {
        return Err(run_prepare_error(
            "self_edit_replay_raw_response",
            format!(
                "raw provider sidecar {} has no response records",
                path.display()
            ),
        ));
    }

    let assistant_ids = records
        .iter()
        .map(|record| record.assistant_message_id)
        .collect::<BTreeSet<_>>();
    if assistant_ids.len() != 1 {
        return Err(run_prepare_error(
            "self_edit_replay_raw_response",
            format!(
                "raw provider sidecar {} contains {} assistant_message_id streams; provide a single self-edit attempt sidecar",
                path.display(),
                assistant_ids.len()
            ),
        ));
    }

    records.sort_by_key(|record| record.response_index());
    let duplicates = duplicate_response_indices(&records);
    if !duplicates.is_empty() {
        let duplicates = duplicates
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(run_prepare_error(
            "self_edit_replay_raw_response",
            format!(
                "raw provider sidecar {} contains duplicate response_index values: {duplicates}",
                path.display()
            ),
        ));
    }
    let missing = missing_response_indices(&records);
    if !missing.is_empty() {
        let missing = missing
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(run_prepare_error(
            "self_edit_replay_raw_response",
            format!(
                "raw provider sidecar {} is incomplete; missing response_index values: {missing}",
                path.display()
            ),
        ));
    }

    Ok(records)
}

fn select_raw_response_records(
    records: Vec<RawFullResponseRecord>,
    through_response_index: Option<ResponseIndex>,
) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let Some(through_response_index) = through_response_index else {
        return Ok(records);
    };
    let mut found = false;
    let selected = records
        .into_iter()
        .filter(|record| {
            let include = record.response_index() <= through_response_index;
            found |= record.response_index() == through_response_index;
            include
        })
        .collect::<Vec<_>>();
    if !found {
        return Err(run_prepare_error(
            "self_edit_replay_raw_response",
            format!(
                "raw provider sidecar does not contain response_index {through_response_index}"
            ),
        ));
    }
    Ok(selected)
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

pub(crate) fn tool_requests_from_events(
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

fn records_from_tool_requests(
    requests: &[ToolRequestRecord],
) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let assistant_id = Uuid::new_v4();
    let mut records = Vec::new();
    for (index, request) in requests.iter().enumerate() {
        records.push(tool_response_record(assistant_id, index, request)?);
    }
    Ok(records)
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

fn count_tool_request_records(records: &[RawFullResponseRecord]) -> Result<usize, PrepareError> {
    let mut count = 0_usize;
    for record in records {
        let body = serde_json::to_string(record.response()).map_err(PrepareError::Serialize)?;
        let step = parse_chat_outcome(&body).map_err(|source| {
            run_prepare_error(
                "self_edit_replay_raw_response",
                format!(
                    "parse raw response_index {}: {source}",
                    record.response_index()
                ),
            )
        })?;
        if let ploke_llm::manager::ChatStepOutcome::ToolCalls { calls, .. } = step.outcome {
            count += calls.len();
        }
    }
    Ok(count)
}

fn missing_response_indices(records: &[RawFullResponseRecord]) -> Vec<ResponseIndex> {
    let Some(last) = records.last().map(|record| record.response_index().get()) else {
        return Vec::new();
    };
    let present = records
        .iter()
        .map(|record| record.response_index().get())
        .collect::<BTreeSet<_>>();
    (0..=last)
        .filter(|index| !present.contains(index))
        .map(ResponseIndex::new)
        .collect()
}

fn duplicate_response_indices(records: &[RawFullResponseRecord]) -> Vec<ResponseIndex> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for record in records {
        let response_index = record.response_index();
        if !seen.insert(response_index) {
            duplicates.insert(response_index);
        }
    }
    duplicates.into_iter().collect()
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
        tui_adapter::HeadlessTerminal::AppliedValidationFailed { feedback, .. } => {
            format!(
                "applied_validation_failed feedback={}",
                truncate_chars(feedback, 160)
            )
        }
        tui_adapter::HeadlessTerminal::AppliedValidationMissing { missing, .. } => {
            format!("applied_validation_missing missing={}", missing.join(", "))
        }
        tui_adapter::HeadlessTerminal::AppliedTurnAborted {
            outcome, summary, ..
        } => {
            format!(
                "applied_turn_aborted outcome={outcome} summary={}",
                truncate_chars(summary, 160)
            )
        }
        tui_adapter::HeadlessTerminal::AppliedTimedOut { secs, .. } => {
            format!("applied_timed_out secs={secs}")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_full_response_source_loads_single_assistant_stream() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("llm_full_response.log");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                response_line(assistant, 1, "stop"),
                tool_response_line(assistant, 0)
            ),
        )
        .expect("write sidecar");

        let replay = Replay::load(&Source::RawFullResponse { path }, 0, false, None)
            .expect("load raw source");

        assert_eq!(replay.selected_events, 0);
        assert_eq!(replay.selected_response_records, Some(2));
        assert_eq!(replay.selected_tool_requests, 1);
        assert_eq!(
            replay
                .records
                .iter()
                .map(|record| record.response_index().get())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(replay.installed_records(ReplayTail::Stop), 3);
    }

    #[test]
    fn raw_full_response_source_slices_through_response_index() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("llm_full_response.log");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        fs::write(
            &path,
            format!(
                "{}\n{}\n{}\n",
                tool_response_line(assistant, 0),
                tool_response_line(assistant, 1),
                response_line(assistant, 2, "stop"),
            ),
        )
        .expect("write sidecar");

        let replay = Replay::load(
            &Source::RawFullResponse { path },
            0,
            false,
            Some(ResponseIndex::new(1)),
        )
        .expect("load raw prefix");

        assert_eq!(replay.selected_response_records, Some(2));
        assert_eq!(replay.selected_tool_requests, 2);
        assert_eq!(
            replay
                .records
                .iter()
                .map(|record| record.response_index().get())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn raw_full_response_source_counts_parallel_tool_calls() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("llm_full_response.log");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                parallel_tool_response_line(assistant, 0),
                response_line(assistant, 1, "stop"),
            ),
        )
        .expect("write sidecar");

        let replay = Replay::load(&Source::RawFullResponse { path }, 0, false, None)
            .expect("load raw source with parallel tool calls");

        assert_eq!(replay.selected_response_records, Some(2));
        assert_eq!(
            replay.selected_tool_requests, 2,
            "operator health output should count actual tool calls, not responses"
        );
    }

    #[test]
    fn raw_full_response_source_rejects_multiple_assistant_streams() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("llm_full_response.log");
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                response_line(
                    Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa),
                    0,
                    "a"
                ),
                response_line(
                    Uuid::from_u128(0xbbbbbbbb_bbbb_bbbb_bbbb_bbbbbbbbbbbb),
                    0,
                    "b"
                ),
            ),
        )
        .expect("write sidecar");

        let err = Replay::load(&Source::RawFullResponse { path }, 0, false, None)
            .expect_err("mixed assistant streams should be rejected");

        assert!(
            err.to_string().contains("assistant_message_id streams"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn raw_full_response_source_rejects_duplicate_response_indices() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("llm_full_response.log");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                response_line(assistant, 0, "a"),
                response_line(assistant, 0, "b"),
            ),
        )
        .expect("write sidecar");

        let err = Replay::load(&Source::RawFullResponse { path }, 0, false, None)
            .expect_err("duplicate response indexes should be rejected");

        assert!(
            err.to_string().contains("duplicate response_index"),
            "unexpected error: {err}"
        );
    }

    fn response_line(assistant_message_id: Uuid, response_index: usize, content: &str) -> String {
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
                        "content": content
                    }
                }],
                "created": 0,
                "model": "test/model",
                "object": "chat.completion"
            }
        })
        .to_string()
    }

    fn tool_response_line(assistant_message_id: Uuid, response_index: usize) -> String {
        serde_json::json!({
            "assistant_message_id": assistant_message_id,
            "response_index": response_index,
            "response": {
                "id": format!("response-{response_index}"),
                "choices": [{
                    "index": 0,
                    "finish_reason": "tool_calls",
                    "message": {
                        "role": "assistant",
                        "tool_calls": [{
                            "id": "call-1",
                            "type": "function",
                            "function": {
                                "name": "list_dir",
                                "arguments": "{\"dir\":\".\"}"
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

    fn parallel_tool_response_line(assistant_message_id: Uuid, response_index: usize) -> String {
        serde_json::json!({
            "assistant_message_id": assistant_message_id,
            "response_index": response_index,
            "response": {
                "id": format!("response-{response_index}"),
                "choices": [{
                    "index": 0,
                    "finish_reason": "tool_calls",
                    "message": {
                        "role": "assistant",
                        "tool_calls": [
                            {
                                "id": "call-1",
                                "type": "function",
                                "function": {
                                    "name": "list_dir",
                                    "arguments": "{\"dir\":\".\"}"
                                }
                            },
                            {
                                "id": "call-2",
                                "type": "function",
                                "function": {
                                    "name": "read_file",
                                    "arguments": "{\"file\":\"Cargo.toml\"}"
                                }
                            }
                        ]
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
