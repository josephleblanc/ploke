use crate::utils::se_de::{de_arc_str, se_arc_str};
use ploke_core::{
    ArcStr,
    tool_types::{FunctionMarker, ToolName},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct ToolCall {
    #[serde(
        deserialize_with = "de_arc_str",
        serialize_with = "se_arc_str",
        rename = "id"
    )]
    pub call_id: ArcStr,

    #[serde(rename = "type")]
    pub call_type: FunctionMarker,
    pub function: FunctionCall,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_content: Option<ToolCallExtraContent>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct FunctionCall {
    pub name: ToolName,
    // Store raw JSON arguments - needs to be owned String for deserialization from OpenRouter
    pub arguments: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct ToolCallExtraContent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleToolCallExtraContent>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct GoogleToolCallExtraContent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::RequestMessage;
    use serde_json::json;

    #[test]
    fn google_openai_thought_signature_round_trips_on_assistant_tool_call() {
        let call: ToolCall = serde_json::from_value(json!({
            "id": "function-call-1",
            "type": "function",
            "function": {
                "name": "request_code_context",
                "arguments": "{\"search_term\":\"find_iter_at_in_context\"}"
            },
            "extra_content": {
                "google": {
                    "thought_signature": "opaque-google-signature"
                }
            }
        }))
        .expect("Google OpenAI-compatible tool call parses");

        let message = RequestMessage::new_assistant_with_tool_calls(None, vec![call]);
        let value = serde_json::to_value(&message).expect("message serializes");

        assert_eq!(
            value["tool_calls"][0]["extra_content"]["google"]["thought_signature"],
            "opaque-google-signature"
        );
    }
}
