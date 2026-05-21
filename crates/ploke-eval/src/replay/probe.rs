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

use std::{path::PathBuf, time::Instant};

use ploke_tree::TurnCursor;
use serde::Serialize;

use crate::{
    cli::prototype1_state::edit_surface::{
        harness_request::BroadEditPolicy,
        tui_adapter::{self, ModelSelection},
    },
    spec::PrepareError,
};

use super::turn::{
    ReplayPrefixSelector, ReplayTail, ResolvedReplayPrefix, TurnAnchor, install_replay_prefix_at,
};

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
    pub(crate) tape_path: PathBuf,
    pub(crate) prefix: ResolvedReplayPrefix,
    /// Number of recorded provider responses installed for this probe.
    ///
    /// This can be less than the historical sidecar length when the operator
    /// chooses `ThroughResponseIndex` or `ThroughEvent`.
    pub(crate) tape_records: usize,
    pub(crate) budget: ProbeBudget,
    pub(crate) elapsed_ms: u128,
    /// Requests observed at the provider boundary.
    ///
    /// `RecordedPrefixThenLive` captures a request before each chat step,
    /// including recorded steps. If this count is greater than `tape_records`,
    /// the session reached the first live provider request after consuming the
    /// historical prefix.
    pub(crate) captured_requests: Vec<CapturedRequest>,
    pub(crate) tui: tui_adapter::evidence::Summary,
}

impl ProbeRun {
    pub(crate) fn tail_reached(&self) -> bool {
        // The request tap fires before both recorded and live chat steps. Once
        // the captured count exceeds the loaded tape length, the next request
        // was sent to the live provider path rather than served from tape.
        self.prefix.tail == ReplayTail::Live && self.captured_requests.len() > self.tape_records
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
            Some(tui_adapter::evidence::Terminal::TimedOut { .. }) => "timed_out".to_string(),
            None => "unknown".to_string(),
        }
    }

    pub(crate) fn attempt_count(&self) -> usize {
        self.tui.attempts.len()
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
    let (prefix, loaded) = install_replay_prefix_at(
        &request.run_dir,
        &request.cursor,
        request.prefix_selector.clone(),
        request.tail,
    )?;
    let anchor = prefix.anchor.clone();
    let _recorded_guard = RecordedResponseGuard;
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);

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
    let start = Instant::now();
    // Resubmitting the recorded issue prompt reconstructs the historical
    // conversation prefix at the model boundary. The response tape supplies the
    // old assistant/tool-call output until exhausted; everything below that
    // boundary runs against `request.workspace`.
    let run = tui_adapter::run_headless_with_model(
        &request.workspace,
        &anchor.issue_prompt,
        tui_budget,
        BroadEditPolicy::WorkspaceExceptPlokeEval,
        &[],
        request.model.clone(),
    )
    .await
    .map_err(|source| PrepareError::DatabaseSetup {
        phase: "replay_probe_run",
        detail: source.to_string(),
    })?;

    Ok(ProbeRun {
        run_dir: request.run_dir,
        workspace: request.workspace,
        anchor,
        live_model,
        live_provider,
        tape_path: loaded.path().to_path_buf(),
        prefix,
        tape_records: loaded.records().len(),
        budget: request.budget,
        elapsed_ms: start.elapsed().as_millis(),
        captured_requests: drain_captured_requests(&request_rx),
        tui: run.evidence(),
    })
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

struct RecordedResponseGuard;

impl Drop for RecordedResponseGuard {
    fn drop(&mut self) {
        ploke_tui::llm::clear_recorded_response_tape();
    }
}
