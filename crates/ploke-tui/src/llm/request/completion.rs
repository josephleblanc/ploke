use crate::llm::types::model_types::serialize_model_id_as_request_string;
use crate::llm::manager::RequestMessage;
pub(crate) use crate::llm::router_only::default_model;
pub(crate) use crate::llm::router_only::default_messages;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    llm::{
        LLMParameters,
        ModelId,
        router_only::{
            ApiRoute,
            openrouter::{self, ProviderPreferences, Transform},
        },
    },
    tools::ToolDefinition,
};
use std::str::FromStr;

use super::{
    endpoint::{FallbackMarker, ToolChoice},
    marker::JsonObjMarker,
};

/// Completion request for the OpenRouter url at
/// - https://openrouter.ai/api/v1/chat/completions
#[derive(Serialize, Debug, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ChatCompReqCore {
    // OpenRouter docs: "Either "messages" or "prompt" is required"
    // corresponding json: `messages?: Message[];`
    #[serde(default = "default_messages")]
    pub(crate) messages: Vec<RequestMessage>,
    // corresponding json: `prompt?: string;`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt: Option<String>,
    /// OpenRouter docs: "If "model" is unspecified, uses the user's default"
    ///  - Note: This default is set on the OpenRouter website
    ///  - If we get errors for "No model available", provide the user with a message suggesting
    ///    they check their OpenRouter account settings on the OpenRouter website for filtered
    ///    providers as the cause of "No model available". If the user filters out all model providers
    ///    that fulfill our (in ploke) filtering requirements (e.g. for tool-calling), this can lead
    ///    to no models being available for the requests we send.
    ///
    /// corresponding json: `model?: string;`
    /// canonical endpoint name `{ author }/{slug}:{variant}`, e.g. 
    /// - deepseek/deepseek-chat-v3.1
    /// - can also have variant, deepseek/deepseek-chat-v3.1:free
    #[serde(default, serialize_with = "serialize_model_id_as_request_string")]
    pub(crate) model: ModelId,
    /// TODO: We should create a Marker struct for this, similar to `FunctionMarker` in
    /// `crates/ploke-tui/src/tools/mod.rs`, since this is a constant value
    /// corresponding json: `response_format?: { type: 'json_object' };`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) response_format: Option<JsonObjMarker>,

    /// corresponding json: `stop?: string | string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stop: Option<Vec<String>>,
    /// OpenRouter docs: "Enable streaming"
    /// corresponding json: `stream?: boolean;`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stream: Option<bool>,
}

impl ChatCompReqCore {
    /// Create a new `ChatCompReqCore` with default values
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Set the messages for the completion request
    pub(crate) fn with_messages(mut self, messages: Vec<RequestMessage>) -> Self {
        self.messages = messages;
        self
    }

    /// Set a single message for the completion request
    pub(crate) fn with_message(mut self, message: RequestMessage) -> Self {
        self.messages = vec![message];
        self
    }

    /// Set the prompt for the completion request (alternative to messages)
    pub(crate) fn with_prompt(mut self, prompt: String) -> Self {
        self.prompt = Some(prompt);
        self.messages = Vec::new(); // Clear messages when using prompt
        self
    }

    /// Set the model for the completion request
    pub(crate) fn with_model(mut self, model: ModelId) -> Self {
        self.model = model;
        self
    }

    /// Set the model by string (parses into ModelId)
    pub(crate) fn with_model_str(self, model_str: &str) -> Result<Self, crate::llm::IdError> {
        let model = ModelId::from_str(model_str)?;
        Ok(self.with_model(model))
    }

    /// Set the response format to JSON object
    pub(crate) fn with_json_response(mut self) -> Self {
        self.response_format = Some(JsonObjMarker);
        self
    }

    /// Set the stop sequences
    pub(crate) fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Add a single stop sequence
    pub(crate) fn with_stop_sequence(mut self, stop: String) -> Self {
        match &mut self.stop {
            Some(stops) => stops.push(stop),
            None => self.stop = Some(vec![stop]),
        }
        self
    }

    /// Enable or disable streaming
    pub(crate) fn with_streaming(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }

    /// Enable streaming (convenience method)
    pub(crate) fn streaming(self) -> Self {
        self.with_streaming(true)
    }

    /// Disable streaming (convenience method)
    pub(crate) fn non_streaming(self) -> Self {
        self.with_streaming(false)
    }

    /// Build the request (identity function for consistency)
    pub(crate) fn build(self) -> Self {
        self
    }

    // TODO: Add a validate function that will check if prompt and messages are there, in which
    // case it should return invalid. Must have one of the two.
}

mod tests {
    use crate::llm::{manager::RequestMessage, request::JsonObjMarker, ModelId};

    use super::ChatCompReqCore;
    use color_eyre::Result;

    #[test]
    fn test_serialize_default() -> Result<()> {
        let core = ChatCompReqCore::default();
        println!("before ser ChatCompReqCore:\n\n{:#?}", core);

        let pretty = serde_json::to_string_pretty(&core)?;
        println!("pretty ser ChatCompReqCore:\n\n{}", pretty);

        // default model
        let model_str = r#""model": "moonshotai/kimi-k2""#;
        assert!(pretty.contains(model_str));

        Ok(())
    }

    #[test]
    fn all_fields_roundtrip() -> Result<()> {
        use std::str::FromStr;
        let core = ChatCompReqCore::default();

        let msg = String::from("test use message");
        let req_msg = vec![ RequestMessage::new_user(msg) ];

        let model_str = "moonshotai/kimi-k2:free";
        let model = ModelId::from_str(model_str)?;

        let stop_token = String::from( "<|im_end|>" );
        let stop= vec![ stop_token ];

        let full_msg = core.with_messages(req_msg)
            .with_model(model)
            .with_json_response()
            .with_streaming(true)
            .with_stop(stop);

        eprintln!("core msg all fields:\n\n{:#?}", full_msg);
        let serial_pretty = serde_json::to_string_pretty(&full_msg)?;
        eprintln!("core msg all fields serialized:\n\n{}", serial_pretty);

        let serial = serde_json::to_string(&full_msg)?;
        let deserial: ChatCompReqCore = serde_json::from_str(&serial)?;

        assert_eq!(full_msg, deserial);
        Ok(())
    }
}
