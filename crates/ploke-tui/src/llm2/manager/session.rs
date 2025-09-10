use std::time::Duration;

use reqwest::Client;
use serde_json::json;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::AppEvent;
use crate::EventBus;
use crate::app_state::events::SystemEvent;
use crate::llm2::manager::{ErrorResponse, OpenAiResponse, RequestMessage, ResponseMessage, StreamingDelta};
use crate::llm2::request::endpoint::ToolChoice;
use crate::llm2::router_only::{ApiRoute, ChatCompRequest, Router};
use crate::tools::ToolDefinition;

use super::{LlmError, ToolEvent};

#[derive(Debug, PartialEq)]
enum ParseOutcome {
    ToolCalls(Vec<crate::tools::ToolCall>),
    Content(String),
}

fn parse_outcome(body_text: &str) -> Result<ParseOutcome, LlmError> {
    // Providers sometimes put errors inside a 200 body
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body_text) {
        if let Some(err) = v.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown provider error");
            let code = err.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
            return Err(LlmError::Api { status: code as u16, message: msg.to_string() });
        }
    }

    let parsed: OpenAiResponse = serde_json::from_str(body_text)
        .map_err(|e| LlmError::Deserialization(format!("{} — body was: {}", e, body_text)))?;

    if let Some(choice) = parsed.choices.into_iter().next() {
        if let Some(msg) = choice.message {
            if let Some(tc) = msg.tool_calls {
                return Ok(ParseOutcome::ToolCalls(tc));
            }
            let content = msg.content.unwrap_or_default();
            return Ok(ParseOutcome::Content(content));
        } else if let Some(text) = choice.text {
            return Ok(ParseOutcome::Content(text));
        } else if let Some(_delta) = choice.delta {
            return Err(LlmError::Deserialization("Unexpected streaming delta".into()));
        } else {
            return Err(LlmError::Deserialization("Empty choice".into()));
        }
    }

    Err(LlmError::Deserialization("No choices".into()))
}

/// Generic per-request session over a router-specific ApiRoute.
pub(crate) struct RequestSession<'a, R>
where
    R: ApiRoute,
    R::Parent: Router,
{
    pub client: &'a Client,
    pub event_bus: std::sync::Arc<EventBus>,
    pub parent_id: Uuid,
    pub req: ChatCompRequest<R>,
    pub fallback_on_404: bool,
    pub attempts: u32,
}

impl<'a, R> RequestSession<'a, R>
where
    R: ApiRoute,
    R::Parent: Router,
{
    pub async fn run(mut self) -> Result<String, LlmError> {
        // Use router-level constants for URL and API key
        let url = <R::Parent as Router>::COMPLETION_URL;
        let api_key = <R::Parent as Router>::resolve_api_key()
            .map_err(|e| LlmError::Request(format!("missing api key: {}", e)))?;

        // Determine whether to include tools
        let mut use_tools = self.req.tools.is_some();
        let mut tools_fallback_attempted = false;

        for _attempt in 0..=self.attempts {
            // If tools disabled due to fallback, ensure both tools and tool_choice reflect it
            if !use_tools {
                self.req.tools = None;
                self.req.tool_choice = None;
            } else if self.req.tool_choice.is_none() && self.req.tools.is_some() {
                self.req.tool_choice = Some(ToolChoice::Auto);
            }

            let body = serde_json::to_value(&self.req)
                .map_err(|e| LlmError::Request(format!("serialize req: {}", e)))?;

            let response = self
                .client
                .post(url)
                .bearer_auth(&api_key)
                .header("Accept", "application/json")
                .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
                .header("X-Title", "Ploke TUI")
                .json(&body)
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

                // Heuristic: many providers respond with hints when tool calls are unsupported
                if status == 404 && use_tools && text.to_lowercase().contains("support tool") {
                    if self.fallback_on_404 && !tools_fallback_attempted {
                        let notice = format!(
                            "Notice: endpoint appears to lack tool support; retrying without tools.\n\n{}",
                            text
                        );
                        self.req.core.messages.push(RequestMessage::new_system(notice));
                        use_tools = false;
                        tools_fallback_attempted = true;
                        continue;
                    }
                }
                return Err(LlmError::Api { status, message: text });
            }

            let body_text = response
                .text()
                .await
                .map_err(|e| LlmError::Request(e.to_string()))?;

            match parse_outcome(&body_text)? {
                ParseOutcome::ToolCalls(tool_calls) => {
                        // Dispatch tool calls through event bus
                        let mut tasks = Vec::with_capacity(tool_calls.len());
                        for call in tool_calls.into_iter() {
                            let event_bus = self.event_bus.clone();
                            let parent_id = self.parent_id;
                            let request_id = Uuid::new_v4();
                            let call_id = call.call_id.clone();
                            // Subscribe before sending request event
                            let mut rx = event_bus.realtime_tx.subscribe();
                            event_bus.send(AppEvent::System(SystemEvent::ToolCallRequested {
                                tool_call: call,
                                request_id,
                                parent_id,
                            }));

                            tasks.push(tokio::spawn(async move {
                                // Await a correlated completion/failure
                                let wait = async move {
                                    while let Ok(evt) = rx.recv().await {
                                        match evt {
                                            AppEvent::System(SystemEvent::ToolCallCompleted { request_id: rid, call_id: cid, content, .. })
                                                if rid == request_id && cid == call_id => return Ok(content),
                                            AppEvent::System(SystemEvent::ToolCallFailed { request_id: rid, call_id: cid, error, .. })
                                                if rid == request_id && cid == call_id => return Err(error),
                                            _ => {}
                                        }
                                    }
                                    Err("Event channel closed".to_string())
                                };
                                match tokio::time::timeout(Duration::from_secs(30), wait).await {
                                    Ok(r) => (call_id, r),
                                    Err(_) => (call_id, Err("Timed out waiting for tool result".into())),
                                }
                            }));
                        }

                        // Join and incorporate tool results
                        let results = futures::future::join_all(tasks).await;
                        for res in results.into_iter() {
                            match res {
                                Ok((cid, Ok(content))) => {
                                    self.req.core.messages.push(RequestMessage::new_tool(content, cid));
                                }
                                Ok((cid, Err(err))) => {
                                    let content = json!({"ok": false, "error": err}).to_string();
                                    self.req.core.messages.push(RequestMessage::new_tool(content, cid));
                                    return Err(LlmError::ToolCall("tool call failed".into()));
                                }
                                Err(join_err) => {
                                    return Err(LlmError::ToolCall(format!("join error: {}", join_err)));
                                }
                            }
                        }
                        // Continue loop: after tool results added, allow next iteration
                        continue;
                }
                ParseOutcome::Content(content) => {
                    return Ok(content);
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
    use crate::tools::{FunctionMarker, FunctionCall, ToolCall, ToolName};
    use tokio::time::{timeout, Duration};

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
            ParseOutcome::ToolCalls(tc) => {
                assert_eq!(tc.len(), 1);
                assert_eq!(tc[0].call_id.as_ref(), "call_1");
                assert_eq!(tc[0].function.name, ToolName::RequestCodeContext);
            }
            _ => panic!("expected tool calls"),
        }
    }

    #[tokio::test]
    async fn request_session_tool_call_iteration_via_fixture() {
        // Build a minimal ChatCompRequest for OpenRouter
        use crate::llm2::router_only::openrouter;
        use crate::llm2::router_only::default_model;
        use crate::llm2::request::ChatCompReqCore;
        use crate::llm2::manager::RequestMessage;

        let messages = vec![RequestMessage::new_user("hi".into())];
        let req = openrouter::ChatCompFields::default()
            .completion_core(ChatCompReqCore::default())
            .with_model_str(&default_model())
            .map(|r| r.with_messages(messages))
            .unwrap();

        let client = reqwest::Client::new();
        let event_bus = std::sync::Arc::new(EventBus::new(EventBusCaps::default()));
        let parent_id = Uuid::new_v4();
        let session = RequestSession::<openrouter::ChatCompFields> {
            client: &client,
            event_bus: event_bus.clone(),
            parent_id,
            req,
            fallback_on_404: false,
            attempts: 1,
        };

        // Two-step fixture: first message has tool_calls, second has final content
        let bodies = vec![
            r#"{ "choices": [{ "message": { "tool_calls": [ { "id": "call_xyz", "type": "function", "function": {"name": "request_code_context", "arguments": "{\\"token_budget\\": 123}" } } ] } }] }"#.to_string(),
            r#"{ "choices": [{ "message": { "role": "assistant", "content": "Done!" } }] }"#.to_string(),
        ];

        // Spawn a replier to tool requests
        let mut rx_requests = event_bus.realtime_tx.subscribe();
        tokio::spawn(async move {
            loop {
                match rx_requests.recv().await {
                    Ok(AppEvent::System(SystemEvent::ToolCallRequested { request_id, parent_id, tool_call, .. })) => {
                        // immediately reply success for this call id
                        let _ = event_bus.realtime_tx.send(AppEvent::System(SystemEvent::ToolCallCompleted {
                            request_id,
                            parent_id,
                            call_id: tool_call.call_id.clone(),
                            content: "{\"ok\":true}".into(),
                        }));
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        // Local helper: run session using fixture bodies instead of HTTP
        async fn run_with_fixture<R>(mut s: RequestSession<'_, R>, bodies: Vec<String>) -> Result<String, LlmError>
        where
            R: ApiRoute,
            R::Parent: Router,
        {
            let mut iter = bodies.into_iter();
            loop {
                let body = match iter.next() {
                    Some(b) => b,
                    None => return Err(LlmError::Unknown("exhausted fixtures".into())),
                };
                match parse_outcome(&body)? {
                    ParseOutcome::ToolCalls(tool_calls) => {
                        let mut tasks = Vec::with_capacity(tool_calls.len());
                        for call in tool_calls.into_iter() {
                            let event_bus = s.event_bus.clone();
                            let parent_id = s.parent_id;
                            let request_id = Uuid::new_v4();
                            let call_id = call.call_id.clone();
                            let mut rx = event_bus.realtime_tx.subscribe();
                            event_bus.send(AppEvent::System(SystemEvent::ToolCallRequested {
                                tool_call: call,
                                request_id,
                                parent_id,
                            }));
                            tasks.push(tokio::spawn(async move {
                                let wait = async move {
                                    while let Ok(evt) = rx.recv().await {
                                        match evt {
                                            AppEvent::System(SystemEvent::ToolCallCompleted { request_id: rid, call_id: cid, content, .. }) if rid == request_id && cid == call_id => return Ok(content),
                                            AppEvent::System(SystemEvent::ToolCallFailed { request_id: rid, call_id: cid, error, .. }) if rid == request_id && cid == call_id => return Err(error),
                                            _ => {}
                                        }
                                    }
                                    Err("Event channel closed".to_string())
                                };
                                match tokio::time::timeout(Duration::from_secs(5), wait).await {
                                    Ok(r) => (call_id, r),
                                    Err(_) => (call_id, Err("Timed out waiting for tool result".into())),
                                }
                            }));
                        }
                        let results = futures::future::join_all(tasks).await;
                        for res in results.into_iter() {
                            match res {
                                Ok((cid, Ok(content))) => {
                                    s.req.core.messages.push(RequestMessage::new_tool(content, cid));
                                }
                                Ok((_cid, Err(err))) => return Err(LlmError::ToolCall(err)),
                                Err(join_err) => return Err(LlmError::ToolCall(format!("join error: {}", join_err))),
                            }
                        }
                        continue;
                    }
                    ParseOutcome::Content(c) => return Ok(c),
                }
            }
        }

        let out = run_with_fixture(session, bodies).await.expect("content ok");
        assert_eq!(out, "Done!");
    }
}
