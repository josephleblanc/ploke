//! Live replay probes over typed agent-turn playback anchors.
//!
//! This module is the library body behind `ploke-eval run replay turn-live`.
//! The CLI chooses paths and rendering; this module owns the executable probe:
//!
//! 1. Resolve a `ploke-tree::TurnCursor` into a `TurnAnchor`.
//! 2. Install either the full recorded provider tape or a selected prefix.
//! 3. Start the normal headless TUI runtime against the requested workspace.
//! 4. Submit the recorded prompt and let `session.rs` drive tool calls.
//! 5. Either stop when the prefix is exhausted or live-tail through the normal
//!    provider path.
//! 6. Capture provider requests so the operator can see whether the live tail
//!    was reached and what context the model received.
//!
//! The key property is that the probe replays provider/model output, not tool
//! events. Historical tool calls still flow through the current search/code/edit
//! tools in the target workspace, which is what makes this useful for checking
//! whether recent tool-surface fixes actually help the live agent.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use ploke_records::llm_response::RawFullResponseRecord;
use ploke_tree::TurnCursor;
use serde::{Deserialize, Serialize};

use crate::{
    cli::prototype1_state::edit_surface::{
        surface_policy::SurfacePolicy,
        tui_adapter::{self, ModelSelection},
    },
    spec::PrepareError,
};

use super::turn::{
    ReplayPrefixSelector, ReplayTail, ResolvedReplayPrefix, TurnAnchor, resolve_replay_prefix_at,
};

const REPLAY_BRANCH_SCHEMA: &str = "ploke-eval-replay-branch.v1";

#[derive(Debug, Clone)]
pub(crate) struct ProbeRequest {
    /// Directory containing typed run records and `llm-full-responses.jsonl`.
    ///
    /// This is kept explicit rather than inferred from a campaign id so early
    /// use-testing can point at whichever historical node/output directory is
    /// interesting without adding selector/discovery semantics to the CLI.
    pub(crate) run_dir: PathBuf,
    /// Target checkout whose current files, index, and tool behavior are used.
    ///
    /// The recorded prefix may contain old tool arguments, but the actual tool
    /// execution happens here. That distinction is the point of the probe.
    pub(crate) workspace: PathBuf,
    pub(crate) cursor: TurnCursor,
    pub(crate) prefix_selector: ReplayPrefixSelector,
    pub(crate) tail: ReplayTail,
    pub(crate) budget: ProbeBudget,
    pub(crate) model: Option<ModelSelection>,
    /// Previously observed live responses for this replay branch.
    ///
    /// Branch records are appended after the selected historical prefix before
    /// the tape is installed. That makes a second CLI invocation continue from
    /// the first live step instead of repeating it. The branch records are still
    /// provider response envelopes; tool results are re-created by running the
    /// current TUI tools again against the requested workspace.
    pub(crate) branch: Option<ReplayBranchTape>,
    /// Optional branch tape output path.
    ///
    /// When `tail == LiveStep`, newly captured live provider responses are
    /// appended to the incoming branch and written here. The file is a replay
    /// input for a later `--branch-in`, not an authority record for Prototype 1
    /// History or candidate admission.
    pub(crate) branch_out: Option<PathBuf>,
}

impl ProbeRequest {
    pub(crate) async fn run(self) -> Result<ProbeRun, PrepareError> {
        run_prefix_then_live_probe(self).await
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub(crate) struct ProbeBudget {
    pub(crate) max_attempts: u32,
    pub(crate) timeout_secs: u64,
}

impl ProbeBudget {
    pub(crate) fn new(max_attempts: u32, timeout_secs: u64) -> Self {
        Self {
            max_attempts,
            timeout_secs,
        }
    }

    fn into_tui_budget(self) -> Result<tui_adapter::Budget, PrepareError> {
        tui_adapter::Budget::new(self.max_attempts, self.timeout_secs).map_err(|source| {
            PrepareError::DatabaseSetup {
                phase: "replay_probe_budget",
                detail: source.to_string(),
            }
        })
    }
}

impl Default for ProbeBudget {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            timeout_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProbeRun {
    pub(crate) run_dir: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) anchor: TurnAnchor,
    pub(crate) live_model: Option<String>,
    pub(crate) live_provider: Option<String>,
    /// Git snapshot of the target workspace before the probe starts.
    ///
    /// This is an operator signal only. It helps distinguish pre-existing
    /// workspace edits from changes produced by the current live step.
    pub(crate) workspace_before: WorkspaceGit,
    /// Git snapshot of the target workspace after the probe finishes.
    pub(crate) workspace_after: WorkspaceGit,
    pub(crate) tape_path: PathBuf,
    pub(crate) prefix: ResolvedReplayPrefix,
    /// Number of recorded provider responses installed for this probe.
    ///
    /// This can be less than the historical sidecar length when the operator
    /// chooses `ThroughResponseIndex` or `ThroughEvent`.
    pub(crate) tape_records: usize,
    /// Number of provider responses loaded from `--branch-in`.
    pub(crate) branch_in_records: usize,
    /// Replayable live responses accumulated for this probe branch.
    ///
    /// These records are useful for step-by-step debugging only. They do not
    /// contain tool results and do not replace agent-turn artifacts; they let a
    /// later probe rebuild the same provider-response prefix through the live
    /// session/tool path.
    pub(crate) branch_records: Vec<RawFullResponseRecord>,
    pub(crate) branch_out: Option<PathBuf>,
    pub(crate) budget: ProbeBudget,
    pub(crate) elapsed_ms: u128,
    /// Requests observed at the provider boundary.
    ///
    /// `RecordedPrefixThenLive` captures a request before each chat step,
    /// including recorded steps. If this count is greater than `tape_records`,
    /// the session reached the first live provider request after consuming the
    /// historical prefix.
    pub(crate) captured_requests: Vec<CapturedRequest>,
    /// Provider responses observed by the response tap during this probe.
    ///
    /// The tap sees both recorded prefix responses and live responses. A live
    /// response is identified by having an index at or beyond the installed
    /// tape length, after any incoming branch records have been appended.
    pub(crate) captured_responses: Vec<RawFullResponseRecord>,
    /// Full headless TUI events for this probe invocation.
    ///
    /// `tui` is the persisted/bounded evidence summary. It intentionally stores
    /// short previews, which is right for JSON evidence but too lossy for
    /// interactive step-through output: a truncated `request_code_context`
    /// payload can hide whether later results were useful or misleading. Table
    /// rendering may use these full in-memory events to produce bounded
    /// per-result displays without changing the persisted summary shape.
    #[serde(skip_serializing)]
    pub(crate) events: Vec<tui_adapter::Event>,
    pub(crate) tui: tui_adapter::evidence::Summary,
}

impl ProbeRun {
    pub(crate) fn tail_reached(&self) -> bool {
        // The request tap fires before both recorded and live chat steps. Once
        // the captured count exceeds the loaded tape length, the next request
        // was sent to the live provider path rather than served from tape.
        self.prefix.tail == ReplayTail::Live && self.captured_requests.len() > self.tape_records
    }

    pub(crate) fn live_step_response_count(&self) -> usize {
        // `tape_records` is the full installed prefix length: historical prefix
        // plus any incoming branch. Anything captured at or beyond that index
        // came from the live provider during this invocation.
        self.captured_responses
            .iter()
            .filter(|record| record.response_index().get() >= self.tape_records)
            .count()
    }

    pub(crate) fn live_step_response_records(&self) -> Vec<&RawFullResponseRecord> {
        // `tape_records` is the full installed prefix length: historical prefix
        // plus any incoming branch. Anything captured at or beyond that index
        // came from the live provider during this invocation.
        self.captured_responses
            .iter()
            .filter(|record| record.response_index().get() >= self.tape_records)
            .collect()
    }

    pub(crate) fn live_step_tool_calls(&self) -> Vec<ProbeToolCall> {
        self.live_step_response_records()
            .into_iter()
            .flat_map(probe_tool_calls_from_response_record)
            .collect()
    }

    pub(crate) fn live_step_tool_events(&self) -> Vec<&tui_adapter::Event> {
        let call_ids = self
            .live_step_tool_calls()
            .into_iter()
            .map(|call| call.call_id)
            .collect::<BTreeSet<_>>();
        if call_ids.is_empty() {
            return Vec::new();
        }
        self.events
            .iter()
            .filter(|event| {
                probe_event_call_id(event)
                    .map(|call_id| call_ids.contains(call_id))
                    .unwrap_or(false)
            })
            .collect()
    }

    pub(crate) fn live_step_boundary_reached(&self) -> bool {
        // The request tap fires before a chat step is served. In live-step mode
        // the expected boundary is the extra request after the allowed live
        // response and its tool results have been appended. That request is
        // intentionally stopped by the replay step limiter.
        self.prefix.tail == ReplayTail::LiveStep
            && self.captured_requests.len() > self.tape_records + self.live_step_response_count()
    }

    pub(crate) fn terminal_label(&self) -> String {
        match self.tui.terminal.as_ref() {
            Some(tui_adapter::evidence::Terminal::Applied { .. }) => "applied".to_string(),
            Some(tui_adapter::evidence::Terminal::Exhausted { .. }) => "exhausted".to_string(),
            Some(tui_adapter::evidence::Terminal::CompletedWithoutEdit { .. }) => {
                "completed_without_edit".to_string()
            }
            Some(tui_adapter::evidence::Terminal::ToolFailed { .. }) => "tool_failed".to_string(),
            Some(tui_adapter::evidence::Terminal::NoEdit) => "no_edit".to_string(),
            Some(tui_adapter::evidence::Terminal::ContextUnavailable { .. }) => {
                "context_unavailable".to_string()
            }
            Some(tui_adapter::evidence::Terminal::ProviderUnavailable { .. }) => {
                "provider_unavailable".to_string()
            }
            Some(tui_adapter::evidence::Terminal::SetupUnavailable { .. }) => {
                "setup_unavailable".to_string()
            }
            Some(tui_adapter::evidence::Terminal::AppliedValidationFailed { .. }) => {
                "applied_validation_failed".to_string()
            }
            Some(tui_adapter::evidence::Terminal::AppliedValidationMissing { .. }) => {
                "applied_validation_missing".to_string()
            }
            Some(tui_adapter::evidence::Terminal::AppliedTurnAborted { .. }) => {
                "applied_turn_aborted".to_string()
            }
            Some(tui_adapter::evidence::Terminal::AppliedTimedOut { .. }) => {
                "applied_timed_out".to_string()
            }
            Some(tui_adapter::evidence::Terminal::TimedOut { .. }) => "timed_out".to_string(),
            None => "unknown".to_string(),
        }
    }

    pub(crate) fn attempt_count(&self) -> usize {
        self.tui.attempts.len()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WorkspaceGit {
    pub(crate) dirty_paths: Vec<PathBuf>,
    pub(crate) error: Option<String>,
}

impl WorkspaceGit {
    pub(crate) fn dirty_count(&self) -> usize {
        self.dirty_paths.len()
    }

    pub(crate) fn is_dirty(&self) -> bool {
        !self.dirty_paths.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProbeToolCall {
    pub(crate) response_index: usize,
    pub(crate) call_id: String,
    pub(crate) tool: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReplayBranchTape {
    pub(crate) schema: String,
    pub(crate) records: Vec<RawFullResponseRecord>,
}

impl ReplayBranchTape {
    /// Create a branch tape from live provider responses captured by a probe.
    ///
    /// The branch tape deliberately reuses `RawFullResponseRecord`, the same
    /// persisted provider-envelope shape as `llm-full-responses.jsonl`. It is a
    /// lightweight operator artifact for continuing replay probes, not a new
    /// durable run-record family.
    pub(crate) fn new(records: Vec<RawFullResponseRecord>) -> Self {
        Self {
            schema: REPLAY_BRANCH_SCHEMA.to_owned(),
            records,
        }
    }

    /// Load a replay branch written by a previous live-step invocation.
    ///
    /// Validation is intentionally narrow here: schema compatibility is checked
    /// at load time, while assistant-message identity, duplicate indices, and
    /// contiguous response ordering are checked when the branch is appended to
    /// the resolved historical tape.
    pub(crate) fn load(path: &Path) -> Result<Self, PrepareError> {
        let body = fs::read_to_string(path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "load_replay_branch",
            detail: format!(
                "failed to read replay branch '{}': {source}",
                path.display()
            ),
        })?;
        let tape: Self =
            serde_json::from_str(&body).map_err(|source| PrepareError::DatabaseSetup {
                phase: "load_replay_branch",
                detail: format!(
                    "failed to parse replay branch '{}': {source}",
                    path.display()
                ),
            })?;
        if tape.schema != REPLAY_BRANCH_SCHEMA {
            return Err(PrepareError::DatabaseSetup {
                phase: "load_replay_branch",
                detail: format!(
                    "replay branch '{}' has unsupported schema '{}'",
                    path.display(),
                    tape.schema
                ),
            });
        }
        Ok(tape)
    }

    /// Write the current branch as pretty JSON for operator use.
    ///
    /// This is a small CLI-facing artifact, so it is easier to inspect between
    /// steps than JSONL. The contained records remain the canonical typed
    /// response record shape.
    pub(crate) fn write(&self, path: &Path) -> Result<(), PrepareError> {
        let body = serde_json::to_string_pretty(self).map_err(PrepareError::Serialize)?;
        fs::write(path, body).map_err(|source| PrepareError::DatabaseSetup {
            phase: "write_replay_branch",
            detail: format!(
                "failed to write replay branch '{}': {source}",
                path.display()
            ),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CapturedRequest {
    pub(crate) message_count: usize,
    pub(crate) messages: Vec<ploke_tui::llm::RequestMessage>,
}

pub(crate) async fn run_prefix_then_live_probe(
    request: ProbeRequest,
) -> Result<ProbeRun, PrepareError> {
    if !request.run_dir.is_dir() {
        return Err(PrepareError::DatabaseSetup {
            phase: "replay_probe_run_dir",
            detail: format!(
                "run directory '{}' does not exist",
                request.run_dir.display()
            ),
        });
    }
    if !request.workspace.is_dir() {
        return Err(PrepareError::DatabaseSetup {
            phase: "replay_probe_workspace",
            detail: format!(
                "workspace directory '{}' does not exist",
                request.workspace.display()
            ),
        });
    }

    // Install the tape before starting the headless TUI attempt. The actual
    // session is still created by the normal TUI adapter, so tool requests,
    // proposal handling, path policy, and retry behavior remain current code.
    let (prefix, historical_loaded) = resolve_replay_prefix_at(
        &request.run_dir,
        &request.cursor,
        request.prefix_selector.clone(),
        request.tail,
    )?;
    let branch_in_records = request
        .branch
        .as_ref()
        .map(|branch| branch.records.as_slice())
        .unwrap_or(&[]);
    let loaded = historical_loaded.with_appended_records(branch_in_records)?;
    match request.tail {
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
        ReplayTail::LiveStep => {
            ploke_tui::llm::install_recorded_response_prefix_then_live_steps(
                loaded.clone().into_recorded_response_tape(),
                1,
            );
        }
    }
    let anchor = prefix.anchor.clone();
    let _recorded_guard = RecordedResponseGuard;
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
    let (response_tx, response_rx) = std::sync::mpsc::channel();
    let _response_tap_guard = ploke_tui::llm::install_response_tap(response_tx);

    let tui_budget = request.budget.into_tui_budget()?;
    let live_model = request
        .model
        .as_ref()
        .map(|model| model.model_id().to_string());
    let live_provider = request
        .model
        .as_ref()
        .and_then(|model| model.provider())
        .map(|provider| provider.slug.as_str().to_string());
    let workspace_before = workspace_git(&request.workspace);
    let start = Instant::now();
    // Resubmitting the recorded issue prompt reconstructs the historical
    // conversation prefix at the model boundary. The response tape supplies the
    // old assistant/tool-call output until exhausted; everything below that
    // boundary runs against `request.workspace`.
    let run = tui_adapter::run_headless_with_model(
        &request.workspace,
        &anchor.issue_prompt,
        tui_budget,
        &SurfacePolicy::workspace_except_core(),
        &[],
        request.model.clone(),
    )
    .await
    .map_err(|source| PrepareError::DatabaseSetup {
        phase: "replay_probe_run",
        detail: source.to_string(),
    })?;
    let workspace_after = workspace_git(&request.workspace);

    let captured_requests = drain_captured_requests(&request_rx);
    let assistant_message_id = loaded.assistant_message_id();
    let captured_responses = drain_captured_responses(assistant_message_id, &response_rx);
    let new_live_records = captured_responses
        .iter()
        .filter(|record| record.response_index().get() >= loaded.record_count())
        .cloned()
        .collect::<Vec<_>>();
    let mut branch_records = branch_in_records.to_vec();
    branch_records.extend(new_live_records);
    if let Some(path) = request.branch_out.as_ref() {
        ReplayBranchTape::new(branch_records.clone()).write(path)?;
    }

    Ok(ProbeRun {
        run_dir: request.run_dir,
        workspace: request.workspace,
        anchor,
        live_model,
        live_provider,
        workspace_before,
        workspace_after,
        tape_path: loaded.path().to_path_buf(),
        prefix,
        tape_records: loaded.records().len(),
        branch_in_records: branch_in_records.len(),
        branch_records,
        branch_out: request.branch_out,
        budget: request.budget,
        elapsed_ms: start.elapsed().as_millis(),
        captured_requests,
        captured_responses,
        events: run.events().to_vec(),
        tui: run.evidence(),
    })
}

fn workspace_git(workspace: &Path) -> WorkspaceGit {
    match Command::new("git")
        .current_dir(workspace)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
    {
        Ok(output) if output.status.success() => WorkspaceGit {
            dirty_paths: parse_git_status_paths(&String::from_utf8_lossy(&output.stdout)),
            error: None,
        },
        Ok(output) => WorkspaceGit {
            dirty_paths: Vec::new(),
            error: Some(format!(
                "git status exited with {}: {}",
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        },
        Err(source) => WorkspaceGit {
            dirty_paths: Vec::new(),
            error: Some(format!("failed to run git status: {source}")),
        },
    }
}

fn parse_git_status_paths(stdout: &str) -> Vec<PathBuf> {
    stdout
        .lines()
        .filter_map(|line| {
            let path = line.get(3..)?.trim();
            if path.is_empty() {
                return None;
            }
            let path = path
                .rsplit_once(" -> ")
                .map(|(_, renamed)| renamed)
                .unwrap_or(path);
            Some(PathBuf::from(path))
        })
        .collect()
}

fn probe_tool_calls_from_response_record(record: &RawFullResponseRecord) -> Vec<ProbeToolCall> {
    let Ok(body) = serde_json::to_string(record.response()) else {
        return Vec::new();
    };
    let Ok(step) = ploke_llm::manager::parse_chat_outcome(&body) else {
        return Vec::new();
    };
    match step.outcome {
        ploke_llm::manager::ChatStepOutcome::ToolCalls { calls, .. } => calls
            .into_iter()
            .map(|call| ProbeToolCall {
                response_index: record.response_index().get(),
                call_id: call.call_id.to_string(),
                tool: call.function.name.as_str().to_owned(),
            })
            .collect(),
        ploke_llm::manager::ChatStepOutcome::Content { .. } => Vec::new(),
    }
}

fn probe_event_call_id(event: &tui_adapter::Event) -> Option<&str> {
    match event {
        tui_adapter::Event::ToolRequest { call_id, .. }
        | tui_adapter::Event::Tool { call_id, .. } => Some(call_id),
        _ => None,
    }
}

fn drain_captured_requests(
    request_rx: &std::sync::mpsc::Receiver<Vec<ploke_tui::llm::RequestMessage>>,
) -> Vec<CapturedRequest> {
    request_rx
        .try_iter()
        .map(|messages| CapturedRequest {
            message_count: messages.len(),
            messages,
        })
        .collect()
}

fn drain_captured_responses(
    assistant_message_id: uuid::Uuid,
    response_rx: &std::sync::mpsc::Receiver<ploke_llm::manager::RecordedResponse>,
) -> Vec<RawFullResponseRecord> {
    response_rx
        .try_iter()
        .map(|recorded_response| RawFullResponseRecord {
            assistant_message_id,
            recorded_response,
        })
        .collect()
}

struct RecordedResponseGuard;

impl Drop for RecordedResponseGuard {
    fn drop(&mut self) {
        ploke_tui::llm::clear_recorded_response_tape();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_git_status_paths_handles_basic_and_renamed_paths() {
        let paths = parse_git_status_paths(
            " M crates/printer/src/util.rs\n?? scratch.txt\nR  old.rs -> src/new.rs\n",
        );

        assert_eq!(
            paths,
            vec![
                PathBuf::from("crates/printer/src/util.rs"),
                PathBuf::from("scratch.txt"),
                PathBuf::from("src/new.rs"),
            ]
        );
    }
}
