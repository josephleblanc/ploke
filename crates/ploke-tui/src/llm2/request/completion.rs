pub(crate) use crate::llm2::router_only::default_model;
pub(crate) use crate::llm2::router_only::default_messages;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    llm2::{
        LLMParameters,
        chat_msg::RequestMessage,
        ModelId,
        router_only::{
            ApiRoute,
            openrouter::{self, ProviderPreferences, Transform},
        },
    },
    tools::ToolDefinition,
};

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
    /// canonical endpoint name (author/slug), e.g. deepseek/deepseek-chat-v3.1
    #[serde(default = "default_model")]
    pub(crate) model: String,
    /// TODO: We should create a Marker struct for this, similar to `FunctionMarker` in
    /// `crates/ploke-tui/src/tools/mod.rs`, since this is a constant value
    /// corresponding json: `response_format?: { type: 'json_object' };`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) response_format: Option<JsonObjMarker>, // TODO

    /// corresponding json: `stop?: string | string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stop: Option<Vec<String>>,
    /// OpenRouter docs: "Enable streaming"
    /// corresponding json: `stream?: boolean;`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stream: Option<bool>,

    #[serde(flatten)]
    pub(crate) llm_params: LLMParameters,
}
