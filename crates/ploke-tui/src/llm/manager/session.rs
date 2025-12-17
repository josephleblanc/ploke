use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::DateTime;
use ploke_llm::ChatHttpConfig;
use ploke_llm::ChatStepOutcome;
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
use crate::llm::ChatHistoryTarget;
use crate::tools::ToolDefinition;
use crate::tools::ToolName;
use crate::utils::consts::TOOL_CALL_TIMEOUT;
use ploke_llm::RequestMessage;
use ploke_llm::request::ToolChoice;
use ploke_llm::response::FinishReason;
use ploke_llm::response::OpenAiResponse;
use ploke_llm::router_only::{ApiRoute, ChatCompRequest, Router};

use ploke_llm::LlmError;

const OPENROUTER_RESPONSE_LOG_PARSED: &str = "logs/openrouter/session/last_parsed.json";

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
                })
            } else {
                Err(LlmError::Deserialization("No choices".into()))
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to Deserialize to json: {e}");
            Err(LlmError::Deserialization(err_msg))
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
#[derive(Clone, Copy)]
pub struct TuiToolPolicy {
    pub tool_call_timeout: ToolCallTimeout,
    pub tool_call_chain_limit: usize,
    pub retry_without_tools_on_404: bool,
}

type ToolCallTimeout = Duration;

impl Default for TuiToolPolicy {
    fn default() -> Self {
        Self {
            tool_call_timeout: Duration::from_secs(10),
            // TODO:ploke-llm 2025-12-14
            // Set to 15 as initial default, experiment to determine the right default to set
            tool_call_chain_limit: 15,
            retry_without_tools_on_404: false,
        }
    }
}

pub async fn run_chat_session<R: Router>(
    client: &Client,
    mut req: ChatCompRequest<R>,
    parent_id: Uuid,
    event_bus: Arc<EventBus>,
    state_cmd_tx: mpsc::Sender<StateCommand>,
    policy: TuiToolPolicy,
) -> Result<String, LlmError> {
    // Optionally: set tool_choice=Auto if tools exist, etc.

    // TODO:ploke-llm 2025-12-14
    // placeholder default config for now, fix up later
    let cfg = ChatHttpConfig::default();
    let mut initial_message_updated = false;
    for _chain in 0..policy.tool_call_chain_limit {
        let outcome = ploke_llm::chat_step(client, &req, &cfg).await?;
        // match ploke_llm::chat_step(client, &req, &cfg).await {
        //     Ok(chat_step_outcome) => chat_step_outcome,
        //     Err(e) => {}
        // };

        match outcome {
            ChatStepOutcome::Content(text) => return Ok(text),

            ChatStepOutcome::ToolCalls { calls, content, .. } => {
                req.core
                    .messages
                    .push(RequestMessage::new_assistant_with_tool_calls(
                        content.clone(),
                        calls.clone(),
                    ));
                let step_request_id = Uuid::new_v4();
                // 1) update placeholder message once (UI concern)
                if !initial_message_updated {
                    let is_updated = update_assistant_placeholder_once(
                        &state_cmd_tx,
                        parent_id,
                        content,
                        initial_message_updated,
                    )
                    .await;
                    initial_message_updated = is_updated;
                } else {
                    let msg = content.unwrap_or_else(|| "Calling tools...".to_string());
                    state_cmd_tx
                        .send(StateCommand::AddMessageImmediate {
                            msg,
                            kind: MessageKind::Assistant,
                            new_msg_id: Uuid::new_v4(), 
                        })
                        .await
                        .expect("state manager must be running");
                }

                // 2) run tools (EventBus + waiting is TUI concern)
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
                                    tool_call_id: call_id,
                                })
                                .await
                                .expect("state manager must be running");
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
                                    tool_call_id: call_id,
                                })
                                .await
                                .expect("state manager must be running");
                            continue;
                        }
                    }
                }

                // loop again
            }
        }
    }

    Err(LlmError::ToolCall("tool call chain limit exceeded".into()))
}

#[instrument(skip(state_cmd_tx), fields( msg_content = ?content, initial_message_updated ))]
async fn update_assistant_placeholder_once(
    state_cmd_tx: &mpsc::Sender<StateCommand>,
    parent_id: Uuid,
    content: Option<String>,
    initial_message_updated: bool,
) -> bool {
    let assistant_update = content.unwrap_or_else(|| String::from("Calling Tools"));

    if !initial_message_updated {
        state_cmd_tx
            .send(StateCommand::UpdateMessage {
                id: parent_id,
                update: MessageUpdate {
                    content: Some(assistant_update),
                    status: Some(MessageStatus::Completed),
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

pub struct ToolExecPolicy {
    pub timeout: Duration,
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

async fn log_api_parsed_json_response(
    url: &str,
    status: u16,
    parsed: &OpenAiResponse,
) -> color_eyre::Result<()> {
    let payload: String = serde_json::to_string_pretty(parsed)?;
    info!(target: "api_json", "\n// URL: {url}\n// Status: {status}\n{payload}\n");
    Ok(())
}

impl<'a, R> RequestSession<'a, R>
where
    R: Router,
    R::CompletionFields: ApiRoute,
{
    pub async fn run(mut self) -> Result<String, LlmError> {
        #[derive(serde::Deserialize)]
        struct CodeEditArgsMinimal {
            edits: Vec<CodeEditEditMinimal>,
        }
        #[derive(serde::Deserialize)]
        struct CodeEditEditMinimal {
            file: String,
            code: String,
        }

        // Use router-level constants for URL and API key
        let url = R::COMPLETION_URL;
        let api_key = R::resolve_api_key()
            .map_err(|e| LlmError::Request(format!("missing api key: {}", e)))?;

        // Determine whether to include tools
        let mut use_tools = self.req.tools.is_some();
        let mut tools_fallback_attempted = false;
        let mut assistant_intro: String = String::new();
        let state_cmd_tx = self.state_cmd_tx.clone();

        let mut initial_message_updated = false;
        for _attempt in 0..=self.attempts {
            if !use_tools {
                self.req.tools = None;
                self.req.tool_choice = None;
            } else if self.req.tool_choice.is_none() && self.req.tools.is_some() {
                self.req.tool_choice = Some(ToolChoice::Auto);
            }

            let _ = self.log_request().await;
            let response = self
                .client
                .post(url)
                .bearer_auth(&api_key)
                .header("Accept", "application/json")
                .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
                .header("X-Title", "Ploke TUI")
                .json(&self.req)
                .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
                .send()
                .await
                .map_err(|e| LlmError::Request(e.to_string()))?;

            if !response.status().is_success() {
                let error_code = response.status();
                tracing::error!(status_code = ?error_code, "Error status returned from API");
                let status = response.status().as_u16();
                let text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "<no error body>".into());

                if status == 404
                    && use_tools
                    && text.to_lowercase().contains("support tool")
                    && self.fallback_on_404
                    && !tools_fallback_attempted
                {
                    let notice = format!(
                        "Notice: endpoint appears to lack tool support; retrying without tools.\n\n{}",
                        text
                    );
                    self.req
                        .core
                        .messages
                        .push(RequestMessage::new_system(notice));
                    use_tools = false;
                    tools_fallback_attempted = true;
                    continue;
                }
                return Err(LlmError::Api {
                    status,
                    message: text,
                });
            }

            let log_url = response.url().to_string();
            let log_status = response.status().as_u16();
            let body_text = response
                .text()
                .await
                .map_err(|e| LlmError::Request(e.to_string()))?;

            // Attempt to log parsed response; fall back to provider-embedded error detection
            if let Ok(parsed) = serde_json::from_str::<OpenAiResponse>(&body_text) {
                let mut log_dir = workspace_root();
                log_dir.push(OPENROUTER_RESPONSE_LOG_PARSED);
                let _ = log_api_parsed_json_response(&log_url, log_status, &parsed).await;
            } else if let Err(err) = check_provider_error(&body_text) {
                return Err(err);
            }

            match ploke_llm::manager::parse_chat_outcome(&body_text)? {
                ChatStepOutcome::ToolCalls {
                    calls: tool_calls,
                    content,
                    finish_reason,
                } => {
                    tracing::debug!(calls = ?tool_calls, ?content);
                    let assistant_update = content.unwrap_or_else(|| String::from("Calling Tools"));

                    if !initial_message_updated {
                        state_cmd_tx
                            .send(StateCommand::UpdateMessage {
                                id: self.parent_id,
                                update: MessageUpdate {
                                    content: Some(assistant_update),
                                    status: Some(MessageStatus::Completed),
                                    ..Default::default()
                                },
                            })
                            .await
                            .expect("state command must be running");
                        initial_message_updated = true;
                    }

                    let mut task_set = tokio::task::JoinSet::new();
                    let mut call_feedback: HashMap<
                        ploke_core::ArcStr,
                        (uuid::Uuid, Option<(String, String)>),
                    > = HashMap::new();
                    for call in tool_calls.into_iter() {
                        let tool_name = call.function.name;
                        let args_json = call.function.arguments.clone();
                        let event_bus = self.event_bus.clone();
                        let parent_id = self.parent_id;
                        let request_id = Uuid::new_v4();
                        let call_id = call.call_id.clone();

                        let summary = if matches!(tool_name, ToolName::ApplyCodeEdit) {
                            if let Ok(parsed) =
                                serde_json::from_str::<CodeEditArgsMinimal>(&args_json)
                            {
                                if let Some(first) = parsed.edits.first() {
                                    let file = first.file.clone();
                                    let snippet: String = first.code.chars().take(100).collect();
                                    Some((file, snippet))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        call_feedback.insert(call_id.clone(), (request_id, summary));

                        let mut rx = event_bus.realtime_tx.subscribe();
                        let cmd_tx = state_cmd_tx.clone();
                        let cmd_tx_clone = state_cmd_tx.clone();

                        event_bus.send(AppEvent::System(SystemEvent::ToolCallRequested {
                            tool_call: call,
                            request_id,
                            parent_id,
                        }));

                        task_set.spawn(async move {
                            let call_id_clone = call_id.clone();
                            let wait = async move {
                                while let Ok(evt) = rx.recv().await {
                                    tracing::debug!(?evt, "recv wait tool event for matching");
                                    match evt {
                                        AppEvent::System(SystemEvent::ToolCallCompleted {
                                            request_id: rid,
                                            call_id: cid,
                                            content,
                                            ..
                                        }) if rid == request_id && cid == call_id => {
                                            tracing::debug!(%request_id, ?call_id, ?content, "tool call completed");
                                            return Ok(content);
                                        }
                                        AppEvent::System(SystemEvent::ToolCallFailed {
                                            request_id: rid,
                                            call_id: cid,
                                            error,
                                            ..
                                        }) if rid == request_id && cid == call_id => {
                                            add_sysinfo_message(&call_id, &cmd_tx, "tool call error").await;
                                            return Err(error);
                                        }
                                        _ => {}
                                    }
                                }
                                Err("Event channel closed".to_string())
                            };
                            match tokio::time::timeout(Duration::from_secs(TOOL_CALL_TIMEOUT), wait).await {
                                Ok(r) => (call_id_clone, r),
                                Err(_) => {
                                    add_sysinfo_message(&call_id_clone, &cmd_tx_clone, "timeout").await;
                                    ( call_id_clone, Err("Timed out waiting for tool result".into() ) ) 
                                }
                            }
                        });
                    }

                    while let Some(res) = task_set.join_next().await {
                        match res {
                            Ok((cid, Ok(content))) => {
                                // Append the tool's raw JSON result for the next request
                                self.req
                                    .core
                                    .messages
                                    .push(RequestMessage::new_tool(content, cid.clone()));

                                // If this was an apply_code_edit call, also append a concise System summary
                                if let Some((rid, Some((file, snippet)))) =
                                    call_feedback.get(&cid).cloned()
                                {
                                    let sys_msg = format!(
                                        "Staged code edit recorded.
request_id: {}
file: {}
snippet (first 100 chars):
```
{}
```
If you are ready to return control to the user, respond with finish_reason 'stop'.",
                                        rid, file, snippet
                                    );
                                    self.req
                                        .core
                                        .messages
                                        .push(RequestMessage::new_system(sys_msg));
                                }
                            }
                            Ok((cid, Err(err_string))) => {
                                tracing::debug!(tool_content = ?cid, error_msg = ?err_string);
                                let content = json!({"ok": false, "error": err_string}).to_string();
                                self.req
                                    .core
                                    .messages
                                    .push(RequestMessage::new_tool(content, cid.clone()));
                                let err_msg = format!("tool failed\n\t{cid:?}\n\t{err_string:?}");
                                state_cmd_tx
                                    .send(StateCommand::AddMessageTool {
                                        new_msg_id: Uuid::new_v4(),
                                        msg: err_msg.clone(),
                                        // TODO: Change to 'Tool'
                                        kind: MessageKind::Tool,
                                        tool_call_id: cid,
                                    })
                                    .await
                                    .expect("state manager must be running");
                                continue;
                                // return Err(LlmError::ToolCall(err_msg));
                            }
                            Err(join_err) => {
                                return Err(LlmError::ToolCall(format!(
                                    "join error: {}",
                                    join_err
                                )));
                            }
                        }
                    }
                    if finish_reason == FinishReason::ToolCalls {
                        let remember_stop = "Tool Call completed. Remember to end with a 'stop' finish reason to return conversation control to the user.";
                        state_cmd_tx
                            .send(StateCommand::AddMessageImmediate {
                                msg: remember_stop.to_string(),
                                kind: MessageKind::System,
                                new_msg_id: Uuid::new_v4(),
                            })
                            .await
                            .expect("state manager must be running");
                        continue;
                    } else {
                        if assistant_intro.is_empty() {
                            assistant_intro.push_str("Calling tools")
                        }
                        return Ok(assistant_intro);
                    }
                }
                ChatStepOutcome::Content(content) => {
                    return Ok(content);
                }
            }
        }

        Err(LlmError::Unknown(format!(
            "exhausted after {} attempt(s)",
            self.attempts
        )))
    }

    async fn log_request(&self) -> color_eyre::Result<()> {
        let payload: String = serde_json::to_string_pretty(&self.req)?;
        info!(target: "api_json", "{}", payload);
        Ok(())
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
        match r {
            ChatStepOutcome::Content(c) => assert_eq!(c, "Hello world"),
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
        match r {
            ChatStepOutcome::Content(c) => assert_eq!(c, "Hello text"),
            _ => panic!("expected content"),
        }
    }
}
