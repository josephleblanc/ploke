use std::{collections::HashMap, fs, sync::Arc, time::Duration};

use crate::user_config::{ChatPolicy, ChatTimeoutStrategy, ToolLoopMode};
use chrono::DateTime;
use ploke_llm::ChatStepOutcome;
use ploke_llm::manager::{ChatStepData, RecordedResponse, RecordedResponseTape};
use ploke_llm::registry::calibration::{AttemptTimeout, RouterCalibration};
use ploke_llm::response::ToolCall;
use ploke_llm::{ChatHttpConfig, ChatStepError, ProviderAttempt, ProviderRetryDecision};
use ploke_test_utils::workspace_root;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::{broadcast, watch};
use tracing::instrument;
use uuid::Uuid;

use crate::AppEvent;
use crate::EventBus;
use crate::app_state::StateCommand;
use crate::app_state::events::SystemEvent;
use crate::chat_history::MessageUpdate;
use crate::chat_history::{ContextTokens, MessageKind};
use crate::chat_history::{MessageStatus, TokenKind};
use crate::tracing_setup::{FINISH_REASON_TARGET, FULL_RESPONSE_TARGET, TOKENS_TARGET};
use crate::utils::consts::TOOL_CALL_TIMEOUT;
use ploke_llm::RequestMessage;
use ploke_llm::response::FinishReason;
use ploke_llm::response::OpenAiResponse;
use ploke_llm::response::TokenUsage;
use ploke_llm::router_only::{ApiRoute, ChatCompRequest, Router};
use ploke_llm::types::meta::{LLMMetadata, PerformanceMetrics};

use super::{format_tokens_payload, tokens_logging_enabled};
use crate::llm::manager::loop_error::{
    ChatSessionReport, CommitPhase, ErrorAudience, ErrorContext, LoopError, RetryAdvice,
    RetryStrategy, SessionOutcome, Verbosity, build_loop_error_from_semantic_spec,
    classify_finish_reason, classify_llm_error, mark_repair_budget_exhausted, recovery_from_retry,
    render_error_view,
};
use crate::llm::manager::semantics::{self, RecoveryDecision};
use crate::tools::{
    ToolCallPreflightError, ToolError, ToolErrorCode, ToolErrorWire, ToolUiPayload,
    allowed_tool_names, validate_and_sanitize_tool_calls,
};
use ploke_llm::LlmError;
use tokio::time::sleep;

const OPENROUTER_REQUEST_LOG: &str = "logs/openrouter/session/last_request.json";
const OPENROUTER_RESPONSE_LOG_PARSED: &str = "logs/openrouter/session/last_parsed.json";
const OPENROUTER_RESPONSE_LOG_RAW: &str = "logs/openrouter/session/last_response_raw.txt";
const DEFAULT_REPAIR_ATTEMPTS_PER_SESSION: u32 = 4;
const REPLAY_LIVE_STEP_LIMIT_REACHED: &str = "replay live step limit reached";
/// Minimum number of provider HTTP attempts (one retry) for any router. Routers
/// that calibrate a larger retry budget keep it; others are floored here.
const MIN_CHAT_HTTP_ATTEMPTS: u32 = 2;
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FullResponseTraceRecord {
    assistant_message_id: Uuid,
    #[serde(flatten)]
    recorded_response: RecordedResponse,
}

fn compact_tool_content_for_llm_replay(content: &str, max_file_lines: usize) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(content) else {
        return content.to_string();
    };
    let Some(obj) = value.as_object_mut() else {
        return content.to_string();
    };
    if !obj.contains_key("file_path") {
        return content.to_string();
    }
    let Some(file_content) = obj.get_mut("content") else {
        return content.to_string();
    };
    let Some(file_text) = file_content.as_str() else {
        return content.to_string();
    };

    let mut kept = Vec::new();
    let mut truncated = false;
    for (line_count, line) in file_text.lines().enumerate() {
        if line_count >= max_file_lines {
            truncated = true;
            break;
        }
        kept.push(line);
    }

    if !truncated {
        return content.to_string();
    }

    let mut truncated_content = kept.join("\n");
    if file_text.ends_with('\n') && !truncated_content.is_empty() {
        truncated_content.push('\n');
    }
    truncated_content.push_str(&format!(
        "... [truncated for LLM replay after {max_file_lines} lines]"
    ));

    *file_content = serde_json::Value::String(truncated_content);
    obj.insert(
        "llm_replay_truncated".to_string(),
        serde_json::Value::Bool(true),
    );
    obj.insert(
        "llm_replay_max_lines".to_string(),
        serde_json::Value::from(max_file_lines as u64),
    );

    serde_json::to_string(&value).unwrap_or_else(|_| content.to_string())
}

/// Generic per-request session over a router-specific ApiRoute.
pub(crate) struct RequestSession<'a, R>
where
    R: Router,
    R::CompletionFields: ApiRoute,
{
    pub client: &'a Client,
    pub event_bus: Arc<EventBus>,
    pub assistant_message_id: Uuid,
    pub parent_id: Uuid,
    pub req: ChatCompRequest<R>,
    pub fallback_on_404: bool,
    pub attempts: u32,
    pub state_cmd_tx: mpsc::Sender<StateCommand>,
}

// TODO:ploke-llm 2025-12-13
// put these into a better config data structure
// - ensure there is a place to set the defaults for the user
// - ensure the settings are persisted once set by the user, fall back on defaults
#[derive(Clone, Copy, Debug)]
pub struct TuiToolPolicy {
    pub tool_call_timeout: ToolCallTimeout,
    pub tool_call_chain_limit: usize,
    pub tool_loop_mode: ToolLoopMode,
    pub retry_without_tools_on_404: bool,
}

type ToolCallTimeout = Duration;

impl Default for TuiToolPolicy {
    fn default() -> Self {
        Self {
            tool_call_timeout: Duration::from_secs(30),
            // TODO:ploke-llm 2025-12-14
            // Set to 15 as initial default, experiment to determine the right default to set
            tool_call_chain_limit: 100,
            tool_loop_mode: ToolLoopMode::Auto,
            retry_without_tools_on_404: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TuiTimeoutPolicy {
    duration: Option<Duration>,
    strategy: TimeoutStrategy,
}

impl Default for TuiTimeoutPolicy {
    fn default() -> Self {
        Self {
            duration: Some(Duration::from_secs(30)),
            strategy: TimeoutStrategy::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TimeoutStrategy {
    /// Back off attempts, beginning at `TuiTimoutPolicy.duration` and doubling a number of times
    /// equal to the Backoff value. If None, inifite backoff attempts.
    Backoff(Option<usize>),
    /// Number of attempts to perform retry at the `TuiTimoutPolicy.duration`.
    FixedRetry(usize),
    /// No retries, fail early
    Strict,
}

impl Default for TimeoutStrategy {
    fn default() -> Self {
        Self::FixedRetry(3)
    }
}
impl TuiTimeoutPolicy {
    fn next_timout_dur(self, attempt: usize) -> Option<Duration> {
        match self.strategy {
            TimeoutStrategy::Backoff(attempt_max) => {
                if let Some(policy_max) = attempt_max
                    && let Some(dur) = self.duration
                    && attempt <= policy_max
                {
                    Some(dur * 2_u32.pow(attempt as u32).clamp(2, 64_u32))
                } else {
                    None
                }
            }
            TimeoutStrategy::FixedRetry(attempt_max) => {
                if attempt <= attempt_max {
                    self.duration
                } else {
                    None
                }
            }
            TimeoutStrategy::Strict => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TuiErrorPolicy {
    EndlessRetry,
    RetryLimit(u32),
    Strict,
}

#[derive(Clone, Copy, Debug)]
pub enum TuiLengthPolicy {
    RetryLimit(u32),
    Strict,
}

#[derive(Clone, Debug)]
pub struct FinishPolicy {
    /// Timeout backoff/limit behavior for FinishReason::Timeout.
    timeout: TuiTimeoutPolicy,
    /// Retry policy for FinishReason::Error.
    error: TuiErrorPolicy,
    /// Retry policy for FinishReason::Length.
    length: TuiLengthPolicy,
    /// System prompt appended when retrying after FinishReason::Length.
    length_continue_prompt: String,
}

impl Default for TuiErrorPolicy {
    fn default() -> Self {
        Self::RetryLimit(2)
    }
}

impl Default for TuiLengthPolicy {
    fn default() -> Self {
        Self::RetryLimit(1)
    }
}

impl Default for FinishPolicy {
    fn default() -> Self {
        Self {
            timeout: TuiTimeoutPolicy::default(),
            error: TuiErrorPolicy::default(),
            length: TuiLengthPolicy::default(),
            length_continue_prompt: "Continue from where you left off. Do not repeat prior text."
                .to_string(),
        }
    }
}

pub(crate) fn tool_policy_from_chat(cfg: &ChatPolicy) -> TuiToolPolicy {
    TuiToolPolicy {
        tool_call_timeout: Duration::from_secs(cfg.tool_call_timeout_secs),
        tool_call_chain_limit: cfg.tool_call_chain_limit,
        tool_loop_mode: cfg.tool_loop_mode,
        retry_without_tools_on_404: cfg.retry_without_tools_on_404,
    }
}

pub(crate) fn finish_policy_from_chat(cfg: &ChatPolicy) -> FinishPolicy {
    let strategy = match cfg.timeout_strategy {
        ChatTimeoutStrategy::Backoff { attempts } => TimeoutStrategy::Backoff(attempts),
        ChatTimeoutStrategy::FixedRetry { attempts } => TimeoutStrategy::FixedRetry(attempts),
        ChatTimeoutStrategy::Strict => TimeoutStrategy::Strict,
    };
    let timeout = TuiTimeoutPolicy {
        duration: Some(Duration::from_secs(cfg.timeout_base_secs)),
        strategy,
    };
    FinishPolicy {
        timeout,
        error: TuiErrorPolicy::RetryLimit(cfg.error_retry_limit),
        length: TuiLengthPolicy::RetryLimit(cfg.length_retry_limit),
        length_continue_prompt: cfg.length_continue_prompt.clone(),
    }
}

fn should_retry_error(policy: TuiErrorPolicy, retried_errors: &mut u32) -> bool {
    match policy {
        TuiErrorPolicy::EndlessRetry => {
            *retried_errors = retried_errors.saturating_add(1);
            true
        }
        TuiErrorPolicy::RetryLimit(limit) => {
            if *retried_errors < limit {
                *retried_errors += 1;
                true
            } else {
                false
            }
        }
        TuiErrorPolicy::Strict => false,
    }
}

fn should_retry_length(policy: TuiLengthPolicy, retried_lengths: &mut u32) -> bool {
    match policy {
        TuiLengthPolicy::RetryLimit(limit) => {
            if *retried_lengths < limit {
                *retried_lengths += 1;
                true
            } else {
                false
            }
        }
        TuiLengthPolicy::Strict => false,
    }
}

fn repair_budget_exhausted(state: &ChatLoopState, limit: u32) -> bool {
    // Keep repair bounded independently from generic request retries and from the broader
    // tool-call chain cap so repeated provider/model repair loops cannot dominate the turn.
    state.repair_attempts >= limit
}

fn consume_repair_budget(
    state: &mut ChatLoopState,
    loop_error: &mut LoopError,
    limit: u32,
) -> bool {
    if !matches!(loop_error.recovery, RecoveryDecision::Repair { .. }) {
        return true;
    }
    if repair_budget_exhausted(state, limit) {
        mark_repair_budget_exhausted(loop_error);
        return false;
    }
    state.repair_attempts = state.repair_attempts.saturating_add(1);
    true
}

/// Outcome of finish-reason evaluation for a single response.
///
/// Continue variants tell the caller to retry the chat step, optionally with
/// a system message appended before the next request.
struct FinishContinue {
    finish_reason: Option<FinishReason>,
    system_prompt: Option<String>,
}

enum FinishDecision {
    Continue(FinishContinue),
    Return(Result<OpenAiResponse, LlmError>),
}

/// Internal aggregation of failure reasons across multiple choices.
enum FinishFailure {
    FinishError {
        msg: String,
        finish_reason: FinishReason,
    },
    Error(LlmError),
}

/// Mutable counters for retry behavior within a chat session.
#[derive(Default, Debug, Clone, Copy)]
struct ChatLoopState {
    retried_errors: u32,
    retried_lengths: u32,
    timeout_attempts: usize,
    request_error_retries: u32,
    repair_attempts: u32,
}

fn provider_retry_exhausted(attempts: &[ProviderAttempt]) -> bool {
    attempts
        .last()
        .is_some_and(|attempt| attempt.retry_decision == ProviderRetryDecision::Exhausted)
}

/// Borrowed context required to evaluate finish reasons.
struct ChatLoopContext<'a> {
    cfg: &'a mut ChatHttpConfig,
    model_key: &'a Option<ploke_llm::ModelKey>,
}

impl FinishPolicy {
    /// Decide whether to return, continue, or continue with a system message
    /// based on the finish reasons found in the response choices.
    fn handle_finish_reasons(
        &self,
        full_response: OpenAiResponse,
        ctx: &mut ChatLoopContext<'_>,
        state: &mut ChatLoopState,
    ) -> FinishDecision {
        let span = tracing::trace_span!(
            target: FINISH_REASON_TARGET,
            "finish_reason",
            retried_errors = state.retried_errors,
            retried_lengths = state.retried_lengths,
            timeout_attempts = state.timeout_attempts,
            timeout_policy = ?self.timeout,
            error_policy = ?self.error,
            length_policy = ?self.length
        );
        let _enter = span.enter();
        let mut continue_chain = false;
        let mut continue_reason: Option<FinishReason> = None;
        let mut continue_message: Option<String> = None;
        let mut failure: Option<FinishFailure> = None;
        let mut saw_finish_reason = false;
        let mut stop = false;

        for choice in &full_response.choices {
            let Some(finish_reason) = choice.finish_reason.clone() else {
                continue;
            };
            saw_finish_reason = true;
            let native_finish_reason = choice.native_finish_reason.as_deref();
            tracing::trace!(
                target = FINISH_REASON_TARGET,
                ?finish_reason,
                ?native_finish_reason,
                "finish reason received"
            );

            match finish_reason {
                FinishReason::Stop => {
                    tracing::trace!(
                        target = FINISH_REASON_TARGET,
                        "finish reason decision: stop"
                    );
                    stop = true;
                    break;
                }
                FinishReason::Length => {
                    if should_retry_length(self.length, &mut state.retried_lengths) {
                        continue_message = Some(self.length_continue_prompt.to_string());
                        continue_chain = true;
                        if continue_reason.is_none() {
                            continue_reason = Some(finish_reason.clone());
                        }
                        tracing::trace!(
                            target = FINISH_REASON_TARGET,
                            continue_with_message = true,
                            "finish reason decision: continue"
                        );
                    } else if failure.is_none() {
                        failure = Some(FinishFailure::FinishError {
                            msg: "Provider stopped due to length; try reducing output or retrying."
                                .to_string(),
                            finish_reason,
                        });
                        tracing::trace!(
                            target = FINISH_REASON_TARGET,
                            "finish reason decision: failure"
                        );
                    }
                }
                // should be shown to user
                FinishReason::ContentFilter => {
                    if failure.is_none() {
                        failure = Some(FinishFailure::FinishError {
                            msg: "Provider reports ContentFilter applied, try again.".to_string(),
                            finish_reason,
                        });
                    }
                    tracing::trace!(
                        target = FINISH_REASON_TARGET,
                        "finish reason decision: failure"
                    );
                }
                FinishReason::MalformedFunctionCall => {
                    if failure.is_none() {
                        failure = Some(FinishFailure::FinishError {
                            msg: "Provider returned malformed function call.".to_string(),
                            finish_reason,
                        });
                    }
                    tracing::trace!(
                        target = FINISH_REASON_TARGET,
                        "finish reason decision: failure"
                    );
                }
                FinishReason::UnexpectedToolCall => {
                    if failure.is_none() {
                        failure = Some(FinishFailure::FinishError {
                            msg: "Provider invoked an undeclared function (unexpected tool call)."
                                .to_string(),
                            finish_reason,
                        });
                    }
                    tracing::trace!(
                        target = FINISH_REASON_TARGET,
                        "finish reason decision: failure"
                    );
                }
                // keep looping
                FinishReason::ToolCalls => {
                    continue_chain = true;
                    tracing::trace!(
                        target = FINISH_REASON_TARGET,
                        continue_with_message = false,
                        "finish reason decision: continue"
                    );
                }
                // retry on timout policy
                FinishReason::Timeout => {
                    state.timeout_attempts = state.timeout_attempts.saturating_add(1);
                    if let Some(next_timout) = self.timeout.next_timout_dur(state.timeout_attempts)
                    {
                        // if some, change timout for next loop and ocntinue
                        ctx.cfg.attempt_timeout = AttemptTimeout::fixed(next_timout);
                        continue_chain = true;
                        if continue_reason.is_none() {
                            continue_reason = Some(finish_reason.clone());
                        }
                        tracing::trace!(
                            target = FINISH_REASON_TARGET,
                            continue_with_message = false,
                            "finish reason decision: continue"
                        );
                    } else if failure.is_none() {
                        failure = Some(FinishFailure::Error(LlmError::Timeout));
                        tracing::trace!(
                            target = FINISH_REASON_TARGET,
                            "finish reason decision: failure"
                        );
                    }
                }
                FinishReason::Error(ref e) => {
                    if should_retry_error(self.error, &mut state.retried_errors) {
                        tracing::warn!(
                            target = "chat-loop",
                            error = %e,
                            retried_errors = state.retried_errors,
                            ?ctx.model_key,
                            ?native_finish_reason,
                            "FinishReason::Error, retrying"
                        );
                        continue_chain = true;
                        if continue_reason.is_none() {
                            continue_reason = Some(finish_reason.clone());
                        }
                        tracing::trace!(
                            target = FINISH_REASON_TARGET,
                            continue_with_message = false,
                            "finish reason decision: continue"
                        );
                    } else if failure.is_none() {
                        failure = Some(FinishFailure::FinishError {
                            msg: e.to_string(),
                            finish_reason,
                        });
                        tracing::trace!(
                            target = FINISH_REASON_TARGET,
                            "finish reason decision: failure"
                        );
                    }
                }
            }
        }

        if stop {
            return FinishDecision::Return(Ok(full_response));
        }

        if continue_chain {
            return FinishDecision::Continue(FinishContinue {
                finish_reason: continue_reason,
                system_prompt: continue_message,
            });
        }

        if let Some(failure) = failure {
            let err = match failure {
                FinishFailure::FinishError { msg, finish_reason } => LlmError::FinishError {
                    msg,
                    full_response,
                    finish_reason,
                },
                FinishFailure::Error(err) => err,
            };
            return FinishDecision::Return(Err(err));
        }

        if !saw_finish_reason {
            tracing::trace!(
                target = FINISH_REASON_TARGET,
                "finish reason decision: none"
            );
            return FinishDecision::Return(Err(LlmError::ChatStep(
                "No finish reason in llm response choices.".to_string(),
            )));
        }

        tracing::trace!(
            target = FINISH_REASON_TARGET,
            "finish reason decision: unhandled"
        );
        FinishDecision::Return(Err(LlmError::ChatStep(
            "Unhandled finish reason in llm response choices.".to_string(),
        )))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CancelChatToken {
    KeepOpen,
    Close,
}

#[derive(Debug)]
pub enum ChatStepSource {
    /// Use the configured provider/client for every chat step.
    Live,
    /// Replay only recorded provider envelopes.
    ///
    /// This is useful for deterministic replay regressions. The replay still
    /// flows through the normal session parser and tool execution path, but no
    /// live provider request is allowed once the tape is exhausted.
    Recorded(RecordedResponseTape),
    /// Replay a recorded prefix, then continue with normal live provider calls.
    ///
    /// This is the broad smoke-probe mode: it reconstructs the historical
    /// model-output prefix, lets the current tools handle those calls, and then
    /// gives the live model the resulting conversation state.
    RecordedPrefixThenLive(RecordedResponseTape),
    /// Replay a recorded prefix, then allow a bounded number of live steps.
    ///
    /// A "step" here is one provider response envelope, not one tool call. If
    /// that response requests tools, the normal session loop executes the whole
    /// batch and appends tool results before the step limit is enforced. The
    /// next attempted provider request returns the internal replay-limit error,
    /// which `run_chat_session` turns into a clean completed report. This gives
    /// CLI probes a stop point just before the model would see the next request.
    RecordedPrefixThenLiveSteps {
        tape: RecordedResponseTape,
        live_steps_remaining: usize,
    },
}

impl ChatStepSource {
    pub fn live() -> Self {
        Self::Live
    }

    pub fn recorded(tape: RecordedResponseTape) -> Self {
        Self::Recorded(tape)
    }

    pub fn recorded_prefix_then_live(tape: RecordedResponseTape) -> Self {
        Self::RecordedPrefixThenLive(tape)
    }

    pub fn recorded_prefix_then_live_steps(
        tape: RecordedResponseTape,
        live_steps_remaining: usize,
    ) -> Self {
        Self::RecordedPrefixThenLiveSteps {
            tape,
            live_steps_remaining,
        }
    }

    async fn next_step<R: Router>(
        &mut self,
        client: &Client,
        req: &ChatCompRequest<R>,
        cfg: &ChatHttpConfig,
    ) -> Result<ChatStepData, ChatStepError> {
        capture_request_for_tap(req);
        match self {
            Self::Live => ploke_llm::chat_step_with_attempts(client, req, cfg).await,
            Self::Recorded(tape) => tape.next_chat_step(),
            Self::RecordedPrefixThenLive(tape) if tape.remaining() > 0 => tape.next_chat_step(),
            Self::RecordedPrefixThenLive(_) => {
                ploke_llm::chat_step_with_attempts(client, req, cfg).await
            }
            Self::RecordedPrefixThenLiveSteps { tape, .. } if tape.remaining() > 0 => {
                tape.next_chat_step()
            }
            Self::RecordedPrefixThenLiveSteps {
                live_steps_remaining,
                ..
            } if *live_steps_remaining > 0 => {
                *live_steps_remaining = live_steps_remaining.saturating_sub(1);
                ploke_llm::chat_step_with_attempts(client, req, cfg).await
            }
            Self::RecordedPrefixThenLiveSteps { .. } => Err(ChatStepError::new(
                LlmError::ChatStep(REPLAY_LIVE_STEP_LIMIT_REACHED.to_string()),
            )),
        }
    }
}

impl Default for ChatStepSource {
    fn default() -> Self {
        Self::Live
    }
}

#[cfg(feature = "test_harness")]
static RECORDED_RESPONSE_TAPE: std::sync::OnceLock<
    std::sync::Mutex<Option<InstalledChatStepSource>>,
> = std::sync::OnceLock::new();

#[cfg(feature = "test_harness")]
static REQUEST_TAP: std::sync::OnceLock<
    std::sync::Mutex<Option<std::sync::mpsc::Sender<Vec<RequestMessage>>>>,
> = std::sync::OnceLock::new();

#[cfg(feature = "test_harness")]
static RESPONSE_TAP: std::sync::OnceLock<
    std::sync::Mutex<Option<std::sync::mpsc::Sender<RecordedResponse>>>,
> = std::sync::OnceLock::new();

#[cfg(feature = "test_harness")]
enum InstalledChatStepSource {
    Recorded(RecordedResponseTape),
    RecordedPrefixThenLive(RecordedResponseTape),
    /// Test-harness installer for branchable replay probes.
    ///
    /// This is intentionally keyed in the same global slot as the existing
    /// recorded tape installers so only one chat-step source can be active for
    /// a session. `ploke-eval` owns the CLI semantics; this enum only carries
    /// the session-level source that `take_recorded_chat_step_source` consumes.
    RecordedPrefixThenLiveSteps {
        tape: RecordedResponseTape,
        live_steps: usize,
    },
}

#[cfg(feature = "test_harness")]
pub struct RequestTapGuard;

#[cfg(feature = "test_harness")]
impl Drop for RequestTapGuard {
    fn drop(&mut self) {
        clear_request_tap();
    }
}

#[cfg(feature = "test_harness")]
pub struct ResponseTapGuard;

#[cfg(feature = "test_harness")]
impl Drop for ResponseTapGuard {
    fn drop(&mut self) {
        clear_response_tap();
    }
}

#[cfg(feature = "test_harness")]
pub fn install_recorded_response_tape(tape: RecordedResponseTape) {
    let lock = RECORDED_RESPONSE_TAPE.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("recorded response tape lock should not be poisoned");
    *guard = Some(InstalledChatStepSource::Recorded(tape));
}

#[cfg(feature = "test_harness")]
pub fn install_recorded_response_prefix_then_live(tape: RecordedResponseTape) {
    let lock = RECORDED_RESPONSE_TAPE.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("recorded response tape lock should not be poisoned");
    *guard = Some(InstalledChatStepSource::RecordedPrefixThenLive(tape));
}

#[cfg(feature = "test_harness")]
pub fn install_recorded_response_prefix_then_live_steps(
    tape: RecordedResponseTape,
    live_steps: usize,
) {
    // Used by replay probes that need "advance once, report, then stop"
    // behavior. The recorded prefix preserves historical model output; the
    // live-step limit prevents the headless TUI attempt from running to a full
    // terminal edit/retry outcome before the operator can inspect the tools.
    let lock = RECORDED_RESPONSE_TAPE.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("recorded response tape lock should not be poisoned");
    *guard = Some(InstalledChatStepSource::RecordedPrefixThenLiveSteps { tape, live_steps });
}

#[cfg(feature = "test_harness")]
pub fn clear_recorded_response_tape() {
    let lock = RECORDED_RESPONSE_TAPE.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("recorded response tape lock should not be poisoned");
    *guard = None;
}

#[cfg(feature = "test_harness")]
pub fn install_request_tap(
    sender: std::sync::mpsc::Sender<Vec<RequestMessage>>,
) -> RequestTapGuard {
    let lock = REQUEST_TAP.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("request tap lock should not be poisoned");
    *guard = Some(sender);
    RequestTapGuard
}

#[cfg(feature = "test_harness")]
pub fn clear_request_tap() {
    let lock = REQUEST_TAP.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("request tap lock should not be poisoned");
    *guard = None;
}

#[cfg(feature = "test_harness")]
pub fn install_response_tap(sender: std::sync::mpsc::Sender<RecordedResponse>) -> ResponseTapGuard {
    // Captures provider envelopes after parsing, including recorded responses
    // and live responses. `ploke-eval` converts these back into typed
    // `RawFullResponseRecord` lines so a live-step probe can be continued by a
    // later CLI invocation without inventing a second persisted shape.
    let lock = RESPONSE_TAP.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("response tap lock should not be poisoned");
    *guard = Some(sender);
    ResponseTapGuard
}

#[cfg(feature = "test_harness")]
pub fn clear_response_tap() {
    let lock = RESPONSE_TAP.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("response tap lock should not be poisoned");
    *guard = None;
}

#[cfg(feature = "test_harness")]
fn capture_request_for_tap<R: Router>(req: &ChatCompRequest<R>) {
    let lock = REQUEST_TAP.get_or_init(|| std::sync::Mutex::new(None));
    let guard = lock
        .lock()
        .expect("request tap lock should not be poisoned");
    if let Some(sender) = guard.as_ref() {
        let _ = sender.send(req.core.messages.clone());
    }
}

#[cfg(not(feature = "test_harness"))]
fn capture_request_for_tap<R: Router>(_req: &ChatCompRequest<R>) {}

#[cfg(feature = "test_harness")]
fn capture_response_for_tap(response_index: usize, response: &OpenAiResponse) {
    let lock = RESPONSE_TAP.get_or_init(|| std::sync::Mutex::new(None));
    let guard = lock
        .lock()
        .expect("response tap lock should not be poisoned");
    if let Some(sender) = guard.as_ref() {
        let _ = sender.send(RecordedResponse::new(response_index, response.clone()));
    }
}

#[cfg(not(feature = "test_harness"))]
fn capture_response_for_tap(_response_index: usize, _response: &OpenAiResponse) {}

fn is_replay_live_step_limit_error(error: &LlmError) -> bool {
    // This sentinel is not a model failure. It is the intentional breakpoint
    // used by `RecordedPrefixThenLiveSteps` after the allowed live responses
    // have been consumed and their tool results are already in the request.
    matches!(error, LlmError::ChatStep(message) if message == REPLAY_LIVE_STEP_LIMIT_REACHED)
}

fn is_replay_boundary_error(error: &LlmError) -> bool {
    // Both cases mean the replay harness reached an operator-chosen boundary:
    // a recorded-only tape ended, or a live-step probe consumed its allowed
    // live provider envelope. Neither should enter model-repair handling.
    is_replay_live_step_limit_error(error) || matches!(error, LlmError::ReplayExhausted(_))
}

#[cfg(feature = "test_harness")]
pub(super) fn take_recorded_chat_step_source() -> ChatStepSource {
    let lock = RECORDED_RESPONSE_TAPE.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock
        .lock()
        .expect("recorded response tape lock should not be poisoned");
    guard
        .take()
        .map(|source| match source {
            InstalledChatStepSource::Recorded(tape) => ChatStepSource::recorded(tape),
            InstalledChatStepSource::RecordedPrefixThenLive(tape) => {
                ChatStepSource::recorded_prefix_then_live(tape)
            }
            InstalledChatStepSource::RecordedPrefixThenLiveSteps { tape, live_steps } => {
                ChatStepSource::recorded_prefix_then_live_steps(tape, live_steps)
            }
        })
        .unwrap_or_else(ChatStepSource::live)
}

#[cfg(not(feature = "test_harness"))]
pub(super) fn take_recorded_chat_step_source() -> ChatStepSource {
    ChatStepSource::live()
}

pub struct ChatSession<R: Router> {
    pub client: Client,
    pub req: ChatCompRequest<R>,
    pub chat_step_source: ChatStepSource,
    pub parent_id: Uuid,
    pub assistant_message_id: Uuid,
    pub event_bus: Arc<EventBus>,
    pub state_cmd_tx: mpsc::Sender<StateCommand>,
    pub included_message_ids: Vec<Uuid>,
    pub chat_policy: ChatPolicy,
    pub cancel_rx: watch::Receiver<CancelChatToken>,
}

async fn wait_for_cancel_signal(cancel_rx: &mut watch::Receiver<CancelChatToken>) {
    loop {
        if matches!(*cancel_rx.borrow(), CancelChatToken::Close) {
            return;
        }
        if cancel_rx.changed().await.is_err() {
            return;
        }
    }
}

async fn abort_for_user_cancel(
    report: &mut ChatSessionReport,
    state_cmd_tx: &mpsc::Sender<StateCommand>,
    assistant_message_id: Uuid,
    initial_message_updated: &mut bool,
    attempts: u32,
    chain_index: usize,
    model_key: &Option<ploke_llm::ModelKey>,
    commit_phase: CommitPhase,
) -> ChatSessionReport {
    let err = LlmError::ChatStep("Cancelled by user.".to_string());
    let context = base_error_context(
        attempts,
        chain_index,
        "user_cancel",
        model_key,
        assistant_message_id,
    );
    let loop_error = classify_llm_error(&err, context, commit_phase.clone());
    emit_loop_error(
        state_cmd_tx,
        assistant_message_id,
        initial_message_updated,
        &loop_error,
    )
    .await;
    report.record_error(loop_error.clone());
    report.outcome = SessionOutcome::Aborted {
        error_id: loop_error.error_id,
    };
    report.commit_phase = commit_phase;
    report.attempts = attempts;
    report.clone()
}

/// Chat loop structure:
/// - issue a chat step
/// - handle tool calls (if any), update UI, append tool results
/// - handle finish reasons to decide return vs retry
// Optionally: set tool_choice=Auto if tools exist, etc.
pub async fn run_chat_session<R: Router + RouterCalibration>(
    session: ChatSession<R>,
    llm_timeout_secs: u64,
) -> ChatSessionReport {
    let ChatSession {
        client,
        mut req,
        mut chat_step_source,
        parent_id,
        assistant_message_id,
        event_bus,
        state_cmd_tx,
        included_message_ids,
        chat_policy,
        mut cancel_rx,
    } = session;
    let policy = tool_policy_from_chat(&chat_policy);
    let finish_policy = finish_policy_from_chat(&chat_policy);
    let repair_attempt_limit = chat_policy.repair_attempt_limit.max(1);
    let http_timeout = Duration::from_secs(llm_timeout_secs);
    let mut loop_state = ChatLoopState::default();
    let model_key = req.model_key.clone();
    let session_id = Uuid::new_v4();
    let mut report = ChatSessionReport::new(
        session_id,
        assistant_message_id,
        parent_id,
        assistant_message_id,
    );
    let mut commit_phase = CommitPhase::PreCommit;
    let mut attempts = 0_u32;

    let mut initial_message_updated = false;
    for chain_index in 0..policy.tool_call_chain_limit {
        attempts = attempts.saturating_add(1);
        if matches!(*cancel_rx.borrow(), CancelChatToken::Close) {
            return abort_for_user_cancel(
                &mut report,
                &state_cmd_tx,
                assistant_message_id,
                &mut initial_message_updated,
                attempts,
                chain_index,
                &model_key,
                commit_phase,
            )
            .await;
        }

        if tokens_logging_enabled() {
            let request_payload = format_tokens_payload(&req);
            tracing::info!(
                target: TOKENS_TARGET,
                session_id = %session_id,
                parent_id = %parent_id,
                assistant_message_id = %assistant_message_id,
                model = ?model_key,
                attempt = attempts,
                kind = "api_request",
                request = %request_payload,
                "Outgoing chat request (truncated when large)"
            );
        }
        let calibration_input = R::calibration_input(&req);
        let mut provider_timing = R::resolve_provider_timing(calibration_input);
        provider_timing.attempt_timeout = AttemptTimeout::fixed(http_timeout);
        // Honor the router-calibrated HTTP attempt budget (e.g. the direct-Google
        // path opts into a larger exponential-backoff budget to ride out Vertex
        // DSQ 429s) while keeping a floor of one retry for routers that do not
        // customize it. The per-attempt timeout stays statically session-driven.
        provider_timing.max_attempts = provider_timing.max_attempts.max(MIN_CHAT_HTTP_ATTEMPTS);
        let mut cfg = ChatHttpConfig::from(&provider_timing);
        let ChatStepData {
            outcome,
            full_response,
            provider_attempts,
        } = match tokio::select! {
            res = chat_step_source.next_step(&client, &req, &cfg) => res,
            _ = wait_for_cancel_signal(&mut cancel_rx) => {
                return abort_for_user_cancel(
                    &mut report,
                    &state_cmd_tx,
                    assistant_message_id,
                    &mut initial_message_updated,
                    attempts,
                    chain_index,
                    &model_key,
                    commit_phase,
                ).await;
            }
        } {
            Ok(step) => step,
            Err(chat_step_error) => {
                let provider_attempts = chat_step_error.provider_attempts;
                let provider_exhausted = provider_retry_exhausted(&provider_attempts);
                report.record_chat_step(chain_index, provider_timing.clone(), provider_attempts);
                let err = chat_step_error.source;
                if is_replay_boundary_error(&err) {
                    report.outcome = SessionOutcome::Completed;
                    report.commit_phase = commit_phase;
                    report.attempts = attempts;
                    return report;
                }
                let allowed = allowed_tool_names();
                let semantic_context = base_error_context(
                    attempts,
                    chain_index,
                    "parse_response",
                    &model_key,
                    assistant_message_id,
                );
                if let Some(spec) = semantics::normalize_llm_error(&err, &allowed, semantic_context)
                {
                    let mut loop_error =
                        build_loop_error_from_semantic_spec(spec, commit_phase.clone());
                    if !consume_repair_budget(
                        &mut loop_state,
                        &mut loop_error,
                        repair_attempt_limit,
                    ) {
                        emit_loop_error(
                            &state_cmd_tx,
                            assistant_message_id,
                            &mut initial_message_updated,
                            &loop_error,
                        )
                        .await;
                        report.record_error(loop_error.clone());
                        report.outcome = SessionOutcome::Aborted {
                            error_id: loop_error.error_id,
                        };
                        report.commit_phase = commit_phase;
                        report.attempts = attempts;
                        return report;
                    }
                    emit_loop_error(
                        &state_cmd_tx,
                        assistant_message_id,
                        &mut initial_message_updated,
                        &loop_error,
                    )
                    .await;
                    push_llm_payload(&mut req, &loop_error);
                    report.record_error(loop_error);
                    continue;
                }
                let context = base_error_context(
                    attempts,
                    chain_index,
                    "chat_step",
                    &model_key,
                    assistant_message_id,
                );
                let loop_error = classify_llm_error(&err, context, commit_phase.clone());
                if !provider_exhausted
                    && matches!(&loop_error.recovery, RecoveryDecision::Retry { .. })
                    && loop_state.request_error_retries < chat_policy.error_retry_limit
                {
                    loop_state.request_error_retries =
                        loop_state.request_error_retries.saturating_add(1);
                    report.record_error(loop_error.clone());

                    let retry_delay = match &loop_error.recovery {
                        RecoveryDecision::Retry {
                            strategy: RetryStrategy::Fixed,
                            ..
                        } => finish_policy
                            .timeout
                            .duration
                            .unwrap_or_else(|| Duration::from_secs(0)),
                        RecoveryDecision::Retry {
                            strategy: RetryStrategy::Backoff,
                            ..
                        } => finish_policy
                            .timeout
                            .next_timout_dur(loop_state.request_error_retries as usize)
                            .unwrap_or_else(|| {
                                finish_policy
                                    .timeout
                                    .duration
                                    .unwrap_or_else(|| Duration::from_secs(0))
                            }),
                        _ => Duration::from_secs(0),
                    };

                    tracing::warn!(
                        target = "chat-loop",
                        error = %err,
                        retried_request_errors = loop_state.request_error_retries,
                        retry_delay_secs = retry_delay.as_secs_f32(),
                        ?model_key,
                        "chat_step failed; retrying"
                    );

                    tokio::select! {
                        _ = sleep(retry_delay) => {}
                        _ = wait_for_cancel_signal(&mut cancel_rx) => {
                            return abort_for_user_cancel(
                                &mut report,
                                &state_cmd_tx,
                                assistant_message_id,
                                &mut initial_message_updated,
                                attempts,
                                chain_index,
                                &model_key,
                                commit_phase,
                            ).await;
                        }
                    }

                    continue;
                }

                emit_loop_error(
                    &state_cmd_tx,
                    assistant_message_id,
                    &mut initial_message_updated,
                    &loop_error,
                )
                .await;
                report.record_error(loop_error.clone());
                report.outcome = SessionOutcome::Aborted {
                    error_id: loop_error.error_id,
                };
                report.commit_phase = commit_phase;
                report.attempts = attempts;
                return report;
            }
        };
        report.record_chat_step(chain_index, provider_timing, provider_attempts);
        emit_full_response_trace(
            session_id,
            parent_id,
            assistant_message_id,
            &model_key,
            chain_index,
            &full_response,
        );
        capture_response_for_tap(chain_index, &full_response);

        let token_usage = full_response.usage;
        if let Some(resp_tokens) = token_usage {
            state_cmd_tx
                .send(StateCommand::UpdateContextTokens {
                    tokens: ContextTokens {
                        count: resp_tokens.prompt_tokens as usize,
                        kind: TokenKind::Actual,
                    },
                })
                .await
                .expect("Invariant: state manager running");
        }
        match outcome {
            ChatStepOutcome::ToolCalls {
                calls,
                content,
                reasoning,
                ..
            } => {
                let calls = match validate_and_sanitize_tool_calls(&calls) {
                    Ok(validated) => validated,
                    Err(preflight_error) => {
                        let context = base_error_context(
                            attempts,
                            chain_index,
                            "tool_call_preflight",
                            &model_key,
                            assistant_message_id,
                        );
                        let spec = semantics::normalize_tool_call_preflight_error(
                            preflight_error,
                            provider_slug_from_response(&full_response),
                            context,
                        );
                        let mut loop_error =
                            build_loop_error_from_semantic_spec(spec, commit_phase.clone());
                        if !consume_repair_budget(
                            &mut loop_state,
                            &mut loop_error,
                            repair_attempt_limit,
                        ) {
                            emit_loop_error(
                                &state_cmd_tx,
                                assistant_message_id,
                                &mut initial_message_updated,
                                &loop_error,
                            )
                            .await;
                            report.record_error(loop_error.clone());
                            report.outcome = SessionOutcome::Aborted {
                                error_id: loop_error.error_id,
                            };
                            report.commit_phase = commit_phase;
                            report.attempts = attempts;
                            return report;
                        }
                        emit_loop_error(
                            &state_cmd_tx,
                            assistant_message_id,
                            &mut initial_message_updated,
                            &loop_error,
                        )
                        .await;
                        push_llm_payload(&mut req, &loop_error);
                        report.record_error(loop_error);
                        continue;
                    }
                };
                let assistant_msg = if content.as_ref().is_some_and(|c| !c.is_empty()) {
                    content.as_ref().map(|s| s.to_string())
                } else if reasoning.as_ref().is_some_and(|r| !r.is_empty()) {
                    reasoning.as_ref().map(|s| s.to_string())
                } else {
                    None
                };
                req.core
                    .messages
                    .push(RequestMessage::new_assistant_with_tool_calls(
                        content.map(|s| s.to_string()),
                        calls.clone(),
                    ));
                let step_request_id = Uuid::new_v4();
                // 1) update placeholder message once (UI concern)
                add_or_update_assistant_message(
                    assistant_message_id,
                    &state_cmd_tx,
                    &mut initial_message_updated,
                    assistant_msg.unwrap_or_else(|| "Calling tools...".to_string()),
                    MessageStatus::Completed,
                )
                .await;
                commit_phase = CommitPhase::MessageCommitted;

                // 2) run tools (EventBus + waiting is TUI concern)
                let mut call_name_by_id: HashMap<ploke_core::ArcStr, ploke_core::ArcStr> =
                    HashMap::new();
                for call in &calls {
                    call_name_by_id.insert(
                        call.call_id.clone(),
                        ploke_core::ArcStr::from(call.function.name.as_str()),
                    );
                }
                let results = execute_tools_via_event_bus(
                    event_bus.clone(),
                    parent_id,
                    step_request_id,
                    calls,
                    policy.tool_call_timeout,
                    policy.tool_loop_mode,
                );
                let results = tokio::select! {
                    result = results => result,
                    _ = wait_for_cancel_signal(&mut cancel_rx) => {
                        return abort_for_user_cancel(
                            &mut report,
                            &state_cmd_tx,
                            assistant_message_id,
                            &mut initial_message_updated,
                            attempts,
                            chain_index,
                            &model_key,
                            commit_phase,
                        ).await;
                    }
                };

                // 3) append tool results into req.core.messages for the next step
                for (call_id, tool_json_result) in results.into_iter() {
                    let call_id_for_state = call_id.clone();
                    match tool_json_result {
                        Ok(tool_result) => {
                            let replay_content = compact_tool_content_for_llm_replay(
                                &tool_result.content,
                                chat_policy.tool_replay.max_file_lines,
                            );
                            req.core
                                .messages
                                .push(RequestMessage::new_tool(replay_content, call_id.clone()));
                            let _ = send_state_command_or_warn(
                                &state_cmd_tx,
                                StateCommand::AddMessageTool {
                                    new_msg_id: Uuid::new_v4(),
                                    msg: tool_result.content,
                                    kind: MessageKind::Tool,
                                    tool_call_id: call_id_for_state,
                                    tool_payload: tool_result.ui_payload,
                                },
                                "tool_result_message",
                            )
                            .await;
                            commit_phase = CommitPhase::ToolResultsCommitted;
                        }
                        Err(tool_error) => {
                            let content =
                                if let Some(wire) = ToolErrorWire::parse(&tool_error.error) {
                                    serde_json::to_string(&wire.llm)
                                        .unwrap_or_else(|_| tool_error.error.clone())
                                } else {
                                    json!({ "ok": false, "error": tool_error.error }).to_string()
                                };
                            req.core
                                .messages
                                .push(RequestMessage::new_tool(content.clone(), call_id.clone()));

                            let _ = send_state_command_or_warn(
                                &state_cmd_tx,
                                StateCommand::AddMessageTool {
                                    new_msg_id: Uuid::new_v4(),
                                    msg: content,
                                    kind: MessageKind::Tool,
                                    tool_call_id: call_id_for_state,
                                    tool_payload: tool_error.ui_payload,
                                },
                                "tool_error_message",
                            )
                            .await;
                            commit_phase = CommitPhase::ToolResultsCommitted;
                            let mut context = base_error_context(
                                attempts,
                                chain_index,
                                "tool_execution",
                                &model_key,
                                assistant_message_id,
                            );
                            context.tool_call_id = Some(call_id.clone());
                            if let Some(tool_name) = call_name_by_id.get(&call_id) {
                                context.tool_name = Some(tool_name.clone());
                            }
                            let tool_err = LlmError::ToolCall(tool_error.error);
                            let loop_error =
                                classify_llm_error(&tool_err, context, commit_phase.clone());
                            push_llm_payload(&mut req, &loop_error);
                            report.record_error(loop_error);
                            continue;
                        }
                    }
                }

                // loop again
            }
            ChatStepOutcome::Content {
                content: None,
                reasoning: None,
            } => {
                let err = LlmError::ChatStep(
                    "No content, reasoning, or tool calls in llm chat step response. This indicates an issue with the chat/tool call loop.".to_string(),
                );
                let context = base_error_context(
                    attempts,
                    chain_index,
                    "parse_response",
                    &model_key,
                    assistant_message_id,
                );
                let loop_error = classify_llm_error(&err, context, commit_phase.clone());
                emit_loop_error(
                    &state_cmd_tx,
                    assistant_message_id,
                    &mut initial_message_updated,
                    &loop_error,
                )
                .await;
                report.record_error(loop_error.clone());
                report.outcome = SessionOutcome::Aborted {
                    error_id: loop_error.error_id,
                };
                report.commit_phase = commit_phase;
                report.attempts = attempts;
                return report;
            }
            ChatStepOutcome::Content {
                content: Some(msg),
                reasoning: None,
            } => {
                add_or_update_assistant_message(
                    assistant_message_id,
                    &state_cmd_tx,
                    &mut initial_message_updated,
                    msg.to_string(),
                    MessageStatus::Completed,
                )
                .await;
                commit_phase = CommitPhase::MessageCommitted;
            }
            ChatStepOutcome::Content {
                content: None,
                reasoning: Some(msg),
            } => {
                add_or_update_assistant_message(
                    assistant_message_id,
                    &state_cmd_tx,
                    &mut initial_message_updated,
                    msg.to_string(),
                    MessageStatus::Completed,
                )
                .await;
                commit_phase = CommitPhase::MessageCommitted;
            }
            ChatStepOutcome::Content {
                content: Some(content_msg),
                reasoning: Some(reasoning_msg),
            } => {
                let x = "";
                let msg = format!(
                    "{x:-^10} Reasoning {x:-^10}\n
                    {reasoning_msg}\n
                    {x:^20}
                    {content_msg}"
                );
                add_or_update_assistant_message(
                    assistant_message_id,
                    &state_cmd_tx,
                    &mut initial_message_updated,
                    msg,
                    MessageStatus::Completed,
                )
                .await;
                commit_phase = CommitPhase::MessageCommitted;
            }
        };

        let mut ctx = ChatLoopContext {
            cfg: &mut cfg,
            model_key: &model_key,
        };

        match finish_policy.handle_finish_reasons(full_response.clone(), &mut ctx, &mut loop_state)
        {
            FinishDecision::Continue(continue_info) => {
                if let Some(reason) = continue_info.finish_reason {
                    let context = base_error_context(
                        attempts,
                        chain_index,
                        "finish_reason",
                        &model_key,
                        assistant_message_id,
                    );
                    let mut loop_error =
                        classify_finish_reason(&reason, context, commit_phase.clone());
                    if let Some(prompt) = continue_info.system_prompt {
                        apply_prompt_hint(&mut loop_error, prompt);
                    }
                    if !matches!(loop_error.retry, RetryAdvice::Yes { .. }) {
                        let retry = RetryAdvice::Yes {
                            strategy: RetryStrategy::Fixed,
                            reason: ploke_core::ArcStr::from("Retrying within session"),
                        };
                        loop_error.recovery = recovery_from_retry(&retry);
                        loop_error.retry = retry;
                    }
                    push_llm_payload(&mut req, &loop_error);
                    report.record_error(loop_error);
                } else if let Some(prompt) = continue_info.system_prompt {
                    req.core.messages.push(RequestMessage::new_system(prompt));
                }
                continue;
            }
            FinishDecision::Return(result) => match result {
                Ok(_response) => {
                    let response_clone = full_response.clone();
                    if let Some(usage) = response_clone.usage {
                        if tokens_logging_enabled() {
                            tracing::info!(
                                target: TOKENS_TARGET,
                                session_id = %session_id,
                                parent_id = %parent_id,
                                assistant_message_id = %assistant_message_id,
                                model = ?model_key,
                                kind = "actual_usage",
                                prompt_tokens = usage.prompt_tokens,
                                completion_tokens = usage.completion_tokens,
                                total_tokens = usage.total_tokens,
                                "Actual token usage from provider"
                            );
                        }
                        let finish_reason = full_response
                            .choices
                            .iter()
                            .find_map(|c| c.finish_reason.clone())
                            .unwrap_or(FinishReason::Stop);
                        let metadata = LLMMetadata {
                            model: response_clone.model,
                            usage,
                            finish_reason,
                            processing_time: Duration::default(),
                            cost: estimate_cost(usage),
                            performance: PerformanceMetrics {
                                tokens_per_second: 0.0,
                                time_to_first_token: Duration::default(),
                                queue_time: Duration::default(),
                            },
                        };
                        let _ = state_cmd_tx
                            .send(StateCommand::UpdateMessage {
                                id: assistant_message_id,
                                update: MessageUpdate {
                                    metadata: Some(metadata),
                                    ..Default::default()
                                },
                            })
                            .await;
                    }
                    report.outcome = SessionOutcome::Completed;
                    report.commit_phase = commit_phase;
                    report.attempts = attempts;
                    match state_cmd_tx
                        .send(StateCommand::DecrementChatTtl {
                            included_message_ids: included_message_ids.clone(),
                        })
                        .await
                    {
                        Ok(()) => {
                            tracing::info!(
                                target: "chat-loop",
                                "Decremented chat TTL after successful completion"
                            );
                        }
                        Err(err) => {
                            tracing::warn!(
                                target: "chat-loop",
                                error = %err,
                                "Failed to decrement chat TTL after successful completion"
                            );
                        }
                    }
                    return report;
                }
                Err(err) => {
                    let context = base_error_context(
                        attempts,
                        chain_index,
                        "finish_reason",
                        &model_key,
                        assistant_message_id,
                    );
                    let loop_error = classify_llm_error(&err, context, commit_phase.clone());
                    emit_loop_error(
                        &state_cmd_tx,
                        assistant_message_id,
                        &mut initial_message_updated,
                        &loop_error,
                    )
                    .await;
                    report.record_error(loop_error.clone());
                    report.outcome = SessionOutcome::Exhausted {
                        error_id: loop_error.error_id,
                    };
                    report.commit_phase = commit_phase;
                    report.attempts = attempts;
                    return report;
                }
            },
        }
    }

    let err = LlmError::ToolCall("tool call chain limit exceeded".into());
    let context = base_error_context(
        attempts,
        policy.tool_call_chain_limit,
        "tool_call_chain_limit",
        &model_key,
        assistant_message_id,
    );
    let loop_error = classify_llm_error(&err, context, commit_phase.clone());
    emit_loop_error(
        &state_cmd_tx,
        assistant_message_id,
        &mut initial_message_updated,
        &loop_error,
    )
    .await;
    report.record_error(loop_error.clone());
    report.outcome = SessionOutcome::Aborted {
        error_id: loop_error.error_id,
    };
    report.commit_phase = commit_phase;
    report.attempts = attempts;
    report
}

fn emit_full_response_trace(
    session_id: Uuid,
    parent_id: Uuid,
    assistant_message_id: Uuid,
    model_key: &Option<ploke_llm::ModelKey>,
    chain_index: usize,
    full_response: &OpenAiResponse,
) {
    let trace_record = FullResponseTraceRecord {
        assistant_message_id,
        recorded_response: RecordedResponse::new(chain_index, full_response.clone()),
    };

    match serde_json::to_string(&trace_record) {
        Ok(response_json) => {
            tracing::info!(target: FULL_RESPONSE_TARGET, "{response_json}");
        }
        Err(error) => {
            tracing::warn!(
                target: "ploke_tui",
                session_id = %session_id,
                parent_id = %parent_id,
                assistant_message_id = %assistant_message_id,
                model = ?model_key,
                %error,
                "Failed to serialize full_response for tracing"
            );
        }
    }
}

async fn add_or_update_assistant_message(
    assistant_message_id: Uuid,
    state_cmd_tx: &mpsc::Sender<StateCommand>,
    initial_message_updated: &mut bool,
    msg: String,
    status: MessageStatus,
) {
    if !*initial_message_updated {
        let is_updated = update_assistant_placeholder_once(
            state_cmd_tx,
            assistant_message_id,
            msg,
            status,
            *initial_message_updated,
        )
        .await;
        *initial_message_updated = is_updated;
    } else {
        let _ = send_state_command_or_warn(
            state_cmd_tx,
            StateCommand::AddMessageImmediate {
                msg,
                kind: MessageKind::Assistant,
                new_msg_id: Uuid::new_v4(),
            },
            "assistant_message",
        )
        .await;
    }
}

async fn emit_loop_error(
    state_cmd_tx: &mpsc::Sender<StateCommand>,
    assistant_message_id: Uuid,
    initial_message_updated: &mut bool,
    error: &LoopError,
) {
    let view = render_error_view(error, ErrorAudience::User, Verbosity::Normal);
    let mut msg = view.summary;
    if let Some(details) = view.details {
        msg.push('\n');
        msg.push_str(&details);
    }

    if !*initial_message_updated {
        let status = MessageStatus::Error {
            description: error.summary.to_string(),
        };
        let is_updated = update_assistant_placeholder_once(
            state_cmd_tx,
            assistant_message_id,
            msg,
            status,
            *initial_message_updated,
        )
        .await;
        *initial_message_updated = is_updated;
        return;
    }

    let _ = send_state_command_or_warn(
        state_cmd_tx,
        StateCommand::AddMessageImmediate {
            msg,
            kind: MessageKind::System,
            new_msg_id: Uuid::new_v4(),
        },
        "loop_error_message",
    )
    .await;
}

fn push_llm_payload<R: Router>(req: &mut ChatCompRequest<R>, error: &LoopError) {
    let view = render_error_view(error, ErrorAudience::Llm, Verbosity::Normal);
    if let Some(payload) = view.llm_payload {
        let payload_str = serde_json::to_string(&payload).unwrap_or_else(|_| {
            "{\"type\":\"ploke.error\",\"summary\":\"serialization_failed\"}".to_string()
        });
        req.core
            .messages
            .push(RequestMessage::new_system(payload_str));
    }
}

fn apply_prompt_hint(error: &mut LoopError, prompt: String) {
    let prompt = ploke_core::ArcStr::from(prompt);
    match error.llm_action.as_mut() {
        Some(action) => {
            if let Some(step) = action
                .next_steps
                .iter_mut()
                .find(|s| s.action.as_ref() == "continue_output")
                && step.details.is_none()
            {
                step.details = Some(prompt);
                return;
            }
            action
                .next_steps
                .push(crate::llm::manager::loop_error::LlmNextStep {
                    action: ploke_core::ArcStr::from("continue_output"),
                    details: Some(prompt),
                });
        }
        None => {
            error.llm_action = Some(crate::llm::manager::loop_error::LlmAction {
                next_steps: vec![crate::llm::manager::loop_error::LlmNextStep {
                    action: ploke_core::ArcStr::from("continue_output"),
                    details: Some(prompt),
                }],
                constraints: Vec::new(),
                retry_hint: None,
            });
        }
    }
}

fn base_error_context(
    attempts: u32,
    chain_index: usize,
    phase: &'static str,
    model_key: &Option<ploke_llm::ModelKey>,
    assistant_message_id: Uuid,
) -> ErrorContext {
    let mut context = ErrorContext::new(attempts, chain_index);
    context.phase = Some(ploke_core::ArcStr::from(phase));
    context.request_id = Some(assistant_message_id);
    if let Some(key) = model_key {
        context.model = Some(ploke_core::ArcStr::from(key.to_string()));
    }
    context
}

fn provider_slug_from_response(response: &OpenAiResponse) -> Option<ploke_core::ArcStr> {
    response
        .provider
        .as_ref()
        .and_then(|name| name.to_slug())
        .map(|slug| ploke_core::ArcStr::from(slug.as_str()))
}

/// Placeholder cost estimator using usage counts.
/// TODO: derive pricing from active model/endpoint and compute USD accurately.
fn estimate_cost(usage: TokenUsage) -> f64 {
    let prompt = usage.prompt_tokens as f64;
    let completion = usage.completion_tokens as f64;
    // Without pricing info, return 0 and keep the surface for future pricing wiring.
    let _ = (prompt, completion);
    0.0
}

#[instrument(target = "chat-loop", skip(state_cmd_tx), fields( msg_content = ?content, initial_message_updated ))]
async fn update_assistant_placeholder_once(
    state_cmd_tx: &mpsc::Sender<StateCommand>,
    assistant_message_id: Uuid,
    content: String,
    status: MessageStatus,
    initial_message_updated: bool,
) -> bool {
    if !initial_message_updated {
        send_state_command_or_warn(
            state_cmd_tx,
            StateCommand::UpdateMessage {
                id: assistant_message_id,
                update: MessageUpdate {
                    content: Some(content),
                    status: Some(status),
                    ..Default::default()
                },
            },
            "assistant_placeholder_update",
        )
        .await
    } else {
        false
    }
}

async fn send_state_command_or_warn(
    state_cmd_tx: &mpsc::Sender<StateCommand>,
    command: StateCommand,
    context: &'static str,
) -> bool {
    match state_cmd_tx.send(command).await {
        Ok(()) => true,
        Err(_) => {
            tracing::warn!(
                target: "chat-loop",
                context,
                "state manager channel closed while emitting chat session message"
            );
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolCallUiResult {
    pub content: String,
    pub ui_payload: Option<ToolUiPayload>,
}

#[derive(Debug, Clone)]
pub struct ToolCallUiError {
    pub error: String,
    pub ui_payload: Option<ToolUiPayload>,
}

pub async fn execute_tools_via_event_bus(
    event_bus: Arc<EventBus>,
    parent_id: Uuid,
    step_request_id: Uuid,
    calls: Vec<ToolCall>,
    policy_timeout: ToolCallTimeout,
    tool_loop_mode: ToolLoopMode,
) -> Vec<(
    ploke_core::ArcStr,
    Result<ToolCallUiResult, ToolCallUiError>,
)> {
    if calls.is_empty() {
        tracing::info!(
            request_id = %step_request_id,
            "execute_tools_via_event_bus received zero tool calls"
        );
        return Vec::new();
    }

    // One receiver for the whole batch
    let mut rx = event_bus.realtime_tx.subscribe();
    let tool_name_by_call = calls
        .iter()
        .map(|call| (call.call_id.clone(), call.function.name))
        .collect::<HashMap<_, _>>();

    // Per-call waiters
    let mut waiters: HashMap<
        ploke_core::ArcStr,
        oneshot::Sender<Result<ToolCallUiResult, ToolCallUiError>>,
    > = HashMap::new();
    let mut handles = Vec::new();

    for call in &calls {
        let (tx, rx_one) = oneshot::channel();
        waiters.insert(call.call_id.clone(), tx);

        let call_id = call.call_id.clone();
        let call_id_for_error = call_id.clone();
        let tool_name = call.function.name;
        let timeout_secs = policy_timeout.as_secs();
        handles.push(async move {
            // timeout wrapper per call
            match tokio::time::timeout(policy_timeout, rx_one).await {
                Ok(Ok(res)) => (call_id, res),
                Ok(Err(_closed)) => (
                    call_id,
                    Err(ToolCallUiError {
                        error: "tool waiter dropped".into(),
                        ui_payload: None,
                    }),
                ),
                Err(_) => (
                    call_id,
                    Err({
                        let message =
                            format!("Timed out waiting for tool result after {timeout_secs}s");
                        let tool_error = ToolError::new(tool_name, ToolErrorCode::Timeout, message)
                            .retry_hint("Increase tool_call_timeout_secs or use a smaller command");
                        let ui_payload =
                            Some(ToolUiPayload::from_error(call_id_for_error, &tool_error));
                        ToolCallUiError {
                            error: tool_error.to_wire_string(),
                            ui_payload,
                        }
                    }),
                ),
            }
        });
    }

    // Dispatcher task routes broadcast events to the correct waiter
    let dispatcher = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id,
                    call_id,
                    content,
                    ui_payload,
                    ..
                })) if request_id == step_request_id => {
                    let tool_name = tool_name_by_call.get(&call_id).copied();
                    if should_wait_for_settled_edit(tool_loop_mode, tool_name, ui_payload.as_ref())
                    {
                        tracing::debug!(
                            request_id = %step_request_id,
                            call_id = %call_id,
                            "gated tool loop waiting for settled edit result"
                        );
                        continue;
                    }
                    if let Some(tx) = waiters.remove(&call_id) {
                        let _ = tx.send(Ok(ToolCallUiResult {
                            content,
                            ui_payload,
                        }));
                    }
                    if waiters.is_empty() {
                        break;
                    }
                }
                Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    call_id,
                    error,
                    ui_payload,
                    ..
                })) if request_id == step_request_id => {
                    if let Some(tx) = waiters.remove(&call_id) {
                        let _ = tx.send(Err(ToolCallUiError { error, ui_payload }));
                    }
                    if waiters.is_empty() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(%n, "tool dispatcher lagged");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // fail all remaining
                    for (_, tx) in waiters.drain() {
                        let _ = tx.send(Err(ToolCallUiError {
                            error: "Event channel closed".into(),
                            ui_payload: None,
                        }));
                    }
                    break;
                }
            }
        }
    });

    // Emit all tool requests *after* dispatcher is live
    for call in calls {
        event_bus.send(AppEvent::System(SystemEvent::ToolCallRequested {
            tool_call: call,
            request_id: step_request_id,
            parent_id,
        }));
    }

    // Await all tool results
    let results = futures::future::join_all(handles).await;

    // Make sure dispatcher finishes too (best-effort)
    let _ = dispatcher.await;

    results
}

fn should_wait_for_settled_edit(
    mode: ToolLoopMode,
    tool_name: Option<crate::tools::ToolName>,
    ui_payload: Option<&ToolUiPayload>,
) -> bool {
    if !matches!(mode, ToolLoopMode::Gated) {
        return false;
    }
    if !tool_name.is_some_and(is_edit_tool) {
        return false;
    }
    ui_payload.is_some_and(is_pending_edit_payload)
}

fn is_edit_tool(tool_name: crate::tools::ToolName) -> bool {
    matches!(
        tool_name,
        crate::tools::ToolName::ApplyCodeEdit
            | crate::tools::ToolName::InsertRustItem
            | crate::tools::ToolName::CreateFile
            | crate::tools::ToolName::NsPatch
    )
}

fn is_pending_edit_payload(payload: &ToolUiPayload) -> bool {
    payload
        .fields
        .iter()
        .any(|field| field.name.as_ref() == "status" && field.value.as_ref() == "pending")
}

use tracing::info;

fn log_api_request_json(url: &str, payload: &str, rel_path: &str) -> color_eyre::Result<()> {
    info!(target: "api_json", "\n// URL: {url}\n// Request\n{payload}\n");
    write_payload(rel_path, payload);
    Ok(())
}

fn log_api_raw_response(url: &str, status: u16, body: &str) -> color_eyre::Result<()> {
    info!(target: "api_json", "\n// URL: {url}\n// Status: {status}\n{body}\n");
    write_payload(OPENROUTER_RESPONSE_LOG_RAW, body);
    Ok(())
}

async fn log_api_parsed_json_response(
    url: &str,
    status: u16,
    parsed: &OpenAiResponse,
) -> color_eyre::Result<()> {
    let payload: String = serde_json::to_string_pretty(parsed)?;
    info!(target: "api_json", "\n// URL: {url}\n// Status: {status}\n{payload}\n");
    write_payload(OPENROUTER_RESPONSE_LOG_PARSED, &payload);
    Ok(())
}

fn write_payload(rel_path: &str, payload: &str) {
    let mut path = workspace_root();
    path.push(rel_path);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, payload);
}

#[tracing::instrument]
async fn add_sysinfo_message(
    call_id: &ploke_core::ArcStr,
    cmd_tx: &mpsc::Sender<StateCommand>,
    status_msg: &str,
) {
    let completed_msg = format!("Tool call {}: {}", status_msg, call_id.as_ref());
    let _ = send_state_command_or_warn(
        cmd_tx,
        StateCommand::AddMessageImmediate {
            msg: completed_msg,
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        },
        "sysinfo_message",
    )
    .await;
}

#[tracing::instrument]
async fn add_tool_failed_message(
    call_id: &ploke_core::ArcStr,
    cmd_tx: &mpsc::Sender<StateCommand>,
    status_msg: &str,
) {
    let completed_msg = format!("Tool call {}: {}", status_msg, call_id.as_ref());
    let _ = send_state_command_or_warn(
        cmd_tx,
        StateCommand::AddMessageImmediate {
            msg: completed_msg,
            kind: MessageKind::System,
            new_msg_id: Uuid::new_v4(),
        },
        "tool_failed_message",
    )
    .await;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc as StdArc, Mutex as StdMutex};
    use std::time::Duration;

    use once_cell::sync::Lazy;
    use ploke_llm::ProviderSlug;
    use ploke_llm::manager::{
        ApproxCharTokenizer, RecordedResponse, Role, TokenCounter, parse_chat_outcome,
    };
    use ploke_llm::registry::calibration::{AttemptTimeout, ProviderTiming, RouterCalibration};
    use ploke_llm::request::endpoint::ToolChoice;
    use ploke_llm::router_only::ChatCompRequest;
    use ploke_llm::router_only::Router;
    use ploke_llm::router_only::google::Google;
    use ploke_llm::router_only::openrouter::ChatCompFields;
    use ploke_llm::router_only::openrouter::OpenRouter;
    use ploke_llm::router_only::openrouter::OpenRouterModelId;
    use ploke_llm::router_only::openrouter::ProviderPreferences;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use tracing::Event;
    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::{Layer, Registry};

    use super::*;
    use crate::EventBus;
    use crate::app_state::AppState;
    use crate::event_bus::EventBusCaps;
    use crate::tools::{FunctionMarker, Tool, ToolName};
    use crate::user_config::ChatPolicy;
    use ploke_db::Database;
    use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
    use ploke_embed::local::{EmbeddingConfig, LocalEmbedder};
    use ploke_embed::runtime::EmbeddingRuntime;
    use ploke_llm::response::FunctionCall;
    use ploke_rag::{RagService, TokenBudget};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::{Mutex, mpsc, watch};
    use tokio::time::timeout;

    static TEST_ROUTER_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    const TEST_ROUTER_URL: &str = "http://127.0.0.1:39181/v1/chat/completions";
    const TEST_ROUTER_URL_ALT: &str = "http://127.0.0.1:39182/v1/chat/completions";

    #[derive(Clone, Default)]
    struct TraceLines(StdArc<StdMutex<Vec<String>>>);

    impl TraceLines {
        fn push(&self, line: String) {
            self.0.lock().expect("trace lock").push(line);
        }

        fn snapshot(&self) -> Vec<String> {
            self.0.lock().expect("trace lock").clone()
        }
    }

    #[derive(Default)]
    struct TraceFields {
        values: Vec<String>,
    }

    impl TraceFields {
        fn push(&mut self, field: &Field, value: impl Into<String>) {
            self.values
                .push(format!("{}={}", field.name(), value.into()));
        }

        fn finish(self) -> String {
            self.values.join(" ")
        }
    }

    impl Visit for TraceFields {
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.push(field, value.to_string());
        }

        fn record_i64(&mut self, field: &Field, value: i64) {
            self.push(field, value.to_string());
        }

        fn record_u64(&mut self, field: &Field, value: u64) {
            self.push(field, value.to_string());
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.push(field, value.to_string());
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.push(field, format!("{value:?}"));
        }
    }

    struct TraceLayer {
        lines: TraceLines,
    }

    impl<S> Layer<S> for TraceLayer
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span>,
    {
        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            if event.metadata().target() != FULL_RESPONSE_TARGET {
                return;
            }
            let mut fields = TraceFields::default();
            event.record(&mut fields);
            self.lines.push(fields.finish());
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Default, Eq)]
    struct TestRouter;

    impl Router for TestRouter {
        type CompletionFields = ChatCompFields;
        type RouterModelId = OpenRouterModelId;

        const BASE_URL: &str = "http://127.0.0.1:39181/v1";
        const COMPLETION_URL: &str = TEST_ROUTER_URL;
        const MODELS_URL: &str = "http://127.0.0.1:39181/v1/models";
        const ENDPOINTS_TAIL: &str = "endpoints";
        const API_KEY_NAME: &str = "PLOKE_TEST_ROUTER_API_KEY";
        const PROVIDERS_URL: &str = "http://127.0.0.1:39181/v1/providers";

        fn resolve_api_key() -> Result<String, ploke_llm::LlmError> {
            std::env::var(Self::API_KEY_NAME).map_err(LlmError::from)
        }
    }

    impl RouterCalibration for TestRouter {
        type Model = ploke_llm::ModelKey;
        type Provider = ploke_llm::ProviderKey;
        type Preferences = ();
        type Key = ();
    }

    #[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Default, Eq)]
    struct TestRouterAlt;

    impl Router for TestRouterAlt {
        type CompletionFields = ChatCompFields;
        type RouterModelId = OpenRouterModelId;

        const BASE_URL: &str = "http://127.0.0.1:39182/v1";
        const COMPLETION_URL: &str = TEST_ROUTER_URL_ALT;
        const MODELS_URL: &str = "http://127.0.0.1:39182/v1/models";
        const ENDPOINTS_TAIL: &str = "endpoints";
        const API_KEY_NAME: &str = "PLOKE_TEST_ROUTER_API_KEY";
        const PROVIDERS_URL: &str = "http://127.0.0.1:39182/v1/providers";

        fn resolve_api_key() -> Result<String, ploke_llm::LlmError> {
            std::env::var(Self::API_KEY_NAME).map_err(LlmError::from)
        }
    }

    impl RouterCalibration for TestRouterAlt {
        type Model = ploke_llm::ModelKey;
        type Provider = ploke_llm::ProviderKey;
        type Preferences = ();
        type Key = ();
    }

    #[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Default, Eq)]
    struct CalibratedTestRouter;

    impl Router for CalibratedTestRouter {
        type CompletionFields = ChatCompFields;
        type RouterModelId = OpenRouterModelId;

        const BASE_URL: &str = "http://127.0.0.1:39181/v1";
        const COMPLETION_URL: &str = TEST_ROUTER_URL;
        const MODELS_URL: &str = "http://127.0.0.1:39181/v1/models";
        const ENDPOINTS_TAIL: &str = "endpoints";
        const API_KEY_NAME: &str = "PLOKE_TEST_ROUTER_API_KEY";
        const PROVIDERS_URL: &str = "http://127.0.0.1:39181/v1/providers";

        fn resolve_api_key() -> Result<String, ploke_llm::LlmError> {
            std::env::var(Self::API_KEY_NAME).map_err(LlmError::from)
        }
    }

    impl RouterCalibration for CalibratedTestRouter {
        type Model = ploke_llm::ModelKey;
        type Provider = ploke_llm::ProviderKey;
        type Preferences = ();
        type Key = ploke_llm::ModelKey;

        fn calibration_input(req: &ChatCompRequest<Self>) -> ploke_llm::CalibrationInput<Self> {
            ploke_llm::CalibrationInput {
                model: req.model_key.clone(),
                provider: None,
                provider_preferences: None,
                key: req.model_key.clone(),
            }
        }

        fn resolve_provider_timing(_input: ploke_llm::CalibrationInput<Self>) -> ProviderTiming {
            ProviderTiming {
                attempt_timeout: AttemptTimeout::fixed(Duration::from_secs(17)),
                max_attempts: 3,
                ..ProviderTiming::default()
            }
        }
    }

    struct ApiKeyGuard {
        previous: Option<String>,
    }

    impl ApiKeyGuard {
        fn set(key: &str) -> Self {
            let previous = std::env::var(TestRouter::API_KEY_NAME).ok();
            unsafe {
                std::env::set_var(TestRouter::API_KEY_NAME, key);
            }
            Self { previous }
        }
    }

    impl Drop for ApiKeyGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                unsafe {
                    std::env::set_var(TestRouter::API_KEY_NAME, previous);
                }
            } else {
                unsafe {
                    std::env::remove_var(TestRouter::API_KEY_NAME);
                }
            }
        }
    }

    async fn read_http_request(stream: &mut TcpStream) {
        let mut buf = Vec::new();
        let mut chunk = [0_u8; 4096];
        let mut header_end = None;
        let mut content_length = 0usize;

        loop {
            let read = stream.read(&mut chunk).await.expect("read request");
            if read == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..read]);

            if header_end.is_none()
                && let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n")
            {
                let end = pos + 4;
                header_end = Some(end);
                let headers = String::from_utf8_lossy(&buf[..end]);
                content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        if name.eq_ignore_ascii_case("content-length") {
                            value.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
            }

            if let Some(end) = header_end
                && buf.len() >= end + content_length
            {
                break;
            }
        }
    }

    async fn spawn_test_router_server(
        bind_addr: &'static str,
        responses: Vec<String>,
        request_count: std::sync::Arc<AtomicUsize>,
    ) -> tokio::task::JoinHandle<()> {
        let listener = TcpListener::bind(bind_addr)
            .await
            .expect("bind test router");
        tokio::spawn(async move {
            for body in responses {
                let Ok(Ok((mut stream, _))) =
                    timeout(Duration::from_secs(5), listener.accept()).await
                else {
                    break;
                };
                request_count.fetch_add(1, Ordering::SeqCst);
                read_http_request(&mut stream).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write response");
                stream.shutdown().await.expect("shutdown");
            }
        })
    }

    async fn spawn_nonresponding_test_router_server(
        bind_addr: &'static str,
        request_count: std::sync::Arc<AtomicUsize>,
    ) -> (
        tokio::sync::oneshot::Sender<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind(bind_addr)
            .await
            .expect("bind test router");
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    accepted = listener.accept() => {
                        let Ok((mut stream, _)) = accepted else {
                            break;
                        };
                        request_count.fetch_add(1, Ordering::SeqCst);
                        tokio::spawn(async move {
                            read_http_request(&mut stream).await;
                            tokio::time::sleep(Duration::from_secs(10)).await;
                            let _ = stream.shutdown().await;
                        });
                    }
                }
            }
        });
        (stop_tx, handle)
    }

    fn malformed_tool_call_response(index: usize) -> String {
        json!({
            "id": format!("repair-{index}"),
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": format!("call_{index}"),
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"file\":1}"
                        }
                    }]
                }
            }],
            "created": 0,
            "model": "test/model",
            "object": "chat.completion"
        })
        .to_string()
    }

    fn content_response(content: &str) -> String {
        json!({
            "id": "final",
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
        })
        .to_string()
    }

    #[test]
    fn full_response_trace_record_serializes_recorded_response_in_sidecar_shape() {
        let assistant_message_id = Uuid::from_u128(0x8e32b33b_6de5_4e1c_9fa1_14bc2059913f);
        let response = serde_json::from_str(&content_response("final answer"))
            .expect("content response envelope");
        let record = FullResponseTraceRecord {
            assistant_message_id,
            recorded_response: RecordedResponse::new(3, response),
        };

        let value = serde_json::to_value(&record).expect("trace record json");

        assert_eq!(
            value["assistant_message_id"],
            assistant_message_id.to_string()
        );
        assert_eq!(value["response_index"], 3);
        assert_eq!(value["response"]["id"], "final");
        assert!(value.get("recorded_response").is_none());
    }

    async fn run_calibrated_test_router_session() -> ChatSessionReport {
        let responses = vec![content_response("final answer")];
        let request_count = std::sync::Arc::new(AtomicUsize::new(0));
        let server =
            spawn_test_router_server("127.0.0.1:39181", responses, request_count.clone()).await;

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let drain = tokio::spawn(async move { while state_cmd_rx.recv().await.is_some() {} });
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let req = ChatCompRequest::<CalibratedTestRouter>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::live(),
                parent_id: Uuid::new_v4(),
                assistant_message_id: Uuid::new_v4(),
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            90,
        )
        .await;

        server.await.expect("server task");
        drain.abort();
        assert_eq!(
            request_count.load(Ordering::SeqCst),
            1,
            "report={report:#?}"
        );
        report
    }

    #[tokio::test]
    async fn run_chat_session_replays_recorded_response_without_provider_http() {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let assistant_message_id = Uuid::new_v4();
        let req = ChatCompRequest::<TestRouter>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);
        let response = serde_json::from_str(&content_response("recorded final answer"))
            .expect("recorded response parses");
        let tape = RecordedResponseTape::new(vec![RecordedResponse::new(0, response)]);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::recorded(tape),
                parent_id: Uuid::new_v4(),
                assistant_message_id,
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            1,
        )
        .await;

        let mut assistant_update = None;
        while let Ok(command) = state_cmd_rx.try_recv() {
            if let StateCommand::UpdateMessage { id, update } = command
                && id == assistant_message_id
                && let Some(content) = update.content
            {
                assistant_update = Some(content);
            }
        }

        assert!(matches!(report.outcome, SessionOutcome::Completed));
        assert_eq!(report.errors.len(), 0);
        assert_eq!(report.attempts, 1);
        assert!(
            report.chat_steps.is_empty(),
            "recorded replay should not emit provider HTTP attempts"
        );
        assert_eq!(assistant_update.as_deref(), Some("recorded final answer"));
    }

    #[tokio::test]
    async fn run_chat_session_replays_recorded_tool_arg_repair_without_provider_http() {
        let trace_lines = TraceLines::default();
        let subscriber = Registry::default().with(TraceLayer {
            lines: trace_lines.clone(),
        });
        let trace_guard = tracing::subscriber::set_default(subscriber);

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let assistant_message_id = Uuid::new_v4();
        let req = ChatCompRequest::<TestRouter>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);
        let malformed_response = serde_json::from_str(&malformed_tool_call_response(1))
            .expect("malformed tool response envelope still parses as provider response");
        let final_response = serde_json::from_str(&content_response("recovered after repair"))
            .expect("recorded final response parses");
        let tape = RecordedResponseTape::new(vec![
            RecordedResponse::new(0, malformed_response),
            RecordedResponse::new(1, final_response),
        ]);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::recorded(tape),
                parent_id: Uuid::new_v4(),
                assistant_message_id,
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            2,
        )
        .await;
        drop(trace_guard);

        let mut assistant_updates = Vec::new();
        while let Ok(command) = state_cmd_rx.try_recv() {
            match command {
                StateCommand::UpdateMessage { id, update } if id == assistant_message_id => {
                    if let Some(content) = update.content {
                        assistant_updates.push(content);
                    }
                }
                StateCommand::AddMessageImmediate {
                    msg,
                    kind: MessageKind::Assistant,
                    ..
                } => assistant_updates.push(msg),
                _ => {}
            }
        }

        assert!(matches!(report.outcome, SessionOutcome::Completed));
        assert_eq!(report.attempts, 2);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].code.as_ref(), "TOOL_ARGS_REPAIR_REQUIRED");
        assert!(
            report.chat_steps.is_empty(),
            "recorded replay should not emit provider HTTP attempts"
        );
        assert!(
            assistant_updates
                .iter()
                .any(|content| content.contains("recovered after repair")),
            "expected final assistant update after recorded repair, got {assistant_updates:?}"
        );
        let traces = trace_lines.snapshot();
        assert_eq!(
            traces.len(),
            2,
            "recorded repair replay should trace both provider envelopes, got {traces:?}"
        );
        assert!(
            traces
                .iter()
                .any(|line| line.contains("\"id\":\"repair-1\"")),
            "expected malformed tool-call provider envelope in full-response trace, got {traces:?}"
        );
        assert!(
            traces.iter().any(|line| line.contains("\"id\":\"final\"")),
            "expected final provider envelope in full-response trace, got {traces:?}"
        );
    }

    #[tokio::test]
    async fn run_chat_session_stops_cleanly_when_recorded_tape_exhausted() {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let assistant_message_id = Uuid::new_v4();
        let req = ChatCompRequest::<TestRouter>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);
        let malformed_response = serde_json::from_str(&malformed_tool_call_response(1))
            .expect("malformed tool response envelope still parses as provider response");
        let tape = RecordedResponseTape::new(vec![RecordedResponse::new(0, malformed_response)]);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::recorded(tape),
                parent_id: Uuid::new_v4(),
                assistant_message_id,
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            2,
        )
        .await;

        let mut assistant_updates = Vec::new();
        while let Ok(command) = state_cmd_rx.try_recv() {
            match command {
                StateCommand::UpdateMessage { id, update } if id == assistant_message_id => {
                    if let Some(content) = update.content {
                        assistant_updates.push(content);
                    }
                }
                StateCommand::AddMessageImmediate {
                    msg,
                    kind: MessageKind::Assistant,
                    ..
                } => assistant_updates.push(msg),
                _ => {}
            }
        }

        assert!(matches!(report.outcome, SessionOutcome::Completed));
        assert_eq!(report.attempts, 2);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].code.as_ref(), "TOOL_ARGS_REPAIR_REQUIRED");
        assert!(
            report.chat_steps.is_empty(),
            "recorded replay should not emit provider HTTP attempts"
        );
        assert!(
            assistant_updates
                .iter()
                .all(|content| !content.contains("INVALID_MODEL_RESPONSE")),
            "recorded tape exhaustion should not surface as invalid model output: {assistant_updates:?}"
        );
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_google_chat_session_executes_list_dir_tool_call_success_or_quota() {
        let db = Arc::new(Database::new_init().expect("database initializes"));
        let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            EmbeddingProcessor::new(EmbeddingSource::Local(
                LocalEmbedder::new(EmbeddingConfig::default()).expect("local embedder initializes"),
            )),
        ));
        let rag = Arc::new(
            RagService::new(Arc::clone(&db), Arc::clone(&embedder))
                .expect("rag service initializes"),
        );
        let (rag_tx, _rag_rx) = mpsc::channel(16);
        let state = Arc::new(AppState::new(
            db,
            embedder,
            ploke_io::IoManagerHandle::new(),
            rag,
            TokenBudget::default(),
            rag_tx,
        ));
        let workspace_root = std::env::current_dir().expect("current dir is available");
        state
            .with_system_txn(|txn| {
                txn.set_loaded_workspace(
                    workspace_root.clone(),
                    vec![workspace_root.clone()],
                    Some(workspace_root),
                );
            })
            .await;

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let mut tool_rx = event_bus.subscribe(crate::EventPriority::Realtime);
        let requested_tools = Arc::new(AtomicUsize::new(0));
        let completed_tools = Arc::new(AtomicUsize::new(0));
        let tool_state = Arc::clone(&state);
        let tool_event_bus = Arc::clone(&event_bus);
        let requested_tools_for_task = Arc::clone(&requested_tools);
        let completed_tools_for_task = Arc::clone(&completed_tools);
        let tool_dispatcher = tokio::spawn(async move {
            while let Ok(event) = tool_rx.recv().await {
                if let AppEvent::System(SystemEvent::ToolCallRequested {
                    tool_call,
                    request_id,
                    parent_id,
                }) = event
                {
                    requested_tools_for_task.fetch_add(1, Ordering::SeqCst);
                    let ctx = crate::tools::Ctx {
                        state: Arc::clone(&tool_state),
                        event_bus: Arc::clone(&tool_event_bus),
                        request_id,
                        parent_id,
                        call_id: tool_call.call_id.clone(),
                    };
                    if crate::tools::process_tool(tool_call, ctx).await.is_ok() {
                        completed_tools_for_task.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        });

        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let drain = tokio::spawn(async move { while state_cmd_rx.recv().await.is_some() {} });
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let model = std::env::var("PLOKE_LIVE_GOOGLE_CHAT_MODEL")
            .unwrap_or_else(|_| "google/gemini-2.5-flash".to_string());
        let req = ChatCompRequest::<Google>::default()
            .with_model_str(&model)
            .expect("Google model id parses")
            .with_message(RequestMessage::new_user(
                "Call the list_dir tool exactly once with dir \".\" and max_entries 3. After the tool result, reply with the word listed."
                    .to_string(),
            ))
            .with_max_tokens(160)
            .with_temperature(0.0)
            .with_tools(Some(vec![crate::tools::list_dir::ListDir::tool_def()]))
            .with_tool_choice(Some(ToolChoice::Auto));

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::live(),
                parent_id: Uuid::new_v4(),
                assistant_message_id: Uuid::new_v4(),
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            90,
        )
        .await;

        tool_dispatcher.abort();
        drain.abort();

        assert!(
            matches!(report.outcome, SessionOutcome::Completed),
            "expected completed Google TUI session, got {report:#?}"
        );
        assert!(
            report.errors.is_empty(),
            "Google TUI session recorded errors: {:#?}",
            report.errors
        );
        assert_eq!(
            requested_tools.load(Ordering::SeqCst),
            1,
            "expected exactly one list_dir request, report={report:#?}"
        );
        assert_eq!(
            completed_tools.load(Ordering::SeqCst),
            1,
            "expected exactly one completed list_dir execution, report={report:#?}"
        );
        assert_eq!(
            report.chat_steps.len(),
            2,
            "tool session should include Google tool-call and final-response steps: {report:#?}"
        );
    }

    #[tokio::test]
    async fn run_chat_session_can_replay_recorded_prefix_then_continue_live() {
        let _router_guard = TEST_ROUTER_LOCK.lock().await;
        let _api_key = ApiKeyGuard::set("test-key");
        let responses = vec![content_response("live tail after recorded prefix")];
        let request_count = std::sync::Arc::new(AtomicUsize::new(0));
        let server =
            spawn_test_router_server("127.0.0.1:39181", responses, request_count.clone()).await;

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let assistant_message_id = Uuid::new_v4();
        let req = ChatCompRequest::<TestRouter>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);
        let malformed_response = serde_json::from_str(&malformed_tool_call_response(1))
            .expect("malformed tool response envelope still parses as provider response");
        let tape = RecordedResponseTape::new(vec![RecordedResponse::new(0, malformed_response)]);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::recorded_prefix_then_live(tape),
                parent_id: Uuid::new_v4(),
                assistant_message_id,
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            2,
        )
        .await;

        server.await.expect("server task");
        let mut assistant_updates = Vec::new();
        while let Ok(command) = state_cmd_rx.try_recv() {
            match command {
                StateCommand::UpdateMessage { id, update } if id == assistant_message_id => {
                    if let Some(content) = update.content {
                        assistant_updates.push(content);
                    }
                }
                StateCommand::AddMessageImmediate {
                    msg,
                    kind: MessageKind::Assistant,
                    ..
                } => assistant_updates.push(msg),
                _ => {}
            }
        }

        assert!(matches!(report.outcome, SessionOutcome::Completed));
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].code.as_ref(), "TOOL_ARGS_REPAIR_REQUIRED");
        assert_eq!(report.attempts, 2);
        assert_eq!(
            request_count.load(Ordering::SeqCst),
            1,
            "only the live tail should reach the provider"
        );
        assert!(
            assistant_updates
                .iter()
                .any(|content| content.contains("live tail after recorded prefix")),
            "expected live-tail assistant update, got {assistant_updates:?}"
        );
    }

    #[tokio::test]
    async fn run_chat_session_can_replay_prefix_then_take_one_live_step() {
        let _router_guard = TEST_ROUTER_LOCK.lock().await;
        let _api_key = ApiKeyGuard::set("test-key");
        let responses = vec![malformed_tool_call_response(2)];
        let request_count = std::sync::Arc::new(AtomicUsize::new(0));
        let server =
            spawn_test_router_server("127.0.0.1:39181", responses, request_count.clone()).await;

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, _state_cmd_rx) = mpsc::channel(128);
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let assistant_message_id = Uuid::new_v4();
        let req = ChatCompRequest::<TestRouter>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);
        let recorded_response = serde_json::from_str(&malformed_tool_call_response(1))
            .expect("malformed tool response envelope still parses as provider response");
        let tape = RecordedResponseTape::new(vec![RecordedResponse::new(0, recorded_response)]);
        let (request_tx, request_rx) = std::sync::mpsc::channel();
        let _request_tap = install_request_tap(request_tx);
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        let _response_tap = install_response_tap(response_tx);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::recorded_prefix_then_live_steps(tape, 1),
                parent_id: Uuid::new_v4(),
                assistant_message_id,
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            2,
        )
        .await;

        server.await.expect("server task");
        let captured_requests = request_rx.try_iter().collect::<Vec<_>>();
        let captured_responses = response_rx.try_iter().collect::<Vec<_>>();

        assert!(matches!(report.outcome, SessionOutcome::Completed));
        assert_eq!(
            request_count.load(Ordering::SeqCst),
            1,
            "exactly one live step should reach the provider"
        );
        assert_eq!(
            captured_requests.len(),
            3,
            "expected recorded request, live request, then step-boundary request"
        );
        assert_eq!(
            captured_responses.len(),
            2,
            "expected recorded and one live provider response"
        );
        assert_eq!(captured_responses[0].index(), 0);
        assert_eq!(captured_responses[1].index(), 1);
    }

    #[test]
    fn compact_tool_content_for_llm_replay_truncates_file_payloads_to_configured_limit() {
        let file_text = (1..=250)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let payload = json!({
            "ok": true,
            "file_path": "/tmp/example.rs",
            "exists": true,
            "byte_len": file_text.len(),
            "truncated": false,
            "content": file_text,
        })
        .to_string();

        let replay = compact_tool_content_for_llm_replay(&payload, 200);
        let replay_json: serde_json::Value =
            serde_json::from_str(&replay).expect("replay payload must be json");
        let replay_content = replay_json
            .get("content")
            .and_then(serde_json::Value::as_str)
            .expect("replay content must be string");

        assert!(replay_json["llm_replay_truncated"].as_bool() == Some(true));
        assert_eq!(replay_json["llm_replay_max_lines"].as_u64(), Some(200));
        assert!(replay_content.contains("line 1"));
        assert!(replay_content.contains("line 200"));
        assert!(!replay_content.contains("line 201"));
        assert!(replay_content.contains("truncated for LLM replay after 200 lines"));
    }

    #[test]
    fn compact_tool_content_for_llm_replay_leaves_non_file_payloads_unchanged() {
        let payload = json!({
            "ok": true,
            "search_term": "fn build",
            "context": [
                {"file_path": "/tmp/example.rs", "snippet": "fn build() {}"}
            ]
        })
        .to_string();

        assert_eq!(compact_tool_content_for_llm_replay(&payload, 200), payload);
    }

    #[test]
    fn compact_tool_content_for_llm_replay_respects_custom_line_limit() {
        let file_text = (1..=20)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let payload = json!({
            "ok": true,
            "file_path": "/tmp/example.rs",
            "exists": true,
            "byte_len": file_text.len(),
            "truncated": false,
            "content": file_text,
        })
        .to_string();

        let replay = compact_tool_content_for_llm_replay(&payload, 7);
        let replay_json: serde_json::Value =
            serde_json::from_str(&replay).expect("replay payload must be json");
        let replay_content = replay_json["content"]
            .as_str()
            .expect("replay content must be string");

        assert_eq!(replay_json["llm_replay_max_lines"].as_u64(), Some(7));
        assert!(replay_content.contains("line 7"));
        assert!(!replay_content.contains("line 8"));
    }

    #[derive(Debug, Deserialize)]
    struct CapturedChatRequest {
        messages: Vec<RequestMessage>,
    }

    fn summarize_tool_payload(content: &str) -> String {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
            return "non-json tool payload".to_string();
        };
        let Some(obj) = value.as_object() else {
            return format!("json {}", value_type_name(&value));
        };

        if obj.contains_key("file_path") && obj.contains_key("content") {
            let file_path = obj
                .get("file_path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            let byte_len = obj
                .get("byte_len")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let content_chars = obj
                .get("content")
                .and_then(serde_json::Value::as_str)
                .map(str::chars)
                .map(Iterator::count)
                .unwrap_or(0);
            let truncated = obj
                .get("truncated")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            return format!(
                "file payload path={file_path} byte_len={byte_len} content_chars={content_chars} truncated={truncated}"
            );
        }

        if let Some(context) = obj.get("context").and_then(serde_json::Value::as_array) {
            let snippet_chars: usize = context
                .iter()
                .filter_map(|entry| entry.get("snippet"))
                .filter_map(serde_json::Value::as_str)
                .map(|snippet| snippet.chars().count())
                .sum();
            let search_term = obj
                .get("search_term")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<none>");
            return format!(
                "context payload search_term={search_term:?} entries={} snippet_chars={snippet_chars}",
                context.len()
            );
        }

        if let Some(error) = obj.get("error") {
            let preview = error
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| error.to_string());
            return format!("error payload chars={}", preview.chars().count());
        }

        let keys = obj.keys().cloned().collect::<Vec<_>>().join(", ");
        format!("json object keys=[{keys}]")
    }

    fn value_type_name(value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "bool",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    #[test]
    #[ignore = "diagnostic: inspect captured large chat request payload"]
    fn diagnostic_dump_captured_request_blowup() {
        let path = std::env::var("PLOKE_DIAG_REQUEST_PATH")
            .unwrap_or_else(|_| "/tmp/request16.json".to_string());
        let raw = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("failed to read diagnostic request fixture {path}: {err}")
        });
        let request: CapturedChatRequest = serde_json::from_str(&raw).unwrap_or_else(|err| {
            panic!("failed to parse diagnostic request fixture {path}: {err}")
        });

        let tokenizer = ApproxCharTokenizer::default();
        let mut role_counts = BTreeMap::<&'static str, usize>::new();
        let mut role_chars = BTreeMap::<&'static str, usize>::new();
        let mut indexed = Vec::new();

        for (idx, message) in request.messages.iter().enumerate() {
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
                Role::Tool => "tool",
            };
            let chars = message.content.chars().count();
            *role_counts.entry(role).or_default() += 1;
            *role_chars.entry(role).or_default() += chars;
            indexed.push((chars, idx, role, message));
        }

        indexed.sort_by(|a, b| b.0.cmp(&a.0));

        let total_chars: usize = request
            .messages
            .iter()
            .map(|message| message.content.chars().count())
            .sum();
        let total_tokens: usize = request
            .messages
            .iter()
            .map(|message| tokenizer.count(&message.content))
            .sum();

        println!("diagnostic request path: {path}");
        println!("message_count: {}", request.messages.len());
        println!("total_chars: {total_chars}");
        println!("estimated_tokens: {total_tokens}");
        println!("role_counts: {role_counts:?}");
        println!("role_chars: {role_chars:?}");
        println!();
        println!("top contributors:");

        for (chars, idx, role, message) in indexed.into_iter().take(15) {
            let detail = if message.role == Role::Tool {
                summarize_tool_payload(&message.content)
            } else {
                let preview = message
                    .content
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(120)
                    .collect::<String>();
                format!("preview={preview:?}")
            };
            println!("[{idx}] role={role} chars={chars} {detail}");
        }
    }

    #[tokio::test]
    async fn run_chat_session_can_converge_after_four_tool_arg_repairs() {
        let _guard = TEST_ROUTER_LOCK.lock().await;
        let _api_key = ApiKeyGuard::set("test-key");

        let responses = vec![
            malformed_tool_call_response(1),
            malformed_tool_call_response(2),
            malformed_tool_call_response(3),
            malformed_tool_call_response(4),
            content_response("final answer"),
        ];
        let request_count = std::sync::Arc::new(AtomicUsize::new(0));
        let server =
            spawn_test_router_server("127.0.0.1:39181", responses, request_count.clone()).await;

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let drain = tokio::spawn(async move { while state_cmd_rx.recv().await.is_some() {} });
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);

        let req = ChatCompRequest::<TestRouter>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);
        assert_eq!(TestRouter::COMPLETION_URL, TEST_ROUTER_URL);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::live(),
                parent_id: Uuid::new_v4(),
                assistant_message_id: Uuid::new_v4(),
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            5,
        )
        .await;

        server.await.expect("server task");
        drain.abort();

        assert!(matches!(report.outcome, SessionOutcome::Completed));
        assert_eq!(report.attempts, 5);
        assert_eq!(request_count.load(Ordering::SeqCst), 5);
        assert_eq!(report.errors.len(), 4);
        assert!(
            report
                .errors
                .iter()
                .all(|error| { error.code.as_ref() == "TOOL_ARGS_REPAIR_REQUIRED" })
        );
        assert!(
            report
                .errors
                .iter()
                .all(|error| error.code.as_ref() != "REPAIR_BUDGET_EXHAUSTED")
        );
    }

    #[tokio::test]
    async fn run_chat_session_uses_static_session_timeout_for_provider_timing() {
        let _guard = TEST_ROUTER_LOCK.lock().await;
        let _api_key = ApiKeyGuard::set("test-key");

        let report = run_calibrated_test_router_session().await;

        assert!(matches!(report.outcome, SessionOutcome::Completed));
        assert_eq!(report.attempts, 1);
        let step = report.chat_steps.first().expect("chat step report");
        assert_eq!(
            step.provider_timing.first_timeout(),
            Duration::from_secs(90)
        );
        assert_eq!(
            step.provider_timing.attempt_timeout.for_attempt(2),
            Duration::from_secs(90)
        );
        // The per-attempt timeout is statically overridden to the session
        // timeout, but the router-calibrated attempt budget is now honored
        // (CalibratedTestRouter requests 3, above the floor of 2).
        assert_eq!(step.provider_timing.max_attempts, 3);
    }

    #[tokio::test]
    async fn provider_retry_exhaustion_does_not_outer_retry_chat_step() {
        let _guard = TEST_ROUTER_LOCK.lock().await;
        let _api_key = ApiKeyGuard::set("test-key");
        let request_count = std::sync::Arc::new(AtomicUsize::new(0));
        let (stop_server, server) =
            spawn_nonresponding_test_router_server("127.0.0.1:39181", request_count.clone()).await;

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let drain = tokio::spawn(async move { while state_cmd_rx.recv().await.is_some() {} });
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let req = ChatCompRequest::<TestRouter>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);
        let chat_policy = ChatPolicy {
            error_retry_limit: 2,
            timeout_base_secs: 1,
            ..ChatPolicy::default()
        };

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::live(),
                parent_id: Uuid::new_v4(),
                assistant_message_id: Uuid::new_v4(),
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy,
                cancel_rx,
            },
            1,
        )
        .await;

        let _ = stop_server.send(());
        server.await.expect("server task");
        drain.abort();

        assert!(matches!(report.outcome, SessionOutcome::Aborted { .. }));
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.chat_steps.len(), 1);
        let step = report.chat_steps.first().expect("chat step report");
        assert_eq!(step.provider_attempts.len(), 2);
        assert_eq!(
            step.provider_attempts
                .last()
                .map(|attempt| attempt.retry_decision),
            Some(ProviderRetryDecision::Exhausted)
        );
        assert_eq!(request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    #[ignore = "live OpenRouter API smoke test; requires OPENROUTER_API_KEY"]
    async fn live_openrouter_agent_turn_records_provider_attempt_report() {
        if std::env::var(OpenRouter::API_KEY_NAME)
            .ok()
            .filter(|key| !key.trim().is_empty())
            .is_none()
        {
            eprintln!(
                "skipping live_openrouter_agent_turn_records_provider_attempt_report: {} not set",
                OpenRouter::API_KEY_NAME
            );
            return;
        }

        let model = std::env::var("PLOKE_LIVE_AGENT_MODEL")
            .ok()
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| "x-ai/grok-4-fast".to_string());
        let provider = std::env::var("PLOKE_LIVE_AGENT_PROVIDER")
            .ok()
            .filter(|provider| !provider.trim().is_empty())
            .unwrap_or_else(|| "xai".to_string());
        let provider_slug = ProviderSlug::new(&provider);
        let provider_preferences = ProviderPreferences::default()
            .with_order([provider_slug.clone()])
            .with_only([provider_slug])
            .with_allow_fallbacks(false);
        let assistant_message_id = Uuid::new_v4();
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let req = OpenRouter::default_chat_completion()
            .with_model_str(&model)
            .expect("live model id")
            .with_router_bundle(ChatCompFields::default().with_provider(provider_preferences))
            .with_messages(vec![
                RequestMessage::new_system(
                    "You are a live API smoke test. Reply exactly as requested.".to_string(),
                ),
                RequestMessage::new_user(
                    "Reply with exactly: ploke-live-agent-turn-ok".to_string(),
                ),
            ])
            .with_temperature(0.0)
            .with_max_tokens(32);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::live(),
                parent_id: Uuid::new_v4(),
                assistant_message_id,
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            45,
        )
        .await;

        let mut assistant_update = None;
        while let Ok(command) = state_cmd_rx.try_recv() {
            if let StateCommand::UpdateMessage { id, update } = command
                && id == assistant_message_id
                && let Some(content) = update.content
            {
                assistant_update = Some(content);
            }
        }

        assert!(matches!(report.outcome, SessionOutcome::Completed));
        assert_eq!(report.errors.len(), 0);
        assert_eq!(report.attempts, 1);
        assert!(
            report
                .chat_steps
                .iter()
                .flat_map(|step| &step.provider_attempts)
                .any(|attempt| attempt
                    .status
                    .is_some_and(|status| (200..300).contains(&status))),
            "expected at least one successful provider attempt, report={report:#?}"
        );
        let assistant_update = assistant_update.expect("assistant message should be updated");
        assert!(
            assistant_update.contains("ploke-live-agent-turn-ok"),
            "unexpected assistant content: {assistant_update:?}"
        );

        let step = report.chat_steps.first().expect("chat step report");
        assert_eq!(step.provider_timing.max_attempts, 2);
        assert_eq!(
            step.provider_timing.first_timeout(),
            Duration::from_secs(45)
        );
    }

    #[tokio::test]
    async fn run_chat_session_aborts_when_a_fifth_repair_would_be_required() {
        let _guard = TEST_ROUTER_LOCK.lock().await;
        let _api_key = ApiKeyGuard::set("test-key");

        let responses = vec![
            malformed_tool_call_response(1),
            malformed_tool_call_response(2),
            malformed_tool_call_response(3),
            malformed_tool_call_response(4),
            malformed_tool_call_response(5),
            content_response("would have recovered"),
        ];
        let request_count = std::sync::Arc::new(AtomicUsize::new(0));
        let server =
            spawn_test_router_server("127.0.0.1:39182", responses, request_count.clone()).await;

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (state_cmd_tx, mut state_cmd_rx) = mpsc::channel(128);
        let drain = tokio::spawn(async move { while state_cmd_rx.recv().await.is_some() {} });
        let (_cancel_tx, cancel_rx) = watch::channel(CancelChatToken::KeepOpen);

        let req = ChatCompRequest::<TestRouterAlt>::default()
            .with_model_str("moonshotai/kimi-k2")
            .expect("model id")
            .with_messages(vec![RequestMessage::new_system(
                "You are a test assistant.".to_string(),
            )]);

        let report = run_chat_session(
            ChatSession {
                client: Client::new(),
                req,
                chat_step_source: ChatStepSource::live(),
                parent_id: Uuid::new_v4(),
                assistant_message_id: Uuid::new_v4(),
                event_bus,
                state_cmd_tx,
                included_message_ids: Vec::new(),
                chat_policy: ChatPolicy::default(),
                cancel_rx,
            },
            5,
        )
        .await;

        server.await.expect("server task");
        drain.await.expect("drain task");

        assert!(matches!(report.outcome, SessionOutcome::Aborted { .. }));
        assert_eq!(report.attempts, 5);
        assert_eq!(request_count.load(Ordering::SeqCst), 5);
        assert_eq!(report.errors.len(), 5);
        assert_eq!(
            report.errors.last().map(|error| error.code.as_ref()),
            Some("REPAIR_BUDGET_EXHAUSTED")
        );
    }

    #[test]
    fn parse_chat_outcome_content_message() {
        let body = r#"{
            "choices": [
                { "message": {"role": "assistant", "content": "Hello world"} }
            ]
        }"#;
        let r = parse_chat_outcome(body).unwrap();
        match r.outcome {
            ChatStepOutcome::Content {
                content: Some(c), ..
            } => assert_eq!(c.as_ref(), "Hello world"),
            _ => panic!("expected content"),
        }
    }

    #[test]
    fn parse_chat_outcome_text_field() {
        let body = r#"{
            "choices": [
                { "text": "Hello text" }
            ]
        }"#;
        let r = parse_chat_outcome(body).unwrap();
        match r.outcome {
            ChatStepOutcome::Content {
                content: Some(c), ..
            } => assert_eq!(c.as_ref(), "Hello text"),
            _ => panic!("expected content"),
        }
    }

    #[test]
    fn timeout_policy_fixed_retry_respects_limit() {
        let policy = TuiTimeoutPolicy {
            duration: Some(Duration::from_secs(10)),
            strategy: TimeoutStrategy::FixedRetry(2),
        };

        assert_eq!(policy.next_timout_dur(1), Some(Duration::from_secs(10)));
        assert_eq!(policy.next_timout_dur(2), Some(Duration::from_secs(10)));
        assert_eq!(policy.next_timout_dur(3), None);
    }

    #[test]
    fn timeout_policy_backoff_doubles() {
        let policy = TuiTimeoutPolicy {
            duration: Some(Duration::from_secs(5)),
            strategy: TimeoutStrategy::Backoff(Some(3)),
        };

        assert_eq!(policy.next_timout_dur(1), Some(Duration::from_secs(10)));
        assert_eq!(policy.next_timout_dur(2), Some(Duration::from_secs(20)));
        assert_eq!(policy.next_timout_dur(3), Some(Duration::from_secs(40)));
        assert_eq!(policy.next_timout_dur(4), None);
    }

    #[test]
    fn error_policy_retry_limit_stops_after_limit() {
        let mut retries = 0_u32;

        assert!(should_retry_error(
            TuiErrorPolicy::RetryLimit(2),
            &mut retries
        ));
        assert_eq!(retries, 1);
        assert!(should_retry_error(
            TuiErrorPolicy::RetryLimit(2),
            &mut retries
        ));
        assert_eq!(retries, 2);
        assert!(!should_retry_error(
            TuiErrorPolicy::RetryLimit(2),
            &mut retries
        ));
        assert_eq!(retries, 2);
    }

    #[test]
    fn error_policy_strict_never_retries() {
        let mut retries = 0_u32;
        assert!(!should_retry_error(TuiErrorPolicy::Strict, &mut retries));
        assert_eq!(retries, 0);
    }

    #[tokio::test]
    async fn state_command_emit_after_state_manager_close_is_nonfatal() {
        let (cmd_tx, cmd_rx) = mpsc::channel(1);
        drop(cmd_rx);

        let emitted = send_state_command_or_warn(
            &cmd_tx,
            StateCommand::AddMessageImmediate {
                msg: "late teardown message".to_string(),
                kind: MessageKind::System,
                new_msg_id: Uuid::new_v4(),
            },
            "test_closed_state_manager",
        )
        .await;

        assert!(!emitted);
    }

    #[test]
    fn error_policy_endless_retry_always_retries() {
        let mut retries = 0_u32;
        assert!(should_retry_error(
            TuiErrorPolicy::EndlessRetry,
            &mut retries
        ));
        assert_eq!(retries, 1);
        assert!(should_retry_error(
            TuiErrorPolicy::EndlessRetry,
            &mut retries
        ));
        assert_eq!(retries, 2);
    }

    #[test]
    fn length_policy_retry_limit_stops_after_limit() {
        let mut retries = 0_u32;

        assert!(should_retry_length(
            TuiLengthPolicy::RetryLimit(1),
            &mut retries
        ));
        assert_eq!(retries, 1);
        assert!(!should_retry_length(
            TuiLengthPolicy::RetryLimit(1),
            &mut retries
        ));
        assert_eq!(retries, 1);
    }

    #[test]
    fn length_policy_strict_never_retries() {
        let mut retries = 0_u32;
        assert!(!should_retry_length(TuiLengthPolicy::Strict, &mut retries));
        assert_eq!(retries, 0);
    }

    #[test]
    fn build_preflight_tool_call_repair_error_marks_retryable() {
        let preflight_error = ToolCallPreflightError {
            call_id: ploke_core::ArcStr::from("call_preflight"),
            tool_name: ToolName::NsRead,
            rejected_arguments: "{\"file\":1}".to_string(),
            error: ToolError::new(
                ToolName::NsRead,
                ToolErrorCode::WrongType,
                "failed to parse tool arguments: EOF while parsing a value",
            ),
        };

        let context = base_error_context(1, 0, "tool_call_preflight", &None, Uuid::new_v4());
        let spec = semantics::normalize_tool_call_preflight_error(preflight_error, None, context);
        let loop_error = build_loop_error_from_semantic_spec(spec, CommitPhase::PreCommit);

        assert_eq!(loop_error.code.as_ref(), "TOOL_ARGS_REPAIR_REQUIRED");
        assert!(matches!(
            loop_error.recovery,
            RecoveryDecision::Repair {
                strategy: RetryStrategy::Fixed,
                ..
            }
        ));
        assert!(matches!(
            loop_error.retry,
            RetryAdvice::Yes {
                strategy: RetryStrategy::Fixed,
                ..
            }
        ));
        assert_eq!(loop_error.context.tool_name.as_deref(), Some("read_file"));
        assert_eq!(
            loop_error
                .llm_action
                .as_ref()
                .and_then(|action| action.next_steps.first())
                .map(|step| step.action.as_ref()),
            Some("repair_tool_args")
        );
    }

    #[test]
    fn repair_budget_is_bounded_locally() {
        let mut state = ChatLoopState::default();
        for _ in 0..DEFAULT_REPAIR_ATTEMPTS_PER_SESSION {
            assert!(!repair_budget_exhausted(
                &state,
                DEFAULT_REPAIR_ATTEMPTS_PER_SESSION
            ));
            state.repair_attempts = state.repair_attempts.saturating_add(1);
        }
        assert!(repair_budget_exhausted(
            &state,
            DEFAULT_REPAIR_ATTEMPTS_PER_SESSION
        ));
    }

    #[test]
    fn consume_repair_budget_marks_error_exhausted_after_limit() {
        let mut state = ChatLoopState::default();

        for _ in 0..DEFAULT_REPAIR_ATTEMPTS_PER_SESSION {
            let preflight_error = ToolCallPreflightError {
                call_id: ploke_core::ArcStr::from("call_preflight"),
                tool_name: ToolName::NsRead,
                rejected_arguments: "{\"file\":1}".to_string(),
                error: ToolError::new(
                    ToolName::NsRead,
                    ToolErrorCode::WrongType,
                    "failed to parse tool arguments: EOF while parsing a value",
                ),
            };

            let context = base_error_context(1, 0, "tool_call_preflight", &None, Uuid::new_v4());
            let spec =
                semantics::normalize_tool_call_preflight_error(preflight_error, None, context);
            let mut loop_error = build_loop_error_from_semantic_spec(spec, CommitPhase::PreCommit);

            assert!(consume_repair_budget(
                &mut state,
                &mut loop_error,
                DEFAULT_REPAIR_ATTEMPTS_PER_SESSION,
            ));
            assert_eq!(loop_error.code.as_ref(), "TOOL_ARGS_REPAIR_REQUIRED");
        }

        let preflight_error = ToolCallPreflightError {
            call_id: ploke_core::ArcStr::from("call_preflight"),
            tool_name: ToolName::NsRead,
            rejected_arguments: "{\"file\":1}".to_string(),
            error: ToolError::new(
                ToolName::NsRead,
                ToolErrorCode::WrongType,
                "failed to parse tool arguments: EOF while parsing a value",
            ),
        };
        let context = base_error_context(1, 0, "tool_call_preflight", &None, Uuid::new_v4());
        let spec = semantics::normalize_tool_call_preflight_error(preflight_error, None, context);
        let mut loop_error = build_loop_error_from_semantic_spec(spec, CommitPhase::PreCommit);

        assert!(!consume_repair_budget(
            &mut state,
            &mut loop_error,
            DEFAULT_REPAIR_ATTEMPTS_PER_SESSION,
        ));
        assert_eq!(loop_error.code.as_ref(), "REPAIR_BUDGET_EXHAUSTED");
    }

    #[tokio::test]
    async fn execute_tools_via_event_bus_returns_immediately_for_empty_calls() {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let result = timeout(
            Duration::from_millis(100),
            execute_tools_via_event_bus(
                event_bus,
                Uuid::new_v4(),
                Uuid::new_v4(),
                Vec::new(),
                Duration::from_secs(1),
                ToolLoopMode::Auto,
            ),
        )
        .await
        .expect("empty tool call batch should not block");

        assert!(
            result.is_empty(),
            "empty tool batch should produce no results"
        );
    }

    #[tokio::test]
    async fn execute_tools_via_event_bus_gated_waits_for_settled_edit_result() {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let parent_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let call_id = ploke_core::ArcStr::from("call_gated_ns_patch");
        let tool_call = ToolCall {
            call_id: call_id.clone(),
            call_type: FunctionMarker,
            function: FunctionCall {
                name: ToolName::NsPatch,
                arguments: r#"{"patches":[]}"#.to_string(),
            },
            extra_content: None,
        };

        let mut requested_rx = event_bus.subscribe(crate::EventPriority::Realtime);
        let mut waiter = tokio::spawn(execute_tools_via_event_bus(
            event_bus.clone(),
            parent_id,
            request_id,
            vec![tool_call],
            Duration::from_secs(5),
            ToolLoopMode::Gated,
        ));

        loop {
            let event = timeout(Duration::from_secs(1), requested_rx.recv())
                .await
                .expect("tool request event should arrive")
                .expect("event bus should stay open");
            if matches!(
                event,
                AppEvent::System(SystemEvent::ToolCallRequested {
                    request_id: seen,
                    ..
                }) if seen == request_id
            ) {
                break;
            }
        }

        let staged_payload = ToolUiPayload::new(ToolName::NsPatch, call_id.clone(), "staged")
            .with_request_id(request_id)
            .with_proposal_id(Uuid::new_v4())
            .with_field("status", "pending")
            .with_field("staged", "1")
            .with_field("applied", "0");
        event_bus.send(AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            content: r#"{"ok":true,"staged":1,"applied":0}"#.to_string(),
            ui_payload: Some(staged_payload),
        }));

        assert!(
            timeout(Duration::from_millis(100), &mut waiter)
                .await
                .is_err(),
            "gated edit tool loop must not resolve on a pending staged completion"
        );

        let applied_payload = ToolUiPayload::new(ToolName::NsPatch, call_id.clone(), "applied")
            .with_request_id(request_id)
            .with_proposal_id(Uuid::new_v4())
            .with_field("status", "applied")
            .with_field("applied", "1");
        event_bus.send(AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            content: r#"{"ok":true,"applied":1}"#.to_string(),
            ui_payload: Some(applied_payload),
        }));

        let results = timeout(Duration::from_secs(1), &mut waiter)
            .await
            .expect("settled edit result should resolve")
            .expect("tool waiter task should not panic")
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 1);
        let (seen_call_id, result) = &results[0];
        assert_eq!(seen_call_id.as_ref(), call_id.as_ref());
        let result = result
            .as_ref()
            .expect("settled applied event should be a successful tool result");
        assert_eq!(result.content, r#"{"ok":true,"applied":1}"#);
    }
}
