use std::ops::Mul;
use std::{collections::HashMap, fs, sync::Arc, time::Duration};

use chrono::DateTime;
use ploke_llm::ChatHttpConfig;
use ploke_llm::ChatStepOutcome;
use ploke_llm::manager::ChatStepData;
use ploke_llm::response::ToolCall;
use ploke_test_utils::workspace_root;
use reqwest::Client;
use serde_json::json;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::instrument;
use uuid::Uuid;

use crate::AppEvent;
use crate::EventBus;
use crate::app_state::StateCommand;
use crate::app_state::events::SystemEvent;
use crate::chat_history::MessageKind;
use crate::chat_history::MessageStatus;
use crate::chat_history::MessageUpdate;
use crate::tracing_setup::FINISH_REASON_TARGET;
use crate::utils::consts::TOOL_CALL_TIMEOUT;
use ploke_llm::RequestMessage;
use ploke_llm::response::FinishReason;
use ploke_llm::response::OpenAiResponse;
use ploke_llm::router_only::{ApiRoute, ChatCompRequest, Router};

use crate::llm::manager::loop_error::{
    ChatSessionReport, CommitPhase, ErrorAudience, ErrorContext, LoopError, SessionOutcome,
    Verbosity, classify_llm_error, render_error_view,
};
use ploke_llm::LlmError;

const OPENROUTER_REQUEST_LOG: &str = "logs/openrouter/session/last_request.json";
const OPENROUTER_RESPONSE_LOG_PARSED: &str = "logs/openrouter/session/last_parsed.json";
const OPENROUTER_RESPONSE_LOG_RAW: &str = "logs/openrouter/session/last_response_raw.txt";

fn check_provider_error(body_text: &str) -> Result<(), LlmError> {
    // Providers sometimes put errors inside a 200 body
    match serde_json::from_str::<serde_json::Value>(body_text) {
        Ok(v) => {
            if let Some(err) = v.get("error") {
                let msg = err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown provider error");
                let code = err.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
                Err(LlmError::Api {
                    status: code as u16,
                    message: msg.to_string(),
                    url: None,
                    body_snippet: Some(truncate_for_error(body_text, 4_096)),
                })
            } else {
                Err(LlmError::Deserialization {
                    message: "No choices".into(),
                    body_snippet: Some(truncate_for_error(body_text, 512)),
                })
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to Deserialize to json: {e}");
            Err(LlmError::Deserialization {
                message: err_msg,
                body_snippet: Some(truncate_for_error(body_text, 512)),
            })
        }
    }
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

#[derive(Clone, Copy, Debug)]
pub struct FinishPolicy {
    /// Timeout backoff/limit behavior for FinishReason::Timeout.
    timeout: TuiTimeoutPolicy,
    /// Retry policy for FinishReason::Error.
    error: TuiErrorPolicy,
    /// Retry policy for FinishReason::Length.
    length: TuiLengthPolicy,
    /// System prompt appended when retrying after FinishReason::Length.
    length_continue_prompt: &'static str,
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
            length_continue_prompt: "Continue from where you left off. Do not repeat prior text.",
        }
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

/// Outcome of finish-reason evaluation for a single response.
///
/// Continue variants tell the caller to retry the chat step, optionally with
/// a system message appended before the next request.
enum FinishDecision {
    Continue,
    ContinueWithSystemMessage(String),
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
struct ChatLoopState {
    retried_errors: u32,
    retried_lengths: u32,
    timeout_attempts: usize,
}

impl Default for ChatLoopState {
    fn default() -> Self {
        Self {
            retried_errors: 0,
            retried_lengths: 0,
            timeout_attempts: 0,
        }
    }
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
            return match continue_message {
                Some(msg) => FinishDecision::ContinueWithSystemMessage(msg),
                None => FinishDecision::Continue,
            };
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

#[tracing::instrument(
    target = "chat-loop",
    skip(client, req, event_bus, state_cmd_tx, policy),
    fields(
        model_key = ?req.model_key,
        assistant_message_id = %assistant_message_id,
        parent_id = %parent_id
    )
)]
/// Chat loop structure:
/// - issue a chat step
/// - handle tool calls (if any), update UI, append tool results
/// - handle finish reasons to decide return vs retry
// Optionally: set tool_choice=Auto if tools exist, etc.
pub async fn run_chat_session<R: Router>(
    client: &Client,
    mut req: ChatCompRequest<R>,
    parent_id: Uuid,
    assistant_message_id: Uuid,
    event_bus: Arc<EventBus>,
    state_cmd_tx: mpsc::Sender<StateCommand>,
    policy: TuiToolPolicy,
) -> ChatSessionReport {
    // TODO:ploke-llm 2025-12-14
    // placeholder default config for now, fix up later
    let mut cfg = ChatHttpConfig::default();
    let finish_policy = FinishPolicy::default();
    let mut loop_state = ChatLoopState::default();
    let model_key = req.model_key.clone();
    let session_id = Uuid::new_v4();
    let mut report =
        ChatSessionReport::new(session_id, assistant_message_id, parent_id, assistant_message_id);
    let mut commit_phase = CommitPhase::PreCommit;
    let mut attempts = 0_u32;

    let mut initial_message_updated = false;
    for chain_index in 0..policy.tool_call_chain_limit {
        attempts = attempts.saturating_add(1);
        let ChatStepData {
            outcome,
            full_response,
        } = match ploke_llm::chat_step(client, &req, &cfg).await {
            Ok(step) => step,
            Err(err) => {
                let context = base_error_context(
                    attempts,
                    chain_index,
                    "chat_step",
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
        };

        match outcome {
            ChatStepOutcome::ToolCalls {
                calls,
                content,
                reasoning,
                ..
            } => {
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
                )
                .await;

                // 3) append tool results into req.core.messages for the next step
                for (call_id, tool_json_result) in results.into_iter() {
                    let call_id_for_state = call_id.clone();
                    match tool_json_result {
                        Ok(tool_json) => {
                            req.core
                                .messages
                                .push(RequestMessage::new_tool(tool_json.clone(), call_id.clone()));
                            state_cmd_tx
                                .send(StateCommand::AddMessageTool {
                                    new_msg_id: Uuid::new_v4(),
                                    msg: tool_json,
                                    kind: MessageKind::Tool,
                                    tool_call_id: call_id_for_state,
                                })
                                .await
                                .expect("state manager must be running");
                            commit_phase = CommitPhase::ToolResultsCommitted;
                        }
                        Err(err_string) => {
                            let content = json!({ "ok": false, "error": err_string }).to_string();
                            req.core
                                .messages
                                .push(RequestMessage::new_tool(content, call_id.clone()));

                            let err_msg = format!("tool failed\n\t{call_id:?}\n\t{err_string:?}");
                            state_cmd_tx
                                .send(StateCommand::AddMessageTool {
                                    new_msg_id: Uuid::new_v4(),
                                    msg: err_msg.clone(),
                                    kind: MessageKind::Tool,
                                    tool_call_id: call_id_for_state,
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
                            let tool_err = LlmError::ToolCall(err_string);
                            let loop_error =
                                classify_llm_error(&tool_err, context, commit_phase.clone());
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

        match finish_policy.handle_finish_reasons(full_response, &mut ctx, &mut loop_state) {
            FinishDecision::Continue => continue,
            FinishDecision::ContinueWithSystemMessage(msg) => {
                req.core.messages.push(RequestMessage::new_system(msg));
                continue;
            }
            FinishDecision::Return(result) => match result {
                Ok(_response) => {
                    report.outcome = SessionOutcome::Completed;
                    report.commit_phase = commit_phase;
                    report.attempts = attempts;
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

pub async fn execute_tools_via_event_bus(
    event_bus: Arc<EventBus>,
    parent_id: Uuid,
    step_request_id: Uuid,
    calls: Vec<ToolCall>,
    policy_timeout: ToolCallTimeout,
) -> Vec<(ploke_core::ArcStr, Result<String, String>)> {
    // One receiver for the whole batch
    let mut rx = event_bus.realtime_tx.subscribe();

    // Per-call waiters
    let mut waiters: HashMap<ploke_core::ArcStr, oneshot::Sender<Result<String, String>>> =
        HashMap::new();
    let mut handles = Vec::new();

    for call in &calls {
        let (tx, rx_one) = oneshot::channel();
        waiters.insert(call.call_id.clone(), tx);

        let call_id = call.call_id.clone();
        handles.push(async move {
            // timeout wrapper per call
            match tokio::time::timeout(policy_timeout, rx_one).await {
                Ok(Ok(res)) => (call_id, res),
                Ok(Err(_closed)) => (call_id, Err("tool waiter dropped".into())),
                Err(_) => (call_id, Err("Timed out waiting for tool result".into())),
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
                    ..
                })) if request_id == step_request_id => {
                    if let Some(tx) = waiters.remove(&call_id) {
                        let _ = tx.send(Ok(content));
                    }
                    if waiters.is_empty() {
                        break;
                    }
                }
                Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    call_id,
                    error,
                    ..
                })) if request_id == step_request_id => {
                    if let Some(tx) = waiters.remove(&call_id) {
                        let _ = tx.send(Err(error));
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
                        let _ = tx.send(Err("Event channel closed".into()));
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

fn truncate_for_error(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let head = &s[..max.saturating_sub(200)];
        let tail = &s[s.len().saturating_sub(200)..];
        format!("{head}…<snip>…{tail}")
    }
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
    use ploke_llm::manager::parse_chat_outcome;
    use std::time::Duration;

    use super::*;
    use crate::EventBus;
    use crate::event_bus::EventBusCaps;
    use crate::llm::router_only::openrouter::OpenRouter;
    use crate::tools::ToolName;

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
}
