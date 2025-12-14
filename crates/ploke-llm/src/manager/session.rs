use std::time::Duration;

use crate::response::FinishReason;
use crate::response::OpenAiResponse;
use crate::response::ToolCall;
use crate::router_only::{ChatCompRequest, Router};

use super::LlmError;

#[derive(Debug, PartialEq)]
pub enum ChatStepOutcome {
    Content(String),
    ToolCalls {
        calls: Vec<ToolCall>,
        content: Option<String>,
        finish_reason: FinishReason,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChatHttpConfig {
    referer: &'static str,
    title: &'static str,
    timeout: Duration,
}

impl Default for ChatHttpConfig {
    fn default() -> Self {
        Self {
            referer: HTTP_REFERER,
            title: HTTP_TITLE,
            timeout: Duration::from_secs(10),
        }
    }
}

pub const HTTP_REFERER: &str = "https://github.com/ploke-ai/ploke";
pub const HTTP_TITLE: &str = "Ploke TUI";

pub async fn chat_step<R: Router>(
    client: &reqwest::Client,
    req: &ChatCompRequest<R>,
    cfg: &ChatHttpConfig,
) -> Result<ChatStepOutcome, LlmError> {
    let url = R::COMPLETION_URL;
    let api_key =
        R::resolve_api_key().map_err(|e| LlmError::Request(format!("missing api key: {e}")))?;

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
        .map_err(|e| LlmError::Request(e.to_string()))?;

    let status = resp.status().as_u16();
    let body = resp
        .text()
        .await
        .map_err(|e| LlmError::Request(e.to_string()))?;

    if !(200..300).contains(&status) {
        return Err(LlmError::Api {
            status,
            message: body,
        });
    }

    parse_chat_outcome(&body)
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
pub fn parse_chat_outcome(body_text: &str) -> Result<ChatStepOutcome, LlmError> {
    use serde_json::Value;

    // Parse once as JSON so we can cheaply detect embedded errors without double-deserializing.
    // If this fails, we still attempt typed parsing below to produce a more specific error.
    if let Ok(v) = serde_json::from_str::<Value>(body_text) {
        if let Some(err) = v.get("error") {
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
            });
        }
    }

    let parsed: OpenAiResponse = serde_json::from_str(body_text).map_err(|e| {
        // Avoid dumping arbitrarily large bodies into errors/logs.
        let excerpt = truncate_for_error(body_text, 2_000);
        LlmError::Deserialization(format!("{e} — body excerpt: {excerpt}"))
    })?;

    // We prefer the first choice that yields a usable outcome.
    for choice in parsed.choices.into_iter() {
        // Case 1: Chat-style `message`
        if let Some(msg) = choice.message {
            let calls_opt = msg.tool_calls;
            let content_opt = msg.content;

            // Normalize: tool calls always win.
            if let Some(calls) = calls_opt {
                // If you care about empty tool_calls arrays, you can treat empty as an error.
                // Here, empty still counts as "tool calls" because the session loop expects it.
                let finish_reason = FinishReason::ToolCalls;
                return Ok(ChatStepOutcome::ToolCalls {
                    calls,
                    content: content_opt,
                    finish_reason,
                });
            }

            // No tool calls → return content if present.
            if let Some(text) = content_opt {
                return Ok(ChatStepOutcome::Content(text));
            }

            // If message exists but is empty, fall through to try other forms / choices.
            continue;
        }

        // Case 2: Legacy completions-style `text`
        if let Some(text) = choice.text {
            return Ok(ChatStepOutcome::Content(text));
        }

        // Case 3: Streaming deltas (unsupported in this parser)
        if choice.delta.is_some() {
            return Err(LlmError::Deserialization(
                "Unexpected streaming delta in non-streaming parser".into(),
            ));
        }
    }

    Err(LlmError::Deserialization(
        "No usable choice in LLM response (no message/text/tool_calls)".into(),
    ))
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
        match r {
            ChatStepOutcome::Content(c) => assert_eq!(c, "Hello world"),
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
        match r {
            ChatStepOutcome::Content(c) => assert_eq!(c, "Hello text"),
            _ => panic!("expected content"),
        }
    }
}
