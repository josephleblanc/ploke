use std::time::Duration;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::AppEvent;
use crate::llm::ToolEvent;
use crate::system::SystemEvent;

// --- RequestSession: extracted per-request loop (Milestone 2 partial) ---

use reqwest::Client;
use serde_json::{Value, json};
use std::sync::Arc;

use super::tool_call;
use super::{
    GenericToolCall, LLMParameters, LlmError, OpenAiRequest, RequestMessage, ToolDefinition,
    ToolVendor, cap_messages_by_chars, cap_messages_by_tokens,
};
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
    fallback_on_404: bool,
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
        fallback_on_404: bool,
    ) -> Self {
        Self {
            client,
            provider,
            event_bus,
            parent_id,
            messages,
            tools,
            params,
            fallback_on_404,
            attempts: 0,
        }
    }

    /// Execute the request loop until completion or error.
    pub async fn run(mut self) -> Result<String, LlmError> {
        let span = tracing::info_span!("request_session_run", model = %self.provider.model);
        let _enter = span.enter();
        tracing::debug!(?self.params, "Starting RequestSession::run");
        let max_retries: u32 = self.params.tool_max_retries.unwrap_or(2);
        // Some OpenRouter provider endpoints don't support tool calls even if the model does.
        // Start with tools if configured, but be ready to retry once without tools on a 404 error.
        let mut use_tools: bool = !self.tools.is_empty();
        let mut tools_fallback_attempted = false;

        loop {
            let effective_messages = if let Some(budget_chars) = self.params.history_char_budget {
                cap_messages_by_chars(&self.messages, budget_chars)
            } else {
                let budget_tokens = self.params.max_tokens.map(|t| t as usize).unwrap_or(3000);
                cap_messages_by_tokens(&self.messages, budget_tokens)
            };

            let request_payload = build_openai_request(
                self.provider,
                effective_messages,
                &self.params,
                if use_tools {
                    Some(self.tools.clone())
                } else {
                    None
                },
                use_tools,
            );

            // Brief, structured dispatch log for efficient triage
            let tool_names: Vec<&str> = self.tools.iter().map(|t| t.function.name).collect();
            tracing::info!(
                model = %self.provider.model,
                base_url = %self.provider.base_url,
                provider_slug = ?self.provider.provider_slug,
                use_tools = use_tools,
                tools = %tool_names.join(","),
                "dispatch_request"
            );

            let response = self
                .client
                .post(format!("{}/chat/completions", self.provider.base_url))
                .bearer_auth(self.provider.resolve_api_key())
                .json(&request_payload)
                .timeout(Duration::from_secs(45))
                .send()
                .await
                .map_err(|e| LlmError::Request(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Could not retrieve error body".to_string());

                // Deterministic enforcement for tool use: fail fast when endpoint lacks tool support.
                // Example provider body: {"error":{"message":"No endpoints found that support tool use.", "code":404}}
                if status == 404 && use_tools && error_text.to_lowercase().contains("support tool")
                {
                    tracing::warn!(
                        model = %self.provider.model,
                        provider_slug = ?self.provider.provider_slug,
                        "tool_unsupported_endpoint: {}",
                        error_text
                    );

                    let mut guidance = String::new();
                    guidance.push_str("Selected endpoint appears to lack tool support.\n\n");
                    guidance.push_str("Remediation steps:\n");
                    guidance.push_str(&format!(
                        "  1) List tool-capable endpoints for this model:\n     :model providers {}\n",
                        self.provider.model
                    ));
                    guidance.push_str(&format!(
                        "  2) Pin a specific provider endpoint (slug shown in step 1):\n     :provider pin {} <provider_slug>\n",
                        self.provider.model
                    ));
                    guidance.push_str("  3) If you intentionally want to continue without tools, disable enforcement:\n     :provider tools-only off\n\n");
                    guidance.push_str(&format!("Details: {}", error_text));

                    // Fallback once without tools: inform the model via a system message, then retry (if allowed by policy).
                    if self.fallback_on_404 && !tools_fallback_attempted {
                        self.messages.push(RequestMessage::new_system(format!(
                            "Notice: provider endpoint appears to lack tool support; retrying without tools.\n\n{}",
                            guidance
                        )));
                        tools_fallback_attempted = true;
                        use_tools = false;
                        continue;
                    }

                    return Err(LlmError::Api {
                        status,
                        message: guidance,
                    });
                }

                tracing::warn!(status = status, model = %self.provider.model, "api_error_body: {}", error_text);

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
                                let arguments =
                                    serde_json::from_str::<Value>(&oc.function.arguments)
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
                        let outcomes = tool_call::execute_tool_calls(
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
                        self.messages
                            .push(RequestMessage::new_tool(err_json, call_id));
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
pub fn build_openai_request<'a>(
    provider: &'a crate::user_config::ProviderConfig,
    messages: Vec<super::RequestMessage<'a>>,
    params: &super::LLMParameters,
    tools: Option<Vec<super::ToolDefinition>>,
    use_tools: bool,
) -> super::OpenAiRequest<'a> {
    tracing::trace!(model = %provider.model, use_tools = use_tools, messages = messages.len(), "build_openai_request");
    // NOTE: OpenRouter routing: prefer minimal provider preference shape on chat/completions.
    // We include `provider: { \"order\": [\"<slug>\"] }` when a valid provider_slug is set.

    super::OpenAiRequest {
        model: provider.model.as_str(),
        messages,
        temperature: params.temperature,
        max_tokens: params.max_tokens,
        top_p: params.top_p,
        stream: false,
        tools: if use_tools { tools } else { None },
        tool_choice: if use_tools {
            Some("auto".to_string())
        } else {
            None
        },
        provider: if use_tools {
            provider.provider_slug.as_ref().and_then(|slug| {
                let s = slug.trim();
                if s.is_empty() || s == "-" {
                    None
                } else {
                    Some(super::ProviderPreferences {
                        order: vec![s.to_string()],
                        ..Default::default()
                    })
                }
            })
        } else {
            None
        },
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
    let span =
        tracing::info_span!("await_tool_result", request_id = %request_id, call_id = %call_id);
    let _enter = span.enter();
    tracing::debug!("Awaiting tool result");
    let wait = async {
        loop {
            match rx.recv().await {
                Ok(AppEvent::LlmTool(ToolEvent::Completed {
                    request_id: rid,
                    call_id: cid,
                    content,
                    ..
                })) if rid == request_id && cid == call_id => {
                    break Ok(content);
                }
                Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id: rid,
                    call_id: cid,
                    content,
                    ..
                })) if rid == request_id && cid == call_id => {
                    break Ok(content);
                }
                Ok(AppEvent::LlmTool(ToolEvent::Failed {
                    request_id: rid,
                    call_id: cid,
                    error,
                    ..
                })) if rid == request_id && cid == call_id => {
                    break Err(error);
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

    use crate::AppEvent;
    use crate::EventBus;
    use crate::EventBusCaps;
    use crate::llm::ToolFunctionDef;
    use crate::system::SystemEvent;
    use uuid::Uuid;

    // Simple test logger with a rolling window of 3 files in target/test-logs
    fn init_test_logging(tag: &str) {
        static INIT: once_cell::sync::OnceCell<()> = once_cell::sync::OnceCell::new();
        INIT.get_or_init(|| {
            let dir = "target/test-logs";
            let _ = std::fs::create_dir_all(dir);

            prune_old_logs(dir, "llm_session_test-", 3);

            let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
            let path = format!("{}/{}{}.log", dir, "llm_session_test-", ts);

            if let Ok(file) = std::fs::File::create(&path) {
                let (nb, guard) = tracing_appender::non_blocking(file);
                static GUARD: once_cell::sync::OnceCell<
                    tracing_appender::non_blocking::WorkerGuard,
                > = once_cell::sync::OnceCell::new();
                let _ = GUARD.set(guard);

                let subscriber = tracing_subscriber::fmt()
                    .with_writer(nb)
                    .with_ansi(false)
                    .with_max_level(tracing::Level::DEBUG)
                    .finish();
                let _ = tracing::subscriber::set_global_default(subscriber);
            }
        });

        tracing::info!("Initialized test logging: {}", tag);
    }

    fn prune_old_logs(dir: &str, prefix: &str, keep: usize) {
        use std::fs;
        use std::time::SystemTime;

        let mut entries: Vec<(String, SystemTime)> = Vec::new();
        if let Ok(read) = fs::read_dir(dir) {
            for e in read.flatten() {
                if let Ok(file_name) = e.file_name().into_string() {
                    if file_name.starts_with(prefix) && file_name.ends_with(".log") {
                        if let Ok(md) = e.metadata() {
                            let mtime = md.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                            entries.push((file_name, mtime));
                        }
                    }
                }
            }
        }

        entries.sort_by(|a, b| b.1.cmp(&a.1));
        for (idx, (name, _)) in entries.iter().enumerate() {
            if idx >= keep {
                let _ = std::fs::remove_file(format!("{}/{}", dir, name));
            }
        }
    }

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

    #[test]
    fn test_build_request_regular_chat_snapshot() {
        init_test_logging("regular");
        // Arrange: provider and parameters for qwen model without tools
        let provider = crate::user_config::ProviderConfig {
            id: "qwen-72b".to_string(),
            api_key: "".to_string(),
            provider_slug: Some("openrouter".to_string()),
            api_key_env: None,
            base_url: crate::user_config::OPENROUTER_URL.to_string(),
            model: "qwen/qwen-2.5-72b-instruct".to_string(),
            display_name: Some("qwen/qwen-2.5-72b-instruct".to_string()),
            provider_type: crate::user_config::ProviderType::OpenRouter,
            llm_params: None,
        };

        let params = super::LLMParameters {
            model: provider.model.clone(),
            temperature: Some(0.2),
            max_tokens: Some(256),
            top_p: None,
            presence_penalty: None,
            frequency_penalty: None,
            stop_sequences: vec![],
            parallel_tool_calls: true,
            response_format: Default::default(),
            safety_settings: Default::default(),
            system_prompt: Some("You are helpful.".to_string()),
            tool_max_retries: Some(2),
            tool_token_limit: Some(2048),
            history_char_budget: Some(12000),
            tool_timeout_secs: Some(30),
        };

        let messages = vec![
            super::RequestMessage {
                role: "system",
                content: "You are helpful.".to_string(),
                tool_call_id: None,
            },
            super::RequestMessage {
                role: "user",
                content: "Hello!".to_string(),
                tool_call_id: None,
            },
        ];

        // Act: build request without tools
        let payload = super::build_openai_request(&provider, messages, &params, None, false);
        let json = serde_json::to_string_pretty(&payload).unwrap();

        // Snapshot: expected payload (stable field order)
        let expected = r#"{
  "model": "qwen/qwen-2.5-72b-instruct",
  "messages": [
    {
      "role": "system",
      "content": "You are helpful."
    },
    {
      "role": "user",
      "content": "Hello!"
    }
  ],
  "temperature": 0.2,
  "max_tokens": 256,
  "stream": false
}"#;

        assert_eq!(json, expected);
    }

    #[test]
    fn test_build_request_tool_call_snapshot() {
        init_test_logging("tool-call");
        // Arrange: provider and parameters for qwen model with tools
        let provider = crate::user_config::ProviderConfig {
            id: "qwen-72b".to_string(),
            api_key: "".to_string(),
            provider_slug: Some("openrouter".to_string()),
            api_key_env: None,
            base_url: crate::user_config::OPENROUTER_URL.to_string(),
            model: "qwen/qwen-2.5-72b-instruct".to_string(),
            display_name: Some("qwen/qwen-2.5-72b-instruct".to_string()),
            provider_type: crate::user_config::ProviderType::OpenRouter,
            llm_params: None,
        };

        let params = super::LLMParameters {
            model: provider.model.clone(),
            temperature: Some(0.0),
            max_tokens: Some(128),
            top_p: None,
            presence_penalty: None,
            frequency_penalty: None,
            stop_sequences: vec![],
            parallel_tool_calls: true,
            response_format: Default::default(),
            safety_settings: Default::default(),
            system_prompt: Some("You can call tools.".to_string()),
            tool_max_retries: Some(2),
            tool_token_limit: Some(2048),
            history_char_budget: Some(12000),
            tool_timeout_secs: Some(30),
        };

        let messages = vec![
            super::RequestMessage {
                role: "system",
                content: "You can call tools.".to_string(),
                tool_call_id: None,
            },
            super::RequestMessage {
                role: "user",
                content: "Please fetch context.".to_string(),
                tool_call_id: None,
            },
        ];

        let tool = super::ToolDefinition {
            r#type: "function",
            function: ToolFunctionDef {
                name: "dummy_tool",
                description: "Return a fixed string",
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "arg": { "type": "string" }
                    },
                    "required": ["arg"]
                }),
            },
        };

        // Act: build request with tools and auto tool_choice
        let payload =
            super::build_openai_request(&provider, messages, &params, Some(vec![tool]), true);
        let json = serde_json::to_string_pretty(&payload).unwrap();

        // Snapshot: expected payload (stable field order)
        let expected = r#"{
  "model": "qwen/qwen-2.5-72b-instruct",
  "messages": [
    {
      "role": "system",
      "content": "You can call tools."
    },
    {
      "role": "user",
      "content": "Please fetch context."
    }
  ],
  "temperature": 0.0,
  "max_tokens": 128,
  "stream": false,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "dummy_tool",
        "description": "Return a fixed string",
        "parameters": {
          "properties": {
            "arg": {
              "type": "string"
            }
          },
          "required": [
            "arg"
          ],
          "type": "object"
        }
      }
    }
  ],
  "tool_choice": "auto",
  "provider": {
    "order": [
      "openrouter"
    ]
  }
}"#;

        assert_eq!(json, expected);
    }
}
