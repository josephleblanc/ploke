use std::time::Duration;

use chrono::DateTime;
use ploke_test_utils::workspace_root;
use reqwest::Client;
use serde_json::json;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::llm2::response::OpenAiResponse;
use crate::utils::consts::TOOL_CALL_TIMEOUT;
use crate::AppEvent;
use crate::EventBus;
use crate::app_state::events::SystemEvent;
use crate::llm2::manager::RequestMessage;
use crate::llm2::request::endpoint::ToolChoice;
use crate::llm2::router_only::{ApiRoute, ChatCompRequest, Router};
use crate::tools::ToolDefinition;

use super::LlmError;

const OPENROUTER_RESPONSE_LOG_PARSED: &str = "logs/openrouter/session/last_parsed.json";

#[derive(Debug, PartialEq)]
enum ParseOutcome {
    ToolCalls { calls: Vec<crate::tools::ToolCall>, content: Option<String> },
    Content(String),
}

fn parse_outcome(body_text: &str) -> Result<ParseOutcome, LlmError> {
    // First, detect provider-embedded errors inside a 200 body
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body_text) {
        if let Some(err) = v.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown provider error");
            let code = err.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
            return Err(LlmError::Api {
                status: code as u16,
                message: msg.to_string(),
            });
        }
    }

    // Parse into normalized response
    let parsed: OpenAiResponse = serde_json::from_str(body_text)
        .map_err(|e| LlmError::Deserialization(format!("{} — body was: {}", e, body_text)))?;

    // Some providers may return both content and tool calls in the same message.
    if let Some(choice) = parsed.choices.into_iter().next() {
        if let Some(msg) = choice.message {
            let content = msg.content;
            if let Some(tc) = msg.tool_calls {
                return Ok(ParseOutcome::ToolCalls { calls: tc, content });
            }
            let content = content.unwrap_or_default();
            return Ok(ParseOutcome::Content(content));
        } else if let Some(text) = choice.text {
            return Ok(ParseOutcome::Content(text));
        } else if let Some(_delta) = choice.delta {
            return Err(LlmError::Deserialization(
                "Unexpected streaming delta".into(),
            ));
        } else {
            return Err(LlmError::Deserialization("Empty choice".into()));
        }
    }
    Err(LlmError::Deserialization("No choices".into()))

}

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
    pub event_bus: std::sync::Arc<EventBus>,
    pub parent_id: Uuid,
    pub req: ChatCompRequest<R>,
    pub fallback_on_404: bool,
    pub attempts: u32,
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
        // Use router-level constants for URL and API key
        let url = R::COMPLETION_URL;
        let api_key = R::resolve_api_key()
            .map_err(|e| LlmError::Request(format!("missing api key: {}", e)))?;

        // Determine whether to include tools
        let mut use_tools = self.req.tools.is_some();
        let mut tools_fallback_attempted = false;
        let mut assistant_intro: String = String::new();

        for _attempt in 0..=self.attempts {
            if !use_tools {
                self.req.tools = None;
                self.req.tool_choice = None;
            } else if self.req.tool_choice.is_none() && self.req.tools.is_some() {
                self.req.tool_choice = Some(ToolChoice::Auto);
            }

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
                return Err(LlmError::Api { status, message: text });
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

            match parse_outcome(&body_text)? {
                ParseOutcome::ToolCalls { calls: tool_calls, content } => {
                    tracing::debug!(calls = ?tool_calls, ?content);
                    if let Some(text) = content {
                        if !text.is_empty() {
                            if !assistant_intro.is_empty() {
                                assistant_intro.push_str("\n\n");
                            }
                            assistant_intro.push_str(&text);
                            self.req.core.messages.push(RequestMessage::new_assistant(text));
                        }
                    }

                    let mut task_set = tokio::task::JoinSet::new();
                    for call in tool_calls.into_iter() {
                        let event_bus = self.event_bus.clone();
                        let parent_id = self.parent_id;
                        let request_id = Uuid::new_v4();
                        let call_id = call.call_id.clone();
                        let mut rx = event_bus.realtime_tx.subscribe();
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
                                            return Err(error);
                                        }
                                        _ => {}
                                    }
                                }
                                Err("Event channel closed".to_string())
                            };
                            match tokio::time::timeout(Duration::from_secs(TOOL_CALL_TIMEOUT), wait).await {
                                Ok(r) => (call_id_clone, r),
                                Err(_) => (
                                    call_id_clone,
                                    Err("Timed out waiting for tool result".into()),
                                ),
                            }
                        });
                    }

                    while let Some(res) = task_set.join_next().await {
                        match res {
                            Ok((cid, Ok(content))) => {
                                self.req.core.messages.push(RequestMessage::new_tool(content, cid));
                            }
                            Ok((cid, Err(err))) => {
                                tracing::debug!(tool_content = ?cid, error_msg = ?err);
                                let content = json!({"ok": false, "error": err}).to_string();
                                self.req.core.messages.push(RequestMessage::new_tool(content, cid.clone()));
                                let err_msg = format!("tool failed\n\t{cid:?}\n\t{err:?}");
                                return Err(LlmError::ToolCall(err_msg));
                            }
                            Err(join_err) => {
                                return Err(LlmError::ToolCall(format!("join error: {}", join_err)));
                            }
                        }
                    }
                    continue;
                }
                ParseOutcome::Content(content) => {
                    if assistant_intro.is_empty() {
                        return Ok(content);
                    } else {
                        let mut combined = assistant_intro;
                        if !combined.ends_with('\n') { combined.push('\n'); }
                        if !combined.ends_with("\n\n") { combined.push('\n'); }
                        combined.push_str(&content);
                        return Ok(combined);
                    }
                }
            }
        }

        Err(LlmError::Unknown(format!(
            "exhausted after {} attempt(s)",
            self.attempts
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventBus;
    use crate::event_bus::EventBusCaps;
    use crate::llm2::router_only::openrouter::OpenRouter;
    use crate::tools::ToolName;

    #[test]
    fn parse_outcome_content_message() {
        let body = r#"{
            "choices": [
                { "message": {"role": "assistant", "content": "Hello world"} }
            ]
        }"#;
        let r = parse_outcome(body).unwrap();
        match r {
            ParseOutcome::Content(c) => assert_eq!(c, "Hello world"),
            _ => panic!("expected content"),
        }
    }

    #[test]
    fn parse_outcome_text_field() {
        let body = r#"{
            "choices": [
                { "text": "Hello text" }
            ]
        }"#;
        let r = parse_outcome(body).unwrap();
        match r {
            ParseOutcome::Content(c) => assert_eq!(c, "Hello text"),
            _ => panic!("expected content"),
        }
    }

    #[test]
    fn parse_outcome_tool_calls() {
        let body = r#"{
            "choices": [
                { "message": { 
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": { "name": "request_code_context", "arguments": "{\\"token_budget\\": 64}" }
                        }
                    ]
                }}
            ]
        }"#;
        let r = parse_outcome(body).unwrap();
        match r {
            ParseOutcome::ToolCalls { calls: tc, content } => {
                assert!(content.is_none());
                assert_eq!(tc.len(), 1);
                assert_eq!(tc[0].call_id.as_ref(), "call_1");
                assert_eq!(tc[0].function.name, ToolName::RequestCodeContext);
            }
            _ => panic!("expected tool calls"),
        }
    }

    #[test]
    fn parse_outcome_tool_calls_with_content() {
        let body = r#"{
            "choices": [
                { "message": { 
                    "content": "I will fetch context.",
                    "tool_calls": [
                        {
                            "id": "call_2",
                            "type": "function",
                            "function": { "name": "request_code_context", "arguments": "{\\"token_budget\\": "SomeStruct"}" }
                        }
                    ]
                }}
            ]
        }"#;
        let r = parse_outcome(body).unwrap();
        match r {
            ParseOutcome::ToolCalls { calls: tc, content } => {
                assert_eq!(content.as_deref(), Some("I will fetch context."));
                assert_eq!(tc.len(), 1);
                assert_eq!(tc[0].call_id.as_ref(), "call_2");
            }
            _ => panic!("expected tool calls with content"),
        }
    }
}
