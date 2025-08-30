// Defines the typed response received when calling for a model provider endpoint using `ProvEnd`
// in `provider_endpoint.rs`.
//
// Plans (in progress)
// The typed response, `Endpoint`, should implement Serialize and Deserialize for ergonomic
// deserializeation of the response from the call for all of the available endpoints for a
// specific model.
// This information should (if we understand the API structure correctly) be the data required to
// call into the `chat/completions` endpoints to request a generated response from the OpenRouter
// API.
//  - See `crates/ploke-tui/docs/openrouter/request_structure.md` for the OpenRouter official
//  documentation on the request structure.
//
// Desired functionality:
//  - We should take the typed response, `Endpoint`, and be capable of transforming it into a
//  `CompReq`, a completion request to the OpenRouter API, ideally through `Serialize`.
//      - NOTE: The `CompReq` will deprecate the `llm::`
//  - We can add the `Endpoint` to a cache of `Endpoint` that forms our official
//  `ModelRegistry`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    llm::{ProviderPreferences, RequestMessage},
    tools::ToolDefinition,
};

// Example json response to /endpoints:
//
// {
//   "context_length": 262144,
//   "max_completion_tokens": null,
//   "max_prompt_tokens": null,
//   "model_name": "Qwen: Qwen3 30B A3B Thinking 2507",
//   "name": "Nebius | qwen/qwen3-30b-a3b-thinking-2507",
//   "pricing": {
//     "completion": "0.0000003",
//     "discount": 0,
//     "image": "0",
//     "image_output": "0",
//     "internal_reasoning": "0",
//     "prompt": "0.0000001",
//     "request": "0",
//     "web_search": "0"
//   },
//   "provider_name": "Nebius",
//   "quantization": "fp8",
//   "status": 0,
//   "supported_parameters": [
//     "tools",
//     "tool_choice",
//     "reasoning",
//     "include_reasoning",
//     "max_tokens",
//     "temperature",
//     "top_p",
//     "stop",
//     "frequency_penalty",
//     "presence_penalty",
//     "seed",
//     "top_k",
//     "logit_bias",
//     "logprobs",
//     "top_logprobs"
//   ],
//   "supports_implicit_caching": false,
//   "tag": "nebius/fp8",
//   "uptime_last_30m": null
// },

use std::str::FromStr;
/// Strongly-typed wrappers around provider and model identifiers for endpoints.
use std::sync::Arc;

use crate::llm::openrouter::provider_endpoints::ProvEnd;
use crate::llm::openrouter_catalog::{ModelCapabilitiesRaw, ModelPricing};
use crate::llm::provider_endpoints::SupportedParameters;
use crate::llm::providers::{Author, ProviderName as ProviderNameEnum};

/// Raw model id as returned by APIs (may contain a variant suffix after ':').
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct ModelIdRaw(pub String);
impl ModelIdRaw {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    /// Strip a trailing variant suffix after ':' if present.
    pub fn base_id(&self) -> String {
        match self.0.split_once(':') {
            Some((base, _)) => base.to_string(),
            None => self.0.clone(),
        }
    }
    /// Extract variant suffix (after ':') if present.
    pub fn variant(&self) -> Option<String> {
        self.0.split_once(':').map(|(_, v)| v.to_string())
    }
} 

/// Extract variant suffix (after ':') if present.
    pub fn variant(&self) -> Option<String> {
        self.0.split_once(':').map(|(_, v)| v.to_string())
    }
}

/// Canonical slug "author/model"; can convert to ProvEnd.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct CanonicalSlug(pub String);
impl CanonicalSlug {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    pub fn to_provend(&self) -> Option<ProvEnd> {
        let s: &str = self.0.as_str();
        let (author, model) = s.split_once('/')?;
        let author = Author::from_str(author).ok()?;
        Some(ProvEnd {
            author,
            model: Arc::<str>::from(model),
        })
    }
}

/// Provider slug (preferred stable identifier for routing).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct ProviderSlug(pub String);
impl ProviderSlug {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    pub fn to_author(&self) -> Option<Author> {
        Author::from_str(self.0.as_str()).ok()
    }
}

/// Provider display name (human-readable).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct ProviderNameStr(pub String);
impl ProviderNameStr {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    /// Normalize display name into a conservative slug (lowercase, alnum -> keep, others -> '-').
    pub fn normalized_slug(&self) -> String{
        self.0
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect::<String>()
    } else {
                    '-'
                }
            })
            .collect::< Arc<[char]> >();
        Arc::<str>::from(s)
    }
    /// Attempt conversion via enum ProviderName, then map to Author; fall back to normalized slug parse.
    pub fn to_author(&self) -> Option<Author> {
        // Try precise enum parse via serde
        let try_enum = serde_json::from_str::<ProviderNameEnum>(&format!("\"{}\"", self.0)).ok();
        if let Some(pn) = try_enum {
            return Some(pn.to_slug());
        }
        Author::from_str(&self.normalized_slug()).ok()
    }
}

/// Provider id (raw), aliased across different shapes (id/provider/name/slug/provider_slug).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct ProviderIdRaw(pub String);
impl ProviderIdRaw {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Typed Endpoint entry from `/models/:author/:slug/endpoints`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Endpoint {
    /// Raw provider identifier with aliases.
    #[serde(
        alias = "id",
        alias = "provider",
        alias = "name",
        alias = "slug",
        alias = "provider_slug"
    )]
    pub provider_id: ProviderIdRaw,
    /// Optional explicit slug or name if provided.
    #[serde(default, alias = "provider_slug", alias = "slug")]
    pub provider_slug: Option<ProviderSlug>,
    #[serde(default, alias = "name")]
    pub provider_name: Option<ProviderNameStr>,

    #[serde(default)]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub max_completion_tokens: Option<u32>,
    #[serde(default)]
    pub max_prompt_tokens: Option<u32>,

    #[serde(default)]
    pub pricing: ModelPricing,

    #[serde(default)]
    pub supported_parameters: Option<Vec<SupportedParameters>>,
    #[serde(default)]
    pub capabilities: Option<ModelCapabilitiesRaw>,

    /// Optional canonical identity (author/model) for convenience when present.
    #[serde(default)]
    pub canonical: Option<CanonicalSlug>,
    /// Full raw id if carried alongside endpoint entries.
    #[serde(default)]
    pub raw_id: Option<ModelIdRaw>,
}

impl Endpoint {
    pub fn preferred_provider_slug(&self) -> String {
        if let Some(s) = &self.provider_slug {
            return s.0.clone();
        }
        if let Some(n) = &self.provider_name {
            return n.normalized_slug();
        }
        self.provider_id.0.clone()
    }
    pub fn supports_tools(&self) -> bool {
        self.supported_parameters
            .as_ref()
            .map(|v| v.iter().any(|p| matches!(p, SupportedParameters::Tools)))
            .unwrap_or(false)
    }
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct CompReq<'a> {
    // OpenRouter docs: "Either "messages" or "prompt" is required"
    // corresponding json: `messages?: Message[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<RequestMessage>>,
    // corresponding json: `prompt?: string;`
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    // OpenRouter docs: "If "model" is unspecified, uses the user's default"
    //  - Note: This default is set on the OpenRouter website
    //  - If we get errors for "No model available", provide the user with a message suggesting
    //  they check their OpenRouter account settings on the OpenRouter website for filtered
    //  providers as the cause of "No model available". If the user filters out all model providers
    //  that fulfill our (in ploke) filtering requirements (e.g. for tool-calling), this can lead
    //  to no models being available for the requests we send.
    // corresponding json: `model?: string;`
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    // TODO: We should create a Marker struct for this, similar to `FunctionMarker` in
    // `crates/ploke-tui/src/tools/mod.rs`, since this is a constant value
    // corresponding json: `response_format?: { type: 'json_object' };`
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<JsonObjMarker>, // TODO

    // corresponding json: `stop?: string | string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    // OpenRouter docs: "Enable streaming"
    // corresponding json: `stream?: boolean;`
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,

    // Openrouter docs: See LLM Parameters (openrouter.ai/docs/api-reference/parameters)
    //
    // corresponding json: `max_tokens?: number; // Range: [1, context_length)`
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    // corresponding json: `temperature?: number; // Range: [0, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    // OpenRouter docs:
    //  Tool calling
    //  Will be passed down as-is for providers implementing OpenAI's interface.
    //  For providers with custom interfaces, we transform and map the properties.
    //  Otherwise, we transform the tools into a YAML template. The model responds with an assistant message.
    //  See models supporting tool calling: openrouter.ai/models?supported_parameters=tools
    // NOTE: Do not use the website quoted above `openrouter.ai/models?supported_parameters=tools`
    // for API calls, this is a website and not an API endpoint... fool me once, *sigh*
    // corresponding json: `tools?: Tool[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
    // corresponding json: tool_choice?: ToolChoice;
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>,

    // OpenRouter docs: Advanced optional parameters
    //
    // corresponding json: `seed?: number; // Integer only`
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,
    // corresponding json: `top_p?: number; // Range: (0, 1]`
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    // corresponding json: `top_k?: number; // Range: [1, Infinity) Not available for OpenAI models`
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<f32>,
    // corresponding json: `frequency_penalty?: number; // Range: [-2, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    // corresponding json: `presence_penalty?: number; // Range: [-2, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    // corresponding json: `repetition_penalty?: number; // Range: (0, 2]`
    #[serde(skip_serializing_if = "Option::is_none")]
    repetition_penalty: Option<f32>,
    // corresponding json: `logit_bias?: { [key: number]: number };`
    #[serde(skip_serializing_if = "Option::is_none")]
    logit_bias: Option<BTreeMap<i32, f32>>,
    // corresponding json: `top_logprobs: number; // Integer only`
    #[serde(skip_serializing_if = "Option::is_none")]
    top_logprobs: Option<i32>,
    // corresponding json: `min_p?: number; // Range: [0, 1]`
    #[serde(skip_serializing_if = "Option::is_none")]
    min_p: Option<f32>,
    // corresponding json: `top_a?: number; // Range: [0, 1]`
    #[serde(skip_serializing_if = "Option::is_none")]
    top_a: Option<f32>,

    // OpenRouter docs: OpenRouter-only parameters
    //
    // OpenRouter docs: See "Prompt Transforms" section: openrouter.ai/docs/transforms
    // corresponding json: `transforms?: string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    transforms: Option<Vec<String>>,
    // OpenRouter docs: See "Model Routing" section: openrouter.ai/docs/model-routing
    // corresponding json: `models?: string[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    models: Option<Vec<String>>,
    // corresponding json: `route?: 'fallback';`
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<FallbackMarker>, // TODO
    // OpenRouter docs: See "Provider Routing" section: openrouter.ai/docs/provider-routing
    // corresponding json: `provider?: ProviderPreferences;`
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<ProviderPreferences>,
    // corresponding json: `user?: string; // A stable identifier for your end-users. Used to help detect and prevent abuse.`
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
}

// TODO: We should create a Marker struct for this, similar to `FunctionMarker` in
// `crates/ploke-tui/src/tools/mod.rs`, since this can onlly have the value (in json):
//  - see original json:`{ type: 'json_object' }`
//  - should be capable of Copy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct JsonObjMarker;

// TODO: We should create a Marker struct for this, similar to `FunctionMarker` in
// `crates/ploke-tui/src/tools/mod.rs`, since this can onlly have the value (in json):
//  - see original json: `route?: 'fallback';`
//  - should be capable of Copy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FallbackMarker;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ToolChoice {
    // TODO: fill as per `ToolChoice` in json schema
}
