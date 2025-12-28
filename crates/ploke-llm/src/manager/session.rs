#![allow(
    dead_code,
    unused_variables,
    reason = "evolving api surface, may be useful, written 2025-12-15"
)]

use std::time::Duration;

use ploke_core::ArcStr;
use tracing::info;
use tracing::warn;

use crate::HTTP_REFERER;
use crate::HTTP_TITLE;
use crate::response::FinishReason;
use crate::response::OpenAiResponse;
use crate::response::ToolCall;
use crate::router_only::{ChatCompRequest, Router};

use super::LlmError;

#[derive(Debug, PartialEq)]
pub enum ChatStepOutcome {
    Content {
        content: Option<ArcStr>,
        reasoning: Option<ArcStr>,
    },
    ToolCalls {
        calls: Vec<ToolCall>,
        content: Option<ArcStr>,
        reasoning: Option<ArcStr>,
        finish_reason: FinishReason,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChatHttpConfig {
    referer: &'static str,
    title: &'static str,
    pub timeout: Duration,
}

impl Default for ChatHttpConfig {
    fn default() -> Self {
        Self {
            referer: HTTP_REFERER,
            title: HTTP_TITLE,
            // NOTE:ploke-llm 2025-12-14
            // Setting to 15 secs for now, try using it and getting a feel for the right default
            // timing
            timeout: Duration::from_secs(30),
        }
    }
}

pub async fn chat_step<R: Router>(
    client: &reqwest::Client,
    req: &ChatCompRequest<R>,
    cfg: &ChatHttpConfig,
) -> Result<ChatStepData, LlmError> {
    let url = R::COMPLETION_URL;
    let api_key = R::resolve_api_key().map_err(|e| LlmError::Request {
        message: format!("missing api key: {e}"),
        url: None,
        is_timeout: false,
    })?;

    let request_json = serde_json::to_string_pretty(req).ok();
    if let Some(body) = request_json.as_ref() {
        let _ = log_api_request_json(url, body);
    }
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .header("Accept", "application/json")
        .header("HTTP-Referer", cfg.referer)
        .header("X-Title", cfg.title)
        .json(req)
        .timeout(cfg.timeout)
        .send()
        .await
        .map_err(|e| LlmError::Request {
            message: format!("sending request to {url}: {e}"),
            url: Some(url.to_string()),
            is_timeout: e.is_timeout(),
        })?;

    let resp_url = resp.url().to_string();
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| LlmError::Request {
        message: format!("while reading response body (status {status}): {e}"),
        url: Some(resp_url.clone()),
        is_timeout: e.is_timeout(),
    })?;

    let _ = log_api_raw_response(&resp_url, status, &body);

    if let Ok(parsed) = &serde_json::from_str(&body) {
        let _ = log_api_parsed_json_response(&resp_url, status, parsed).await;
    } else {
        let _ = log_api_raw_response(url, status, &body);
    }

    if !(200..300).contains(&status) {
        return Err(LlmError::Api {
            status,
            message: body.clone(),
            url: Some(resp_url),
            body_snippet: Some(truncate_for_error(&body, 4_096)),
        });
    }

    parse_chat_outcome(&body)
}

async fn log_api_parsed_json_response(
    url: &str,
    status: u16,
    parsed: &OpenAiResponse,
) -> color_eyre::Result<()> {
    let payload: String = serde_json::to_string_pretty(parsed)?;
    tracing::info!(target: "api_json", "\n// URL: {url}\n// Status: {status}\n{payload}\n");
    Ok(())
}

fn log_api_raw_response(url: &str, status: u16, body: &str) -> color_eyre::Result<()> {
    tracing::info!(target: "api_json", "\n// URL: {url}\n// Status: {status}\n{body}\n");
    Ok(())
}

fn log_api_request_json(url: &str, payload: &str) -> color_eyre::Result<()> {
    tracing::info!(target: "api_json", "\n// URL: {url}\n// Request\n{payload}\n");
    Ok(())
}

#[derive(Debug)]
pub struct ChatStepData {
    pub outcome: ChatStepOutcome,
    pub full_response: OpenAiResponse,
}

#[derive(Debug)]
pub struct ChatStepDataBuilder {
    pub outcome: Option<ChatStepOutcome>,
    pub full_response: Option<OpenAiResponse>,
}

impl ChatStepDataBuilder {
    pub fn new() -> Self {
        Self {
            outcome: None,
            full_response: None,
        }
    }

    pub fn outcome(mut self, outcome: ChatStepOutcome) -> Self {
        self.outcome = Some(outcome);
        self
    }

    pub fn full_response(mut self, response: OpenAiResponse) -> Self {
        self.full_response = Some(response);
        self
    }

    pub fn build(self) -> Result<ChatStepData, LlmError> {
        let outcome = self
            .outcome
            .ok_or(LlmError::ChatStep("Outcome is required".to_string()))?;
        let full_response = self
            .full_response
            .ok_or(LlmError::ChatStep("Full response is required".to_string()))?;

        Ok(ChatStepData {
            outcome,
            full_response,
        })
    }
}

impl ChatStepData {
    pub fn new(outcome: ChatStepOutcome, full_response: OpenAiResponse) -> Self {
        Self {
            outcome,
            full_response,
        }
    }
}

/// Parse a (non-streaming) OpenAI/OpenRouter-style response body into a normalized outcome.
///
/// This function is used by the *driver* (session/tool loop) to decide what to do next:
/// - If the model produced tool calls, we return `ParseOutcome::ToolCalls` so the caller can
///   execute them and then continue the conversation.
/// - Otherwise we return `ParseOutcome::Content` containing the assistant text.
/// - Streaming deltas are not supported here; if you enable streaming, route those responses to a
///   different parser.
///
/// ## Finish reason normalization
/// Some providers:
/// - omit `finish_reason`, or
/// - incorrectly set it to `"stop"` even when `tool_calls` are present.
///
/// If `tool_calls` are present, we **force** `finish_reason = FinishReason::ToolCalls` because
/// that is the only safe interpretation for a tool-driving session loop.
///
/// ## Provider-embedded errors
/// Some providers return `{ "error": ... }` in a 200 OK body. We detect that early and surface it
/// as `LlmError::Api`.
pub fn parse_chat_outcome(body_text: &str) -> Result<ChatStepData, LlmError> {
    use serde_json::Value;
    let mut builder = ChatStepDataBuilder::new();

    // Parse once as JSON so we can cheaply detect embedded errors without double-deserializing.
    // If this fails, we still attempt typed parsing below to produce a more specific error.
    if let Ok(v) = serde_json::from_str::<Value>(body_text)
        && let Some(err) = v.get("error")
    {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown provider error");

        // Provider "code" is often not an HTTP status; it may be a string like "invalid_api_key".
        // Prefer an explicit `status` field if present, otherwise mark as 200 (embedded error).
        let status = err.get("status").and_then(|s| s.as_u64()).unwrap_or(200) as u16;

        let code_str = match err.get("code") {
            Some(Value::String(s)) => Some(s.to_string()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        };

        let full_msg = if let Some(code) = code_str {
            format!("{msg} (code: {code})")
        } else {
            msg.to_string()
        };

        return Err(LlmError::Api {
            status,
            message: full_msg,
            url: None,
            body_snippet: Some(truncate_for_error(body_text, 4_096)),
        });
    }

    let parsed: OpenAiResponse = serde_json::from_str(body_text).map_err(|e| {
        // Avoid dumping arbitrarily large bodies into errors/logs.
        let excerpt = truncate_for_error(body_text, 2_000);
        LlmError::Deserialization {
            message: format!("{e} — body excerpt: {excerpt}"),
            body_snippet: Some(excerpt),
        }
    })?;

    // We prefer the first choice that yields a usable outcome.
    for choice in parsed.choices.iter() {
        // Case 1: Chat-style `message`
        if let Some(msg) = &choice.message {
            let calls_opt = &msg.tool_calls;
            let content_opt = &msg.content;
            let reasoning_opt = &msg.reasoning;

            // Normalize: tool calls always win.
            if let Some(calls) = calls_opt {
                // If you care about empty tool_calls arrays, you can treat empty as an error.
                // Here, empty still counts as "tool calls" because the session loop expects it.
                // - however, still warn for the logs
                if choice.finish_reason != Some(FinishReason::ToolCalls) {
                    warn!(target: "chat-loop", "FinishReason is not ToolCalls when calling tools, found finish reason: {:?}", choice.finish_reason);
                }
                info!(target: "chat-loop", "native_finish_reason, type string, is not well-understood yet. Logging to learn more:{:?}", choice.native_finish_reason);
                let finish_reason = FinishReason::ToolCalls;
                let outcome = ChatStepOutcome::ToolCalls {
                    // TODO: Find a way to get rid of this clone
                    calls: calls.clone(),
                    content: content_opt.as_deref().map(ArcStr::from),
                    finish_reason,
                    reasoning: reasoning_opt.as_deref().map(ArcStr::from),
                };
                builder = builder.outcome(outcome).full_response(parsed);

                return builder.build();
            }

            // No tool calls → return content if present.
            if let Some(text) = content_opt {
                let outcome = ChatStepOutcome::Content {
                    reasoning: reasoning_opt.as_deref().map(ArcStr::from),
                    content: Some(ArcStr::from(text.as_str())),
                };
                return builder.outcome(outcome).full_response(parsed).build();
            }

            // If message exists but is empty, fall through to try other forms / choices.
            continue;
        }

        // Case 2: Legacy completions-style `text`
        if let Some(text) = &choice.text {
            let outcome = ChatStepOutcome::Content {
                reasoning: choice
                    .message
                    .as_ref()
                    .and_then(|m| m.reasoning.as_ref().map(|s| ArcStr::from(s.as_str()))),
                content: Some(ArcStr::from(text.as_str())),
            };
            return builder.outcome(outcome).full_response(parsed).build();
        }

        // Case 3: Streaming deltas (unsupported in this parser)
        if choice.delta.is_some() {
            return Err(LlmError::Deserialization {
                message: "Unexpected streaming delta in non-streaming parser".into(),
                body_snippet: Some(truncate_for_error(body_text, 512)),
            });
        }
    }

    Err(LlmError::Deserialization {
        message: "No usable choice in LLM response (no message/text/tool_calls)".into(),
        body_snippet: Some(truncate_for_error(body_text, 512)),
    })
}

/// Truncate large response bodies so error strings remain bounded.
fn truncate_for_error(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Preserve a little tail too (often contains the interesting part).
        let head = &s[..max.saturating_sub(200)];
        let tail = &s[s.len().saturating_sub(200)..];
        format!("{head}…<snip>…{tail}")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_outcome_content_message() {
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
    fn parse_outcome_text_field() {
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
}
