use std::collections::BTreeMap;

use ploke_core::ArcStr;

use super::*;
// --- Your existing CompReq<'a> is assumed available in scope ---
// use crate::llm::openrouter::CompReq;

// - see original json spec below:
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
#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct RequestMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<ArcStr>,
}

// note comment on RequestMessage above, need to be careful when calling tool to include the
// tool_id, or else we get an error as it is a required field.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

/// NOTE: STUB
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OaiChatReq {
    pub model: String,
    pub messages: Vec<RequestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    // add others as needed (frequency_penalty, etc.)
}

impl OaiChatReq {
    pub fn from_params(model_id: &str, messages: Vec<RequestMessage>, p: &LLMParameters) -> Self {
        Self {
            model: model_id.to_string(),
            messages,
            temperature: p.temperature,
            top_p: p.top_p,
            max_tokens: p.max_tokens,
        }
    }
}
