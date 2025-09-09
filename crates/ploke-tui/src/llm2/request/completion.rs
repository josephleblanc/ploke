use crate::llm2::manager::RequestMessage;
pub(crate) use crate::llm2::router_only::default_model;
pub(crate) use crate::llm2::router_only::default_messages;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    llm2::{
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
#[derive(Serialize, Debug, Deserialize, Clone, Default)]
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
    #[serde(default)]
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the messages for the completion request
    pub fn with_messages(mut self, messages: Vec<RequestMessage>) -> Self {
        self.messages = messages;
        self
    }

    /// Set a single message for the completion request
    pub fn with_message(mut self, message: RequestMessage) -> Self {
        self.messages = vec![message];
        self
    }

    /// Set the prompt for the completion request (alternative to messages)
    pub fn with_prompt(mut self, prompt: String) -> Self {
        self.prompt = Some(prompt);
        self.messages = Vec::new(); // Clear messages when using prompt
        self
    }

    /// Set the model for the completion request
    pub fn with_model(mut self, model: ModelId) -> Self {
        self.model = model;
        self
    }

    /// Set the model by string (parses into ModelId)
    pub fn with_model_str(self, model_str: &str) -> Result<Self, crate::llm2::IdError> {
        let model = ModelId::from_str(model_str)?;
        Ok(self.with_model(model))
    }

    /// Set the response format to JSON object
    pub fn with_json_response(mut self) -> Self {
        self.response_format = Some(JsonObjMarker);
        self
    }

    /// Set the stop sequences
    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Add a single stop sequence
    pub fn with_stop_sequence(mut self, stop: String) -> Self {
        match &mut self.stop {
            Some(stops) => stops.push(stop),
            None => self.stop = Some(vec![stop]),
        }
        self
    }

    /// Enable or disable streaming
    pub fn with_streaming(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }

    /// Enable streaming (convenience method)
    pub fn streaming(self) -> Self {
        self.with_streaming(true)
    }

    /// Disable streaming (convenience method)
    pub fn non_streaming(self) -> Self {
        self.with_streaming(false)
    }

    /// Build the request (identity function for consistency)
    pub fn build(self) -> Self {
        self
    }
}
