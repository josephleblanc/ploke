use std::ops::Mul;
use std::{collections::HashMap, fs, sync::Arc, time::Duration};

use crate::user_config::{ChatPolicy, ChatTimeoutStrategy};
use chrono::DateTime;
use ploke_llm::ChatHttpConfig;
use ploke_llm::ChatStepOutcome;
use ploke_llm::manager::ChatStepData;
use ploke_llm::response::ToolCall;
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
const MAX_REPAIR_ATTEMPTS_PER_SESSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FullResponseTraceRecord {
    assistant_message_id: Uuid,
    response_index: usize,
    response: OpenAiResponse,
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

fn repair_budget_exhausted(state: &ChatLoopState) -> bool {
    // Keep repair bounded independently from generic request retries and from the broader
    // tool-call chain cap so repeated provider/model repair loops cannot dominate the turn.
    state.repair_attempts >= MAX_REPAIR_ATTEMPTS_PER_SESSION
}

fn consume_repair_budget(state: &mut ChatLoopState, loop_error: &mut LoopError) -> bool {
    if !matches!(loop_error.recovery, RecoveryDecision::Repair { .. }) {
        return true;
    }
    if repair_budget_exhausted(state) {
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
                        ctx.cfg.timeout = next_timout;
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

pub struct ChatSession<R: Router> {
    pub client: Client,
    pub req: ChatCompRequest<R>,
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
pub async fn run_chat_session<R: Router>(
    session: ChatSession<R>,
    llm_timeout_secs: u64,
) -> ChatSessionReport {
    let ChatSession {
        client,
        mut req,
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
    let http_timeout = Duration::from_secs(llm_timeout_secs);

    // TODO:ploke-llm 2025-12-14
    // placeholder default config for now, fix up later
    let mut cfg = ChatHttpConfig::default();
    cfg.timeout = http_timeout;
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
        let ChatStepData {
            outcome,
            full_response,
        } = match tokio::select! {
            res = ploke_llm::chat_step(&client, &req, &cfg) => res,
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
            Err(err) => {
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
                    if !consume_repair_budget(&mut loop_state, &mut loop_error) {
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
                if matches!(&loop_error.recovery, RecoveryDecision::Retry { .. })
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
                        if !consume_repair_budget(&mut loop_state, &mut loop_error) {
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
                            req.core.messages.push(RequestMessage::new_tool(
                                tool_result.content.clone(),
                                call_id.clone(),
                            ));
                            state_cmd_tx
                                .send(StateCommand::AddMessageTool {
                                    new_msg_id: Uuid::new_v4(),
                                    msg: tool_result.content,
                                    kind: MessageKind::Tool,
                                    tool_call_id: call_id_for_state,
                                    tool_payload: tool_result.ui_payload,
                                })
                                .await
                                .expect("state manager must be running");
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

                            state_cmd_tx
                                .send(StateCommand::AddMessageTool {
                                    new_msg_id: Uuid::new_v4(),
                                    msg: content,
                                    kind: MessageKind::Tool,
                                    tool_call_id: call_id_for_state,
                                    tool_payload: tool_error.ui_payload,
                                })
                                .await
                                .expect("state manager must be running");
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

        let trace_record = FullResponseTraceRecord {
            assistant_message_id,
            response_index: chain_index,
            response: full_response.clone(),
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
        state_cmd_tx
            .send(StateCommand::AddMessageImmediate {
                msg,
                kind: MessageKind::Assistant,
                new_msg_id: Uuid::new_v4(),
            })
            .await
            .expect("state manager must be running");
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

    state_cmd_tx
        .send(StateCommand::AddMessageImmediate {
            msg,
            kind: MessageKind::System,
            new_msg_id: Uuid::new_v4(),
        })
        .await
        .expect("state manager must be running");
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
        state_cmd_tx
            .send(StateCommand::UpdateMessage {
                id: assistant_message_id,
                update: MessageUpdate {
                    content: Some(content),
                    status: Some(status),
                    ..Default::default()
                },
            })
            .await
            .inspect_err(|e| tracing::error!("{e:#?}"))
            .expect("state command must be running");
        true
    } else {
        false
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
    cmd_tx
        .send(StateCommand::AddMessageImmediate {
            msg: completed_msg,
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        })
        .await
        .expect("state manager must be running");
}

#[tracing::instrument]
async fn add_tool_failed_message(
    call_id: &ploke_core::ArcStr,
    cmd_tx: &mpsc::Sender<StateCommand>,
    status_msg: &str,
) {
    let completed_msg = format!("Tool call {}: {}", status_msg, call_id.as_ref());
    cmd_tx
        .send(StateCommand::AddMessageImmediate {
            msg: completed_msg,
            kind: MessageKind::System,
            new_msg_id: Uuid::new_v4(),
        })
        .await
        .expect("state manager must be running");
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use once_cell::sync::Lazy;
    use ploke_llm::manager::parse_chat_outcome;
    use ploke_llm::router_only::ChatCompRequest;
    use ploke_llm::router_only::Router;
    use ploke_llm::router_only::openrouter::ChatCompFields;
    use ploke_llm::router_only::openrouter::OpenRouterModelId;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use super::*;
    use crate::EventBus;
    use crate::event_bus::EventBusCaps;
    use crate::tools::ToolName;
    use crate::user_config::ChatPolicy;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::{Mutex, mpsc, watch};
    use tokio::time::timeout;

    static TEST_ROUTER_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    const TEST_ROUTER_URL: &str = "http://127.0.0.1:39181/v1/chat/completions";
    const TEST_ROUTER_URL_ALT: &str = "http://127.0.0.1:39182/v1/chat/completions";

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
        tokio::spawn(async move {
            let listener = TcpListener::bind(bind_addr)
                .await
                .expect("bind test router");
            for body in responses {
                let Ok(Ok((mut stream, _))) =
                    timeout(Duration::from_millis(500), listener.accept()).await
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
        for _ in 0..MAX_REPAIR_ATTEMPTS_PER_SESSION {
            assert!(!repair_budget_exhausted(&state));
            state.repair_attempts = state.repair_attempts.saturating_add(1);
        }
        assert!(repair_budget_exhausted(&state));
    }

    #[test]
    fn consume_repair_budget_marks_error_exhausted_after_limit() {
        let mut state = ChatLoopState::default();

        for _ in 0..MAX_REPAIR_ATTEMPTS_PER_SESSION {
            let preflight_error = ToolCallPreflightError {
                call_id: ploke_core::ArcStr::from("call_preflight"),
                tool_name: ToolName::NsRead,
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

            assert!(consume_repair_budget(&mut state, &mut loop_error));
            assert_eq!(loop_error.code.as_ref(), "TOOL_ARGS_REPAIR_REQUIRED");
        }

        let preflight_error = ToolCallPreflightError {
            call_id: ploke_core::ArcStr::from("call_preflight"),
            tool_name: ToolName::NsRead,
            error: ToolError::new(
                ToolName::NsRead,
                ToolErrorCode::WrongType,
                "failed to parse tool arguments: EOF while parsing a value",
            ),
        };
        let context = base_error_context(1, 0, "tool_call_preflight", &None, Uuid::new_v4());
        let spec = semantics::normalize_tool_call_preflight_error(preflight_error, None, context);
        let mut loop_error = build_loop_error_from_semantic_spec(spec, CommitPhase::PreCommit);

        assert!(!consume_repair_budget(&mut state, &mut loop_error));
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
            ),
        )
        .await
        .expect("empty tool call batch should not block");

        assert!(
            result.is_empty(),
            "empty tool batch should produce no results"
        );
    }
}
