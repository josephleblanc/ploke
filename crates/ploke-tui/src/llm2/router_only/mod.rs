//! Router-specific implementations

pub(super) mod cli;
pub(crate) mod openrouter;
pub(crate) use cli::{
    COMPLETION_JSON_SIMPLE_DIR, ENDPOINTS_JSON_DIR, MODELS_JSON_ARCH, MODELS_JSON_PRICING,
    MODELS_JSON_RAW, MODELS_TXT_IDS, MODELS_JSON_ID_NOT_NAME
};

use crate::llm2::manager::RequestMessage;
use crate::llm2::manager::Role;
use itertools::Itertools;
use openrouter::{FallbackMarker, MiddleOutMarker, OpenRouterModelId, Transform};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::tools::ToolDefinition;

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
    pub(crate) struct Anthropic;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    // Note: Placeholder, just an example for now
    pub(crate) struct ChatCompFields {
        claude_specific_one: Option<String>,
        claude_specific_two: Option<String>,
    }
}

pub(crate) trait HasModelId {
    fn model_id(&self) -> ModelId;
}

pub(crate) trait HasModels: Router {
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

pub(crate) trait HasEndpoint: Router {
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
pub(crate) enum RouterVariants {
    OpenRouter(openrouter::OpenRouter),
    Anthropic(anthropic::Anthropic),
}

impl Default for RouterVariants {
    fn default() -> Self {
        Self::OpenRouter(openrouter::OpenRouter)
    }
}

pub(crate) trait RouterModelId: From<ModelId> {
    fn into_key(self) -> ModelKey;
    fn key(&self) -> &ModelKey;
    fn into_url_format(self) -> String;
}

pub(crate) trait Router:
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
// `llm2::manager::session` module/file
pub(crate) trait ApiRoute: Sized + Default + Serialize {
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
pub(crate) fn default_model() -> String {
    DEFAULT_MODEL
        .get_or_init(|| "moonshotai/kimi-k2".to_string())
        .clone()
}

static DEFAULT_MESSAGE: OnceLock<Vec<RequestMessage>> = OnceLock::new();
pub(crate) fn default_messages() -> Vec<RequestMessage> {
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
pub(crate) struct ChatCompRequest<R>
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
    pub(crate) model_key: Option<ModelKey>,
    /// Core copletion request items that are common to all routers. This field is flattened into
    /// `ChatCompReq` so the common fields appear in the json request as fields of
    /// `ChatCompRequest`, kept separate here for modularization of different kinds of parameters
    /// that may vary differently across routers.
    #[serde(flatten)]
    pub(crate) core: ChatCompReqCore,
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
    pub(crate) llm_params: LLMParameters,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tools: Option<Vec<ToolDefinition>>,
    /// NOTE: The `ToolsChoice` used below may or may not be unique to OpenRouter, determine when
    /// adding a new router/native API if we need to split `ToolChoice` off into another trait in
    /// the same pattern as `ChatCompRequest<R: ApiRoute>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_choice: Option<ToolChoice>,

    // Router-specific fields merged at the top level
    #[serde(flatten)]
    pub(crate) router: R::CompletionFields,
}

impl<R> ChatCompRequest<R>
where
    R: Router,
    R::CompletionFields: ApiRoute + Serialize,
{
    pub(crate) fn with_core_bundle(mut self, core: ChatCompReqCore) -> Self {
        self.core = core;
        self
    }

    pub(crate) fn with_param_bundle(mut self, llm_params: LLMParameters) -> Self {
        self.llm_params = llm_params;
        self
    }

    pub(crate) fn with_model_key(mut self, model_key: Option<ModelKey>) -> Self {
        self.model_key = model_key;
        self
    }

    pub(crate) fn with_params_union(mut self, prefs: &RegistryPrefs) -> Self {
        let model_prefs = self.model_key.as_ref().and_then(|m| prefs.models.get(m));
        if let Some(pref) = model_prefs.and_then(|mp| mp.get_default_profile()) {
            self.llm_params.apply_union(&pref.params);
        }
        self
    }

    /// Tool definitions we provide for the model to use, see the `Tool` trait.
    pub(crate) fn with_tools(mut self, tools: Option<Vec<ToolDefinition>>) -> Self {
        self.tools = tools;
        self
    }

    /// Tool definitions we provide for the model to use, see the `Tool` trait.
    ///
    /// e.g. for OpenRouter
    /// - Bridge format: "none" | "auto" | { type: "function", function: { name } }
    pub(crate) fn with_tool_choice(mut self, tool_choice: Option<ToolChoice>) -> Self {
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
    pub fn with_model_str(self, model_str: &str) -> Result<Self, crate::llm2::IdError> {
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

#[cfg(test)]
mod tests {
    use crate::{llm2::router_only::openrouter::OpenRouter, tools::Tool};
    use std::time::Duration;

    use crate::{llm2::ModelId, llm2::error::LlmError};
    use std::{path::PathBuf, str::FromStr as _};

    use super::*;
    #[test]
    fn show_openrouter_json2() {
        let req = ChatCompRequest::<OpenRouter> {
            router: openrouter::ChatCompFields::default()
                .with_route(FallbackMarker)
                .with_transforms(Transform::MiddleOut([MiddleOutMarker])),
            ..Default::default()
        };
        let j = serde_json::to_string_pretty(&req).unwrap();
        println!("{j}");
    }

    fn parse_with_env(response_json: &str) -> Result<()> {
        use ploke_test_utils::workspace_root;

        use crate::llm2::router_only::cli::{
            MODELS_JSON_ARCH, MODELS_JSON_RAW, MODELS_JSON_RAW_PRETTY, MODELS_JSON_SUPPORTED, MODELS_JSON_TOP, MODELS_TXT_CANON, MODELS_TXT_IDS
        };

        let mut dir = workspace_root();
        let parsed: models::Response = serde_json::from_str(response_json)?;

        let env_string = std::env::var("WRITE_MODE").unwrap_or_default();
        if ["raw", "all"].contains(&env_string.as_str()) {
            dir.push(MODELS_JSON_RAW);
            println!("Writing '/models' raw response to:\n{}", dir.display());
            std::fs::write(dir, response_json)?;
        }
        if ["raw_pretty", "all"].contains(&env_string.as_str()) {
            let mut dir = workspace_root();
            let raw_pretty = serde_json::Value::from_str(response_json)?;
            let pretty = serde_json::to_string_pretty(&raw_pretty)?;
            dir.push(MODELS_JSON_RAW_PRETTY);
            println!("Writing '/models' raw response to:\n{}", dir.display());
            std::fs::write(dir, &pretty)?;
        }

        write_response(&env_string, &parsed)?;

        if env_string == "all" {
            for op in ["id", "arch", "top", "pricing"] {
                write_response(op, &parsed)?;
            }
        }

        fn write_response(env_str: &str, parsed: &models::Response) -> Result<()> {
            let mut dir = workspace_root();
            match env_str {
                "id" => {
                    let names = parsed.data.iter().map(|r| r.id.to_string()).join("\n");
                    dir.push(MODELS_TXT_IDS);
                    println!(
                        "Writing '/models' id fields response to:\n{}",
                        dir.display()
                    );
                    std::fs::write(dir, &names)?;
                }
                "arch" => {
                    let architecture = parsed
                        .data
                        .iter()
                        .map(|r| r.architecture.clone())
                        .collect_vec();
                    let pretty_arch = serde_json::to_string_pretty(&architecture)?;
                    dir.push(MODELS_JSON_ARCH);
                    println!(
                        "Writing '/models' architecture fields response to:\n{}",
                        dir.display()
                    );
                    std::fs::write(dir, &pretty_arch)?;
                }
                "top" => {
                    let top_provider = parsed
                        .data
                        .iter()
                        .map(|r| r.top_provider.clone())
                        .collect_vec();
                    let pretty_arch = serde_json::to_string_pretty(&top_provider)?;
                    dir.push(MODELS_JSON_TOP);
                    println!(
                        "Writing '/models' top_provider fields response to:\n{}",
                        dir.display()
                    );
                    std::fs::write(dir, &pretty_arch)?;
                }
                "pricing" => {
                    let pricing = parsed.data.iter().map(|r| r.pricing).collect_vec();
                    let pretty_pricing = serde_json::to_string_pretty(&pricing)?;
                    dir.push(MODELS_JSON_PRICING);
                    println!(
                        "Writing '/models' pricing fields response to:\n{}",
                        dir.display()
                    );
                    std::fs::write(dir, &pretty_pricing)?;
                }
                "supported" => {
                    let supported = parsed
                        .data
                        .iter()
                        .map(|r| r.supported_parameters.clone())
                        .collect_vec();
                    let pretty_supported = serde_json::to_string_pretty(&supported)?;
                    dir.push(MODELS_JSON_SUPPORTED);
                    println!(
                        "Writing '/models' supported fields response to:\n{}",
                        dir.display()
                    );
                    std::fs::write(dir, &pretty_supported)?;
                }
                "canon" => {
                    let canon = parsed.data.iter()
                        .filter_map(|r| r.canonical.as_ref().map(|c| c.to_string())).join("\n");
                    dir.push(MODELS_TXT_CANON);
                    println!(
                        "Writing '/models' canon fields response to:\n{}",
                        dir.display()
                    );
                    std::fs::write(dir, &canon)?;
                }
                "all" => { /* handled above, just avoiding print below */ }
                "raw" => { /* handled above, just avoiding print below */ }
                "raw_pretty" => { /* handled above, just avoiding print below */ }
                s => {
                    println!("
Unkown command: {s}\nvalid choices:\n\traw\n\tall\n\tid\n\tarch\n\ttop
\tpricing\n\tcanon\n"
                    );
                }
            }
            Ok(())
        }
        Ok(())
    }

    use color_eyre::Result;
    use ploke_test_utils::workspace_root;
    use reqwest::Client;
    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_simple_query_models() -> Result<()> {

        let url = OpenRouter::MODELS_URL;
        // let key = OpenRouter::resolve_api_key()?;

        let response = Client::new()
            .get(url)
            // auth not required for this request
            // .bearer_auth(key)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let response_json = response.text().await?;

        parse_with_env(&response_json)?;

        Ok(())
    }

    #[test]
    fn test_names_vs_ids() -> Result<()> {
        let mut in_file = workspace_root();
        in_file.push(MODELS_JSON_RAW);
        let mut out_file = workspace_root();
        out_file.push( MODELS_JSON_ID_NOT_NAME );

        let s = std::fs::read_to_string(in_file)?;

        let mr: models::Response = serde_json::from_str(&s)?;

        let not_equal = mr.into_iter()
            .map(|i| ( i.id.key.to_string(), i.name.to_string() )  )
            .filter(|(k, n)| k != n)
            .collect_vec()
        ;
        let pretty = serde_json::to_string_pretty(&not_equal)?;

        std::fs::write(out_file, pretty)?;
        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_default_query_endpoints() -> Result<()> {
        use std::{path::PathBuf, str::FromStr as _};

        use ploke_test_utils::workspace_root;

        // TODO: we need to handle more items like the below, which the `ModelKey` doesn't
        // currently handle:
        // - nousresearch/deephermes-3-llama-3-8b-preview:free
        // - Should turn into a raw curl request like:
        //  https://openrouter.ai/api/v1/models/nousresearch/deephermes-3-llama-3-8b-preview%3Afree/endpoints

        let model_key = OpenRouterModelId::from_str("qwen/qwen3-30b-a3b")?;
        let url = OpenRouter::endpoints_url(model_key);
        eprintln!(
            "Constructed url to query `/:author/:model/endpoints at\n{}",
            url
        );
        assert_eq!(
            "https://openrouter.ai/api/v1/models/qwen/qwen3-30b-a3b/endpoints",
            url
        );
        let key = OpenRouter::resolve_api_key()?;
        let mut dir = workspace_root();
        dir.push(cli::ENDPOINTS_JSON_DIR);

        let response = Client::new()
            .get(url)
            .bearer_auth(key)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let is_success = response.status().is_success();
        eprintln!("is_success: {}", is_success);
        eprintln!("status: {}", response.status());
        let response_text = response.text().await?;

        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
            std::fs::create_dir_all(&dir)?;
            dir.push("endpoints.json");
            println!("Writing raw json reponse to: {}", dir.display());
            std::fs::write(dir, response_raw_pretty)?;
        }
        assert!(is_success);

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_free_query_endpoints() -> Result<()> {
        use ploke_test_utils::workspace_root;

        let model_key =
            OpenRouterModelId::from_str("nousresearch/deephermes-3-llama-3-8b-preview:free")?;
        let url = OpenRouter::endpoints_url(model_key);
        eprintln!(
            "Constructed url to query `/:author/:model/endpoints at\n{}",
            url
        );
        assert_eq!(
            "https://openrouter.ai/api/v1/models/nousresearch/deephermes-3-llama-3-8b-preview:free/endpoints",
            url
        );
        let key = OpenRouter::resolve_api_key()?;
        let mut dir = workspace_root();
        dir.push(cli::ENDPOINTS_JSON_DIR);

        let response = Client::new()
            .get(url)
            .bearer_auth(key)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let is_success = response.status().is_success();
        eprintln!("is_success: {}", is_success);
        eprintln!("status: {}", response.status());
        let response_text = response.text().await?;

        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
            std::fs::create_dir_all(&dir)?;
            dir.push("free_model.json");
            println!("Writing raw json reponse to: {}", dir.display());
            std::fs::write(dir, response_raw_pretty)?;
        }
        assert!(is_success);

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_default_post_completions() -> Result<()> {
        use crate::llm2::{ModelId, router_only::cli::COMPLETION_JSON_SIMPLE_DIR};
        use openrouter::OpenRouterModelId;
        use std::path::PathBuf;

        use ploke_test_utils::workspace_root;

        let model_id = OpenRouterModelId::from_str("qwen/qwen3-30b-a3b-thinking-2507")?;
        let key = OpenRouter::resolve_api_key()?;
        let url = OpenRouter::COMPLETION_URL;
        let mut dir = workspace_root();
        dir.push(COMPLETION_JSON_SIMPLE_DIR);

        let content = String::from("Hello, can you tell me about lifetimes in Rust?");
        let msg = RequestMessage {
            role: Role::User,
            content,
            tool_call_id: None,
        };

        let req = ChatCompRequest::<OpenRouter> {
            router: openrouter::ChatCompFields::default(),
            core: ChatCompReqCore {
                messages: vec![msg],
                ..Default::default()
            },
            ..Default::default()
        };

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let pretty = serde_json::to_string_pretty(&req)?;
            dir.push("request_se.json");
            println!("Writing serialized request to: {}", dir.display());
            std::fs::write(&dir, pretty)?;
        }

        let response = Client::new()
            .post(url)
            .bearer_auth(key)
            .json(&req)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;
        let is_success = response.status().is_success();
        eprintln!("is_success: {}", is_success);
        eprintln!("status: {}", response.status());

        let response_text = response.text().await?;

        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
            dir.pop();
            dir.push("response_raw.json");
            println!("Writing raw json reponse to: {}", dir.display());
            std::fs::write(dir, response_raw_pretty)?;
        }
        assert!(is_success);
        assert!(response_text.contains("help"));

        Ok(())
    }

    #[test]
    fn test_chat_comp_request_serialization_minimal() {
        use crate::llm2::manager::RequestMessage;
        use crate::llm2::request::ChatCompReqCore;
        use crate::llm2::request::endpoint::ToolChoice;
        use crate::llm2::router_only::default_model;
        use crate::tools::GetFileMetadata;

        let messages = vec![
            RequestMessage::new_system("sys".to_string()),
            RequestMessage::new_user("hello".to_string()),
        ];

        let default_model = default_model();
        let req = ChatCompRequest::<OpenRouter>::default()
            .with_core_bundle(ChatCompReqCore::default())
            .with_model_str(&default_model)
            .unwrap()
            .with_messages(messages)
            .with_temperature(0.0)
            .with_max_tokens(128);
        // let req = openrouter::ChatCompFields::default()
        //     .completion_core(ChatCompReqCore::default())
        //     .with_model_str(&default_model)
        //     .map(|r| r.with_messages(messages))
        //     .unwrap()
        //     .with_temperature(0.0)
        //     .with_max_tokens(128);
        let mut req = req;
        req.tools = Some(vec![GetFileMetadata::tool_def()]);
        req.tool_choice = Some(ToolChoice::Auto);

        let v = serde_json::to_value(&req).expect("serialize ChatCompRequest");
        // Top-level fields present
        assert_eq!(v.get("tool_choice").and_then(|t| t.as_str()), Some("auto"));
        assert_eq!(
            v.get("model").and_then(|m| m.as_str()),
            Some(default_model.as_str())
        );
        // Messages array content
        let msgs = v
            .get("messages")
            .and_then(|m| m.as_array())
            .expect("messages");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].get("role").and_then(|r| r.as_str()), Some("system"));
        assert_eq!(msgs[1].get("role").and_then(|r| r.as_str()), Some("user"));
        // Tools
        let tools = v.get("tools").and_then(|t| t.as_array()).expect("tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(
            tools[0].get("type").and_then(|s| s.as_str()),
            Some("function")
        );
    }
}
