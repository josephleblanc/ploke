use std::time::Duration;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::AppEvent;
use crate::system::SystemEvent;

// --- RequestSession: extracted per-request loop (Milestone 2 partial) ---

use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;

use super::{
    cap_messages_by_chars, GenericToolCall, LLMParameters, LlmError, OpenAiRequest, RequestMessage,
    ToolDefinition, ToolVendor,
};
use super::tool_call;
use crate::EventBus;

/// Owns the lifecycle of a single LLM request/response, including tool-call cycles.
pub struct RequestSession<'a> {
    client: &'a Client,
    provider: &'a crate::user_config::ProviderConfig,
    event_bus: Arc<EventBus>,
    parent_id: Uuid,
    messages: Vec<RequestMessage<'a>>,
    tools: Vec<ToolDefinition>,
    params: LLMParameters,
    attempts: u32,
}

impl<'a> RequestSession<'a> {
    pub fn new(
        client: &'a Client,
        provider: &'a crate::user_config::ProviderConfig,
        event_bus: Arc<EventBus>,
        parent_id: Uuid,
        messages: Vec<RequestMessage<'a>>,
        tools: Vec<ToolDefinition>,
        params: LLMParameters,
    ) -> Self {
        Self {
            client,
            provider,
            event_bus,
            parent_id,
            messages,
            tools,
            params,
            attempts: 0,
        }
    }

    /// Execute the request loop until completion or error.
    pub async fn run(mut self) -> Result<String, LlmError> {
        let max_retries: u32 = self.params.tool_max_retries.unwrap_or(2);

        loop {
            let history_budget_chars: usize = if let Some(budget) = self.params.history_char_budget {
                budget
            } else {
                self
                    .params
                    .max_tokens
                    .map(|t| (t as usize).saturating_mul(4))
                    .unwrap_or(12000)
            };

            let effective_messages = cap_messages_by_chars(&self.messages, history_budget_chars);

            let request_payload = OpenAiRequest {
                model: self.provider.model.as_str(),
                messages: effective_messages,
                temperature: self.params.temperature,
                max_tokens: self.params.max_tokens,
                top_p: self.params.top_p,
                stream: false,
                tools: Some(self.tools.clone()),
                tool_choice: Some("auto".to_string()),
            };

            let response = self
                .client
                .post(format!("{}/chat/completions", self.provider.base_url))
                .bearer_auth(self.provider.resolve_api_key())
                .json(&request_payload)
                .send()
                .await
                .map_err(|e| LlmError::Request(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Could not retrieve error body".to_string());

                let user_friendly_msg = if status == 401 {
                    format!(
                        "Authentication failed. Please check your API key configuration.\n\nDetails {}",
                        error_text
                    )
                } else if status == 429 {
                    format!(
                        "Rate limit exceeded. Please wait and try again.\n\nDetails: {}",
                        error_text
                    )
                } else if status >= 500 {
                    format!(
                        "Server error. The API provider may be experiencing issues.\n\nDetails: {}",
                        error_text
                    )
                } else {
                    format!("API error (status {}): {}", status, error_text)
                };

                return Err(LlmError::Api {
                    status,
                    message: user_friendly_msg,
                });
            }

            let body = response
                .text()
                .await
                .map_err(|e| LlmError::Request(e.to_string()))?;
            tracing::debug!("raw body: {}", body);

            // Providers sometimes put errors inside a 200 body
            if let Ok(err) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(err_obj) = err.get("error") {
                    let msg = err_obj
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown provider error");
                    let code = err_obj.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
                    return Err(LlmError::Api {
                        status: code as u16,
                        message: msg.to_string(),
                    });
                }
            }

            let response_body: super::OpenAiResponse = serde_json::from_str(&body)
                .map_err(|e| LlmError::Deserialization(format!("{} — body was: {}", e, body)))?;
            tracing::trace!("raw body response to request: {:#?}", body);

            if let Some(choice) = response_body.choices.into_iter().next() {
                let resp_msg = choice.message;

                if let Some(tool_calls) = resp_msg.tool_calls {
                    self.attempts += 1;

                    // Build specs for supported (OpenAI) tool calls; collect unsupported for immediate error
                    let mut specs: Vec<tool_call::ToolCallSpec> = Vec::new();
                    let mut other_errors: Vec<(String, String)> = Vec::new(); // (call_id, error_json)

                    for call in tool_calls {
                        match call {
                            GenericToolCall::OpenAi(oc) => {
                                tracing::info!(
                                    "OpenAI tool call received: id={}, type={}, fn={}, args={}",
                                    oc.id,
                                    oc.r#type,
                                    oc.function.name,
                                    oc.function.arguments
                                );
                                let arguments = serde_json::from_str::<Value>(&oc.function.arguments)
                                    .unwrap_or(json!({ "raw": oc.function.arguments }));
                                specs.push(tool_call::ToolCallSpec {
                                    name: oc.function.name.clone(),
                                    arguments,
                                    call_id: oc.id.clone(),
                                    vendor: ToolVendor::OpenAI,
                                });
                            }
                            GenericToolCall::Other(v) => {
                                tracing::warn!("Received non-OpenAI tool call payload: {}", v);
                                let call_id = v
                                    .get("id")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let err_json = json!({
                                    "ok": false,
                                    "error": "Unsupported tool call format",
                                    "vendor": "other"
                                })
                                .to_string();
                                other_errors.push((call_id, err_json));
                            }
                        }
                    }

                    // Execute supported tool calls concurrently and append in stable order by call_id
                    if !specs.is_empty() {
                        let outcomes =
                            tool_call::execute_tool_calls(
                                &self.event_bus,
                                self.parent_id,
                                specs,
                                self.params.tool_timeout_secs.unwrap_or(30),
                            )
                            .await;
                        for (spec, result) in outcomes {
                            match result {
                                Ok(content) => {
                                    self.messages
                                        .push(RequestMessage::new_tool(content, spec.call_id));
                                }
                                Err(err) => {
                                    let sys_msg =
                                        format!("Tool call '{}' failed: {}.", spec.name, err);
                                    self.messages.push(RequestMessage::new_system(sys_msg));
                                    self.messages.push(RequestMessage::new_tool(
                                        json!({"ok": false, "error": err}).to_string(),
                                        spec.call_id,
                                    ));
                                }
                            }
                        }
                    }

                    // Handle unsupported/other formats as immediate failures
                    for (call_id, err_json) in other_errors {
                        self.messages.push(RequestMessage::new_system(
                            "Unsupported tool call format".to_string(),
                        ));
                        self.messages.push(RequestMessage::new_tool(err_json, call_id));
                    }

                    if self.attempts > max_retries {
                        return Err(LlmError::Unknown(format!(
                            "Tool call retries exhausted after {} attempt(s)",
                            self.attempts
                        )));
                    }

                    // Continue loop to let the model observe tool outputs and respond
                    continue;
                }

                // No tool calls; finalize content
                let content = resp_msg
                    .content
                    .unwrap_or_else(|| "No content received from API.".to_string());
                return Ok(content);
            } else {
                return Err(LlmError::Deserialization(
                    "No choices in response".to_string(),
                ));
            }
        }
    }
}

/// Await a correlated ToolCall completion/failure on the realtime broadcast channel.
///
/// - `rx`: a subscribed `broadcast::Receiver<AppEvent>` (must be subscribed before the request is emitted)
/// - `request_id`: the UUID assigned to this tool call request
/// - `call_id`: provider-assigned tool call id (string)
/// - `timeout_secs`: how many seconds to wait before returning a timeout error
///
/// Returns Ok(content) when ToolCallCompleted is received with matching (request_id, call_id),
/// or Err(error_string) when ToolCallFailed or other failure occurs (including timeout).
pub async fn await_tool_result(
    mut rx: broadcast::Receiver<AppEvent>,
    request_id: Uuid,
    call_id: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    let wait = async {
        loop {
            match rx.recv().await {
                Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id: rid,
                    call_id: cid,
                    content,
                    ..
                })) if rid == request_id && cid == call_id => {
                    break Ok(content);
                }
                Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id: rid,
                    call_id: cid,
                    error,
                    ..
                })) if rid == request_id && cid == call_id => {
                    break Err(error);
                }
                Ok(_) => {
                    // unrelated event; keep waiting
                }
                Err(e) => {
                    break Err(format!("Event channel error: {}", e));
                }
            }
        }
    };

    match tokio::time::timeout(Duration::from_secs(timeout_secs), wait).await {
        Ok(Ok(content)) => Ok(content),
        Ok(Err(err)) => Err(err),
        Err(_) => Err("Timed out waiting for tool result".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use tokio::time::sleep;

    use crate::EventBus;
    use crate::EventBusCaps;
    use crate::AppEvent;
    use crate::system::SystemEvent;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_await_tool_result_completed() {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let rx = event_bus.realtime_tx.subscribe();
        let request_id = Uuid::new_v4();
        let call_id = "call-123".to_string();
        let content = "tool response".to_string();
        let eb = event_bus.clone();

        // spawn sender that emits completion shortly after
        let call_id_for_task = call_id.clone();
        let content_for_task = content.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            eb.send(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id,
                parent_id: Uuid::new_v4(),
                call_id: call_id_for_task,
                content: content_for_task,
            }));
        });

        let res = await_tool_result(rx, request_id, &call_id, 5).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), content);
    }

    #[tokio::test]
    async fn test_await_tool_result_failed() {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let rx = event_bus.realtime_tx.subscribe();
        let request_id = Uuid::new_v4();
        let call_id = "call-err".to_string();
        let error_msg = "something went wrong".to_string();
        let eb = event_bus.clone();

        let call_id_for_task = call_id.clone();
        let error_msg_for_task = error_msg.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            eb.send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id: Uuid::new_v4(),
                call_id: call_id_for_task,
                error: error_msg_for_task,
            }));
        });

        let res = await_tool_result(rx, request_id, &call_id, 5).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), error_msg);
    }
}
