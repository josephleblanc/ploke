mod commands;
pub mod events;
mod session;
pub use session::{ChatHttpConfig, ChatStepData, ChatStepOutcome, chat_step, parse_chat_outcome};

use crate::error::LlmError;
use crate::manager::events::endpoint;
use crate::response::ToolCall;
use crate::router_only::HasEndpoint;

use ploke_core::ArcStr;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, fs, path::PathBuf, sync::Arc};
use uuid::Uuid;

use super::router_only::openrouter::OpenRouter;
use super::router_only::openrouter::OpenRouterModelId;
use super::*;

/// Trait for counting tokens. Implementations can be provided by consumers.
///
/// NOTE: This mirrors `ploke-rag`'s abstraction but lives in `ploke-llm` to avoid
/// dependency cycles (`ploke-embed -> ploke-llm -> ploke-rag -> ploke-embed`).
pub trait TokenCounter: Send + Sync + std::fmt::Debug {
    fn count(&self, text: &str) -> usize;
}

/// A simple, deterministic tokenizer suitable for approximate budgeting:
/// approximates tokens as ceil(chars / 4).
#[derive(Default, Debug)]
pub struct ApproxCharTokenizer;

impl TokenCounter for ApproxCharTokenizer {
    fn count(&self, text: &str) -> usize {
        text.chars().count().div_ceil(4)
    }
}

#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, PartialOrd)]
pub struct RequestMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<ArcStr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

// TODO: Add Role::Tool
// - be careful when adding `Tool`
// - note differences in the way the original json handles the `type Message` when there is a
// `role: 'tool'`, such that it requires a `tool_call_id`. We will need to propogate this
// requirement somehow. Needs HUMAN decision, ask.
// - see original json below:
// ```json
// type Message =
//   | {
//       role: 'user' | 'assistant' | 'system';
//       // ContentParts are only for the "user" role:
//       content: string | ContentPart[];
//       // If "name" is included, it will be prepended like this
//       // for non-OpenAI models: `{name}: {content}`
//       name?: string;
//     }
//   | {
//       role: 'tool';
//       content: string;
//       tool_call_id: string;
//       name?: string;
//     };
// ```
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl RequestMessage {
    pub fn new_system(content: String) -> Self {
        Self {
            role: Role::System,
            content,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn new_tool(content: String, tool_call_id: ArcStr) -> Self {
        Self {
            role: Role::Tool,
            content,
            tool_call_id: Some(tool_call_id),
            tool_calls: None,
        }
    }

    pub fn new_user(content: String) -> Self {
        Self {
            role: Role::User,
            content,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn new_assistant(content: String) -> Self {
        Self {
            role: Role::Assistant,
            content,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn new_assistant_with_tool_calls(
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        let placeholder_msg = "Calling tools...".to_string();
        let non_empty_content = match content {
            Some(s) => {
                if s.is_empty() {
                    placeholder_msg
                } else {
                    s
                }
            }
            None => placeholder_msg,
        };
        Self {
            role: Role::Assistant,
            content: non_empty_content,
            tool_call_id: None,
            tool_calls: Some(tool_calls),
        }
    }

    /// Validates that the message structure is correct according to OpenAI/OpenRouter spec
    pub fn validate(&self) -> Result<(), String> {
        match self.role {
            Role::Tool => {
                if self.tool_call_id.is_none() {
                    return Err("Tool messages must have a tool_call_id".to_string());
                }
            }
            Role::User | Role::Assistant | Role::System => {
                // These roles should not have tool_call_id set, but we allow it for flexibility
            }
        }
        Ok(())
    }
}

pub async fn handle_endpoint_request_async(
    client: Client,
    model_key: ModelKey,
    variant: Option<types::model_types::ModelVariant>,
) -> endpoint::Event {
    let model_id = ModelId::from_parts(model_key.clone(), variant);
    let typed_model = OpenRouterModelId::from(model_id);
    let result = OpenRouter::fetch_model_endpoints(&client, typed_model.clone())
        .await
        .map(Arc::new)
        .inspect_err(|e| {
            let msg = format!("Failed to fetch endpoints for {}: {:?}", typed_model, e);
            tracing::warn!(msg);
            // TODO: send a response with an error
        })
        .ok();
    endpoint::Event::Response {
        model_key,
        endpoints: result,
    }
}

#[allow(
    dead_code,
    reason = "useful later when we add better token tracking, probably"
)]
pub(super) fn cap_messages_by_chars(
    messages: &[RequestMessage],
    budget: usize,
) -> Vec<RequestMessage> {
    // Walk from the tail so we keep the most recent context, then reverse to restore order
    let mut used = 0usize;
    let mut kept: Vec<&RequestMessage> = Vec::new();
    for m in messages.iter().rev() {
        let len = m.content.len();
        if used.saturating_add(len) > budget && !kept.is_empty() {
            break;
        }
        used = used.saturating_add(len);
        kept.push(m);
    }
    kept.reverse();
    // Clone into a fresh vec; RequestMessage owns String content so zero-copy is not possible here
    kept.into_iter().cloned().collect()
}

#[allow(
    dead_code,
    reason = "useful later when we add better token tracking, probably"
)]
pub(super) fn cap_messages_by_tokens(
    messages: &[RequestMessage],
    token_budget: usize,
) -> Vec<RequestMessage> {
    // Use shared TokenCounter to approximate tokens deterministically
    let tokenizer = ApproxCharTokenizer;
    let mut used = 0usize;
    let mut kept: Vec<&RequestMessage> = Vec::new();
    for m in messages.iter().rev() {
        let tokens = tokenizer.count(&m.content);
        if used.saturating_add(tokens) > token_budget && !kept.is_empty() {
            break;
        }
        used = used.saturating_add(tokens);
        kept.push(m);
    }
    kept.reverse();
    kept.into_iter().cloned().collect()
}

// Diagnostics helpers (env-driven, independent of tracing)
#[allow(
    dead_code,
    reason = "possibly useful for adding log files to tests here"
)]
fn diag_dir() -> Option<PathBuf> {
    // Prefer explicit env override; otherwise default to a stable test-output folder.
    let path = env::var_os("PLOKE_E2E_DIAG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/test-output/openrouter_e2e"));
    let _ = fs::create_dir_all(&path);
    Some(path)
}
#[allow(
    dead_code,
    reason = "possibly useful for adding log files to tests here"
)]
fn now_ts() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

// Legacy diagnostics helpers referencing ModelConfig removed; llm routes via registry prefs.

// Example tool-call handler (stub)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_tool_serialization() {
        // Test that Role::Tool serializes correctly
        let role = Role::Tool;
        let serialized = serde_json::to_string(&role).unwrap();
        assert_eq!(serialized, "\"tool\"");

        // Test deserialization
        let deserialized: Role = serde_json::from_str("\"tool\"").unwrap();
        assert_eq!(deserialized, Role::Tool);
    }

    #[test]
    fn test_tool_message_constructors() {
        // Test new_tool constructor
        let call_123 = ArcStr::from("call_123");
        let tool_msg = RequestMessage::new_tool("result content".to_string(), call_123.clone());
        assert_eq!(tool_msg.role, Role::Tool);
        assert_eq!(tool_msg.content, "result content");
        assert_eq!(tool_msg.tool_call_id, Some(call_123));

        // Test validation passes for valid tool message
        assert!(tool_msg.validate().is_ok());

        // Test other constructors don't have tool_call_id
        let user_msg = RequestMessage::new_user("hello".to_string());
        assert_eq!(user_msg.role, Role::User);
        assert_eq!(user_msg.tool_call_id, None);
        assert!(user_msg.validate().is_ok());
    }

    #[test]
    fn test_tool_message_validation() {
        // Valid tool message
        let call_id = ArcStr::from("call_id");
        let valid_tool = RequestMessage::new_tool("content".to_string(), call_id.clone());
        assert!(valid_tool.validate().is_ok());

        // Invalid tool message (missing tool_call_id)
        let invalid_tool = RequestMessage {
            role: Role::Tool,
            content: "content".to_string(),
            tool_call_id: None,
            tool_calls: None,
        };
        assert!(invalid_tool.validate().is_err());
        assert!(
            invalid_tool
                .validate()
                .unwrap_err()
                .contains("tool_call_id")
        );
    }

    #[test]
    fn test_tool_message_serialization() {
        let call_id = ArcStr::from("call_abc");
        let tool_msg = RequestMessage::new_tool("test result".to_string(), call_id);
        let serialized = serde_json::to_string(&tool_msg).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["role"], "tool");
        assert_eq!(parsed["content"], "test result");
        assert_eq!(parsed["tool_call_id"], "call_abc");
    }

    #[test]
    fn test_cap_messages_by_chars_keeps_latest_even_if_over_budget() {
        let m1 = RequestMessage::new_user("a".into()); // 1
        let m2 = RequestMessage::new_user("bb".into()); // 2
        let m3 = RequestMessage::new_user("ccc".into()); // 3
        let m4 = RequestMessage::new_user("dddd".into()); // 4 (tail)
        let all = vec![m1, m2, m3, m4.clone()];

        // Budget smaller than last message; policy keeps at least the most recent
        let kept = cap_messages_by_chars(&all, 3);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].content, m4.content);

        // Budget that fits last two (3 + 4 > 6), but logic walks tail-first and stops when over
        let kept2 = cap_messages_by_chars(&all, 7);
        // Tail first (4), then preceding (3) fits → 4 + 3 = 7
        assert_eq!(kept2.len(), 2);
        assert_eq!(kept2[0].content, "ccc");
        assert_eq!(kept2[1].content, "dddd");
    }

    #[test]
    fn test_cap_messages_by_tokens_behaves_reasonably() {
        // We cannot assert exact token counts without knowing tokenizer internals,
        // but we can validate ordering and non-empty behavior.
        let m1 = RequestMessage::new_user("short".into());
        let m2 = RequestMessage::new_user("a bit longer".into());
        let m3 = RequestMessage::new_user("the longest content in this small set".into());
        let all = vec![m1.clone(), m2.clone(), m3.clone()];

        // With a tiny budget, we still keep at least the latest
        let kept = cap_messages_by_tokens(&all, 1);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].content, m3.content);

        // With a generous budget, we keep all in original order
        let kept_all = cap_messages_by_tokens(&all, 10_000);
        assert_eq!(kept_all.len(), 3);
        assert_eq!(kept_all[0].content, m1.content);
        assert_eq!(kept_all[1].content, m2.content);
        assert_eq!(kept_all[2].content, m3.content);
    }
}
