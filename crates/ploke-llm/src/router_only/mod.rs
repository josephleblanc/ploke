//! Router-specific implementations

#[cfg(test)]
mod tests;

pub(super) mod cli;
pub mod openrouter;

use crate::manager::RequestMessage;
use crate::manager::Role;
use itertools::Itertools;
use openrouter::{FallbackMarker, MiddleOutMarker, OpenRouterModelId, Transform};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use ploke_core::tool_types::ToolDefinition;

use super::registry::user_prefs::ModelPrefs;
use super::registry::user_prefs::ModelProfile;
use super::{
    EndpointKey, EndpointsResponse, LLMParameters, ModelId, ModelKey,
    registry::user_prefs::RegistryPrefs,
    request::{
        ChatCompReqCore, JsonObjMarker,
        endpoint::{EndpointData, ToolChoice},
        models,
    },
    types::model_types::ModelVariant,
};
mod anthropic {
    use super::*;
    // Note: Placeholder, just an example for now
    #[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Hash, Eq)]
    pub struct Anthropic;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    // Note: Placeholder, just an example for now
    pub struct ChatCompFields {
        claude_specific_one: Option<String>,
        claude_specific_two: Option<String>,
    }
}

pub trait HasModelId {
    fn model_id(&self) -> ModelId;
}

pub trait HasModels: Router {
    // Models response
    type Response: for<'a> Deserialize<'a> + IntoIterator<Item = Self::Models>;
    type Models: for<'a> Deserialize<'a> + HasModelId + Into<models::ResponseItem>;
    type Error;

    async fn fetch_models(client: &reqwest::Client) -> color_eyre::Result<Self::Response> {
        let url = Self::MODELS_URL;
        let api_key = Self::resolve_api_key()?;

        let resp = client
            .get(url)
            .bearer_auth(api_key)
            .header("Accept", "application/json")
            .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
            .header("X-Title", "Ploke TUI")
            .send()
            .await?
            .error_for_status()?;

        let parsed = resp.json::<Self::Response>().await?;

        Ok(parsed)
    }

    async fn fetch_models_iter(
        client: &reqwest::Client,
    ) -> color_eyre::Result<impl IntoIterator<Item = Self::Models>> {
        Self::fetch_models(client).await.map(|r| r.into_iter())
    }
}

pub trait HasEndpoint: Router {
    type EpResponse: for<'a> Deserialize<'a> + Into<EndpointsResponse>;
    type Error;

    async fn fetch_model_endpoints(
        client: &reqwest::Client,
        model: Self::RouterModelId,
    ) -> color_eyre::Result<Self::EpResponse> {
        let url = Self::endpoints_url(model);
        let api_key = Self::resolve_api_key()?;

        let resp = client
            .get(url)
            .bearer_auth(api_key)
            .header("Accept", "application/json")
            .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
            .header("X-Title", "Ploke TUI")
            .send()
            .await?
            .error_for_status()?;

        // let body = resp.json().await?;
        // Parse with stronger typed endpoint shape first
        let parsed = resp.json::<Self::EpResponse>().await?;

        Ok(parsed)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Hash, Eq)]
pub enum RouterVariants {
    OpenRouter(openrouter::OpenRouter),
    Anthropic(anthropic::Anthropic),
}

impl Default for RouterVariants {
    fn default() -> Self {
        Self::OpenRouter(openrouter::OpenRouter)
    }
}

pub trait RouterModelId: From<ModelId> {
    fn into_key(self) -> ModelKey;
    fn key(&self) -> &ModelKey;
    fn into_url_format(self) -> String;
}

pub trait Router:
    Copy + Clone + PartialEq + PartialOrd + Serialize + DeserializeOwned + Eq + Default
{
    /// The `/chat/completions` fields that are unique to this router.
    /// For example, for OpenRouter, there are unique fields like `transforms`, `models`, and
    /// others, which are found in the associated type for this trait implementation of
    /// `OpenRouter`
    ///
    /// These are fields that are not present for all routers.
    //
    // NOTE: If we find that there is a meaningful subset, we can create another trait for that
    // subset by:
    // 1. Creating a  Has* trait for that subset
    // 2. Implementing builder patterns
    // 3. Surface building patterns in the containing `ChatCompRequest<T>` struct for convenience.
    type CompletionFields: ApiRoute + Serialize + Default;
    /// Router-unique way of representing the model for this router.
    /// For example, for OpenRouter there is a non-standard way of defining models by:
    /// {author}/{model}:{variant}, where the `:{variant}` is optional, e.g.
    /// - deepseek/deepseek-v3.1
    /// - deepseek/deepseek-v3.1:free
    ///
    /// See `OpenRouterVariants` and `RouterVariants` enums for possible values.
    type RouterModelId: RouterModelId + From<EndpointKey> + From<ModelId>;
    // This is where we would put other differentiating items, for example if there are
    // OpenRouter-unique fields of `LLMParameters`, we would create something like:
    // LlmParamFields: `RouterLlmParam + From<LlmParameters>
    // - Would require a new trait, `RouterLlmParam`
    // - Should implement `From<LlmParameters>` so we can keep overall process generic

    /// Base url of router, e.g.
    /// - "https://openrouter.ai/api/v1"
    const BASE_URL: &str;
    /// Chat completion url of router, e.g.
    /// - "https://openrouter.ai/api/v1/chat/completions"
    const COMPLETION_URL: &str;
    /// Url to use for getting a list of available models, e.g.
    /// - "https://openrouter.ai/api/v1/models"
    const MODELS_URL: &str;
    /// When getting the endpoints from a router, this is the tail of the url string following a
    /// final `/`, e.g.
    /// - "endpoints"
    /// - For e.g. OpenRouter becomes:
    ///     - "https://openrouter.ai/api/v1/models/{author}/{model}:{variant}/endpoints", where
    ///       `:{variant}` is optional, see note on `ChatCompletionFields` above
    const ENDPOINTS_TAIL: &str;
    /// The expected name of the API key as in an exported env variable, e.g.
    /// - "OPENROUTER_API_KEY"
    const API_KEY_NAME: &str;
    /// The url for the providers for this router, e.g.
    /// - "https://openrouter.ai/api/v1/providers"
    // NOTE: We may remove this, as it is currently not really being used for anything. Evaluate
    // once we have developed this module fully and are refactoring, as this may be the one field
    // that is not common to both native model APIs like OpenAI and true routers like OpenRouter.
    const PROVIDERS_URL: &str;

    fn resolve_api_key() -> Result<String, std::env::VarError> {
        // 1. Check provider-specific env var if specified
        let key_name = Self::API_KEY_NAME;
        std::env::var(key_name)
    }

    fn endpoints_url(model: Self::RouterModelId) -> String {
        // OpenRouter’s models path treats ':' as a reserved char → percent-encode
        // Use a lightweight escape because only ':' needs it for your case
        // WARNING: above claim needs verification via testing + cite test
        let base = model.into_url_format();
        format!("{}/{}/{}", Self::MODELS_URL, base, Self::ENDPOINTS_TAIL)
    }

    fn tranform_endpoint_key(&self, endpoint_key: EndpointKey) -> Self::RouterModelId {
        Self::RouterModelId::from(endpoint_key)
    }

    // TODO: Decide if we are really using this, since it may be confusing as in the case of
    // OpenRouter this quietly ignores the `{:variant}` in `{author}/{model}:{variant}` formatting
    // specific to OpenRouter, may be similar for other routers/native API providers
    fn enpoint_to_url(&self, endpoint_key: EndpointKey) -> String {
        let model_id = self.tranform_endpoint_key(endpoint_key);
        Self::endpoints_url(model_id)
    }

    fn default_chat_completion() -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            router: Self::CompletionFields::default(),
            ..Default::default()
        }
    }
    fn completion_core(
        fields: Self::CompletionFields,
        core: ChatCompReqCore,
    ) -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            model_key: Some(core.model.key),
            router: fields,
            ..Default::default()
        }
    }
}

// TODO: Consider deleting this. Kind of a weird pattern. Currently used in
// `llm::manager::session` module/file
pub trait ApiRoute: Sized + Default + Serialize {
    type Parent: TryFrom<RouterVariants> + Into<RouterVariants> + Default;
    fn parent() -> Self::Parent {
        Self::Parent::default()
    }
    fn router_variant() -> RouterVariants {
        Self::parent().into()
    }
}
use serde_json::{Value, json};
use std::{str::FromStr as _, sync::OnceLock};

static DEFAULT_MODEL: OnceLock<String> = OnceLock::new();
pub fn default_model() -> String {
    DEFAULT_MODEL
        .get_or_init(|| "moonshotai/kimi-k2".to_string())
        .clone()
}

static DEFAULT_MESSAGE: OnceLock<Vec<RequestMessage>> = OnceLock::new();
pub fn default_messages() -> Vec<RequestMessage> {
    DEFAULT_MESSAGE
        .get_or_init(|| {
            vec![RequestMessage {
                role: Role::System,
                content: "You are a helpful assistant, please help the user with their requests."
                    .to_string(),
                tool_call_id: None,
            }]
        })
        .clone()
}

// TODO: Add a GhostData field for validation or use a custom serde method to build the request
// and use `Option` on the bundled fields we want to flatten.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ChatCompRequest<R>
where
    R: Router,
    R::CompletionFields: ApiRoute + Serialize,
{
    /// The more general `model_key` used internally to determine the model, not actually sent in
    /// the request to the API.
    ///
    /// The actual model identifier the API is expecting is in the generic type for `router` below,
    /// as the identifier style may vary across routers.
    ///
    /// Note that `ModelKey` is in the form `{author}/{model}`, and will produced expected behavior
    /// in all cases if used as the `model` in the API request. E.g. for OpenRouter, there is an
    /// optional variant case, `{author}/{model}:{variant}` where `:{variant}` is optional, and so
    /// using this `ModelKey` would remove this distinction in requests.
    #[serde(skip, default)]
    pub model_key: Option<ModelKey>,
    /// Core copletion request items that are common to all routers. This field is flattened into
    /// `ChatCompReq` so the common fields appear in the json request as fields of
    /// `ChatCompRequest`, kept separate here for modularization of different kinds of parameters
    /// that may vary differently across routers.
    #[serde(flatten)]
    pub core: ChatCompReqCore,
    /// The parameters that may be set for this router. This is the set of LLMParameters that are
    /// common to all routers.
    ///
    /// These are flattened into the `ChatCompRequest` during the serialization by `serde` for the
    /// final request sent to the router API.
    // NOTE: When adding our second router (after OpenRouter), consider splitting this into a
    // `CommonLLMParams` and `RouterLlmParams` or something, where the router-specific options will
    // follow a similar pattern to including the other router-specific parameters as in the
    // `Router` trait.
    #[serde(flatten)]
    pub llm_params: LLMParameters,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    /// NOTE: The `ToolsChoice` used below may or may not be unique to OpenRouter, determine when
    /// adding a new router/native API if we need to split `ToolChoice` off into another trait in
    /// the same pattern as `ChatCompRequest<R: ApiRoute>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    // Router-specific fields merged at the top level
    #[serde(flatten)]
    pub router: R::CompletionFields,
}

impl<R> ChatCompRequest<R>
where
    R: Router,
    R::CompletionFields: ApiRoute + Serialize,
{
    pub fn with_core_bundle(mut self, core: ChatCompReqCore) -> Self {
        self.core = core;
        self
    }

    pub fn with_param_bundle(mut self, llm_params: LLMParameters) -> Self {
        self.llm_params = llm_params;
        self
    }

    pub fn with_router_bundle(mut self, router: R::CompletionFields) -> Self {
        self.router = router;
        self
    }

    pub fn with_model_key(mut self, model_key: Option<ModelKey>) -> Self {
        self.model_key = model_key;
        self
    }

    /// Tool definitions we provide for the model to use, see the `Tool` trait.
    pub fn with_tools(mut self, tools: Option<Vec<ToolDefinition>>) -> Self {
        self.tools = tools;
        self
    }

    /// Tool definitions we provide for the model to use, see the `Tool` trait.
    ///
    /// e.g. for OpenRouter
    /// - Bridge format: "none" | "auto" | { type: "function", function: { name } }
    pub fn with_tool_choice(mut self, tool_choice: Option<ToolChoice>) -> Self {
        self.tool_choice = tool_choice;
        self
    }

    /// Set the messages for the completion request
    pub fn with_messages(mut self, messages: Vec<RequestMessage>) -> Self {
        self.core = self.core.with_messages(messages);
        self
    }

    /// Set a single message for the completion request
    pub fn with_message(mut self, message: RequestMessage) -> Self {
        self.core = self.core.with_message(message);
        self
    }

    /// Set the prompt for the completion request (alternative to messages)
    pub fn with_prompt(mut self, prompt: String) -> Self {
        self.core = self.core.with_prompt(prompt);
        self
    }

    /// Set the model for the completion request
    pub fn with_model(mut self, model: ModelId) -> Self {
        self.core = self.core.with_model(model);
        self
    }

    /// Set the model by string (parses into ModelId)
    pub fn with_model_str(self, model_str: &str) -> Result<Self, crate::IdError> {
        let model = ModelId::from_str(model_str)?;
        Ok(self.with_model(model))
    }

    /// Set the response format to JSON object
    pub fn with_json_response(mut self) -> Self {
        self.core = self.core.with_json_response();
        self
    }

    /// Set the stop sequences
    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.core = self.core.with_stop(stop);
        self
    }

    /// Add a single stop sequence
    pub fn with_stop_sequence(mut self, stop: String) -> Self {
        self.core = self.core.with_stop_sequence(stop);
        self
    }

    /// Enable or disable streaming
    pub fn with_streaming(mut self, stream: bool) -> Self {
        self.core = self.core.with_streaming(stream);
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

    /// Set max tokens parameter - Range: [1, context_length)
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.llm_params = self.llm_params.with_max_tokens(max_tokens);
        self
    }

    /// Set temperature parameter - Range: [0, 2]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.llm_params = self.llm_params.with_temperature(temperature);
        self
    }

    /// Set seed parameter - Integer only
    pub fn with_seed(mut self, seed: i64) -> Self {
        self.llm_params = self.llm_params.with_seed(seed);
        self
    }

    /// Set top_p parameter - Range: (0, 1]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.llm_params = self.llm_params.with_top_p(top_p);
        self
    }

    /// Set top_k parameter - Range: [1, Infinity) Not available for OpenAI models
    pub fn with_top_k(mut self, top_k: f32) -> Self {
        self.llm_params = self.llm_params.with_top_k(top_k);
        self
    }

    /// Set frequency_penalty parameter - Range: [-2, 2]
    pub fn with_frequency_penalty(mut self, frequency_penalty: f32) -> Self {
        self.llm_params = self.llm_params.with_frequency_penalty(frequency_penalty);
        self
    }

    /// Set presence_penalty parameter - Range: [-2, 2]
    pub fn with_presence_penalty(mut self, presence_penalty: f32) -> Self {
        self.llm_params = self.llm_params.with_presence_penalty(presence_penalty);
        self
    }

    /// Set repetition_penalty parameter - Range: (0, 2]
    pub fn with_repetition_penalty(mut self, repetition_penalty: f32) -> Self {
        self.llm_params = self.llm_params.with_repetition_penalty(repetition_penalty);
        self
    }

    /// Set logit_bias parameter - { [key: number]: number }
    pub fn with_logit_bias(mut self, logit_bias: std::collections::BTreeMap<i32, f32>) -> Self {
        self.llm_params = self.llm_params.with_logit_bias(logit_bias);
        self
    }

    /// Set top_logprobs parameter - Integer only
    pub fn with_top_logprobs(mut self, top_logprobs: i32) -> Self {
        self.llm_params = self.llm_params.with_top_logprobs(top_logprobs);
        self
    }

    /// Set min_p parameter - Range: [0, 1]
    pub fn with_min_p(mut self, min_p: f32) -> Self {
        self.llm_params = self.llm_params.with_min_p(min_p);
        self
    }

    /// Set top_a parameter - Range: [0, 1]
    pub fn with_top_a(mut self, top_a: f32) -> Self {
        self.llm_params = self.llm_params.with_top_a(top_a);
        self
    }
}
