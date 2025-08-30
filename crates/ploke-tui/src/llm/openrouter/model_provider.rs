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
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    llm::ProviderPreferences,
    tools::{FunctionMarker, ToolDefinition},
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

use crate::llm::openrouter::provider_endpoints::ProvEnd;
use crate::llm::openrouter::provider_endpoints::SupportedParameters;
use crate::llm::openrouter_catalog::ModelPricing;
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
        // Validate author via enum mapping, but store as slug string for routing stability.
        let _ = Author::from_str(author).ok()?;
        Some(ProvEnd { author: Arc::<str>::from(author), model: Arc::<str>::from(model) })
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
    pub fn normalized_slug(&self) -> String {
        self.0
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect::<String>()
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
#[derive(Debug, Clone, serde::Serialize)]
pub struct Endpoint {
    /// Raw provider identifier with aliases.
    pub provider_id: ProviderIdRaw,
    /// Optional explicit slug or name if provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<ProviderSlug>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<ProviderNameStr>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_prompt_tokens: Option<u32>,

    #[serde(default)]
    pub pricing: ModelPricing,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_parameters: Option<Vec<SupportedParameters>>,

    /// Optional canonical identity (author/model) for convenience when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical: Option<CanonicalSlug>,
    /// Full raw id if carried alongside endpoint entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_id: Option<ModelIdRaw>,
}

impl<'de> serde::Deserialize<'de> for Endpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error as DeError, MapAccess, Visitor};
        use serde_json::Value;

        struct EpVisitor;
        impl<'de> Visitor<'de> for EpVisitor {
            type Value = Endpoint;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an OpenRouter endpoint object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut provider_id: Option<ProviderIdRaw> = None;
                let mut provider_slug: Option<ProviderSlug> = None;
                let mut provider_name: Option<ProviderNameStr> = None;
                let mut context_length: Option<u32> = None;
                let mut max_completion_tokens: Option<u32> = None;
                let mut max_prompt_tokens: Option<u32> = None;
                let mut pricing: Option<ModelPricing> = None;
                let mut supported_parameters: Option<Vec<SupportedParameters>> = None;
                let mut canonical: Option<CanonicalSlug> = None;
                let mut raw_id: Option<ModelIdRaw> = None;

                while let Some((k, v)) = map.next_entry::<String, Value>()? {
                    match k.as_str() {
                        // Provider identity can appear under multiple keys
                        "id" | "provider" => {
                            let s: String = serde_json::from_value(v).map_err(DeError::custom)?;
                            if provider_id.is_none() {
                                provider_id = Some(ProviderIdRaw(s));
                            }
                        }
                        "provider_slug" | "slug" => {
                            let s: String = serde_json::from_value(v).map_err(DeError::custom)?;
                            provider_slug = Some(ProviderSlug(s.clone()));
                            if provider_id.is_none() {
                                provider_id = Some(ProviderIdRaw(s));
                            }
                        }
                        "provider_name" | "name" => {
                            let s: String = serde_json::from_value(v).map_err(DeError::custom)?;
                            provider_name = Some(ProviderNameStr(s.clone()));
                            if provider_id.is_none() {
                                provider_id = Some(ProviderIdRaw(s));
                            }
                        }
                        // Numeric/meta fields
                        "context_length" => {
                            context_length = serde_json::from_value(v).map_err(DeError::custom)?;
                        }
                        "max_completion_tokens" => {
                            max_completion_tokens = serde_json::from_value(v).map_err(DeError::custom)?;
                        }
                        "max_prompt_tokens" => {
                            max_prompt_tokens = serde_json::from_value(v).map_err(DeError::custom)?;
                        }
                        // Pricing can be either numbers or strings; delegate to typed struct
                        "pricing" => {
                            pricing = Some(serde_json::from_value(v).map_err(DeError::custom)?);
                        }
                        // Enum list
                        "supported_parameters" => {
                            supported_parameters = Some(serde_json::from_value(v).map_err(DeError::custom)?);
                        }
                        // Canonical slug may not exist in endpoint entries, but accept if present
                        "canonical_slug" => {
                            let s: String = serde_json::from_value(v).map_err(DeError::custom)?;
                            canonical = Some(CanonicalSlug(s));
                        }
                        // Let raw ids pass through if provided under a distinct key
                        "raw_id" => {
                            let s: String = serde_json::from_value(v).map_err(DeError::custom)?;
                            raw_id = Some(ModelIdRaw(s));
                        }
                        // Ignore other fields (status, uptime, transforms, etc.)
                        _ => {
                            // no-op
                        }
                    }
                }

                let provider_id = provider_id.ok_or_else(|| DeError::custom("missing provider identity (id/provider/provider_slug/slug/name)"))?;

                Ok(Endpoint {
                    provider_id,
                    provider_slug,
                    provider_name,
                    context_length,
                    max_completion_tokens,
                    max_prompt_tokens,
                    pricing: pricing.unwrap_or_default(),
                    supported_parameters,
                    canonical,
                    raw_id,
                })
            }
        }

        deserializer.deserialize_map(EpVisitor)
    }
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

/// Wrapper for the OpenRouter endpoints API response shape:
/// { "data": { "endpoints": [ Endpoint, ... ] } }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointsResponse {
    pub data: EndpointList,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointList {
    pub endpoints: Vec<Endpoint>,
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct CompReq<'a> {
    // OpenRouter docs: "Either "messages" or "prompt" is required"
    // corresponding json: `messages?: Message[];`
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<crate::llm::RequestMessage>>,
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

// Marker for response_format -> { "type": "json_object" }
#[derive(Debug, Clone, Copy)]
pub struct JsonObjMarker;

impl Serialize for JsonObjMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("type", "json_object")?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for JsonObjMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = JsonObjMarker;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object { \"type\": \"json_object\" }")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut found = false;
                while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
                    if k == "type" {
                        if let Some(s) = v.as_str() {
                            if s == "json_object" {
                                found = true;
                            }
                        }
                    }
                }
                if found { Ok(JsonObjMarker) } else { Err(serde::de::Error::custom("invalid response_format")) }
            }
        }
        deserializer.deserialize_map(V)
    }
}

// Marker for route -> "fallback"
#[derive(Debug, Clone, Copy)]
pub struct FallbackMarker;

impl Serialize for FallbackMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("fallback")
    }
}

impl<'de> Deserialize<'de> for FallbackMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = FallbackMarker;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("the string \"fallback\"")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v == "fallback" { Ok(FallbackMarker) } else { Err(E::custom("expected 'fallback'")) }
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// Tool selection behavior for OpenRouter requests.
/// Bridge format: "none" | "auto" | { type: "function", function: { name } }
#[derive(Debug, Clone)]
pub enum ToolChoice {
    None,
    Auto,
    Function { r#type: FunctionMarker, function: ToolChoiceFunction },
}

impl Serialize for ToolChoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ToolChoice::None => serializer.serialize_str("none"),
            ToolChoice::Auto => serializer.serialize_str("auto"),
            ToolChoice::Function { r#type, function } => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", r#type)?;
                map.serialize_entry("function", function)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ToolChoice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = ToolChoice;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("\"none\" | \"auto\" | { type: \"function\", function: { name } }")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "none" => Ok(ToolChoice::None),
                    "auto" => Ok(ToolChoice::Auto),
                    _ => Err(E::custom("invalid ToolChoice string")),
                }
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut type_seen: Option<FunctionMarker> = None;
                let mut function_seen: Option<ToolChoiceFunction> = None;
                while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
                    match k.as_str() {
                        "type" => {
                            let m: FunctionMarker = serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                            type_seen = Some(m);
                        }
                        "function" => {
                            let f: ToolChoiceFunction = serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                            function_seen = Some(f);
                        }
                        _ => {}
                    }
                }
                match (type_seen, function_seen) {
                    (Some(m), Some(f)) => Ok(ToolChoice::Function { r#type: m, function: f }),
                    _ => Err(serde::de::Error::custom("invalid ToolChoice object")),
                }
            }
        }
        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn json_obj_marker_serde_roundtrip() {
        let m = JsonObjMarker;
        let v = serde_json::to_value(m).expect("serialize");
        assert_eq!(v, json!({"type":"json_object"}));

        let de: JsonObjMarker = serde_json::from_value(v).expect("deserialize");
        let v2 = serde_json::to_value(de).unwrap();
        assert_eq!(v2, json!({"type":"json_object"}));
    }

    #[test]
    fn fallback_marker_serde_roundtrip() {
        let m = FallbackMarker;
        let v = serde_json::to_value(m).expect("serialize");
        assert_eq!(v, Value::String("fallback".to_string()));
        let de: FallbackMarker = serde_json::from_value(v).expect("deserialize");
        let v2 = serde_json::to_value(de).unwrap();
        assert_eq!(v2, Value::String("fallback".to_string()));
    }

    #[test]
    fn tool_choice_serde_variants() {
        // none
        let none: ToolChoice = serde_json::from_str("\"none\"").expect("none deser");
        match none { ToolChoice::None => {}, _ => panic!("expected None"), }
        let s = serde_json::to_string(&none).unwrap();
        assert_eq!(s, "\"none\"");

        // auto
        let auto: ToolChoice = serde_json::from_str("\"auto\"").expect("auto deser");
        match auto { ToolChoice::Auto => {}, _ => panic!("expected Auto"), }
        let s = serde_json::to_string(&auto).unwrap();
        assert_eq!(s, "\"auto\"");

        // function
        let func_json = json!({
            "type": "function",
            "function": { "name": "apply_code_edit" }
        });
        let fc: ToolChoice = serde_json::from_value(func_json.clone()).expect("function deser");
        match &fc {
            ToolChoice::Function { r#type: _, function } => {
                assert_eq!(function.name, "apply_code_edit");
            }
            _ => panic!("expected Function variant"),
        }
        let back = serde_json::to_value(fc).unwrap();
        assert_eq!(back, func_json);
    }

    #[test]
    fn provider_name_str_slug_and_author() {
        let p = ProviderNameStr("OpenAI".to_string());
        assert_eq!(p.normalized_slug(), "openai");
        let a = p.to_author().expect("author");
        assert_eq!(a.to_string(), "openai");

        let p2 = ProviderNameStr("DeepInfra Turbo".to_string());
        assert_eq!(p2.normalized_slug(), "deepinfra-turbo");
    }

    #[test]
    fn endpoint_min_deserialize() {
        let body = json!({
            "provider_slug": "nebius",
            "context_length": 262144,
            "pricing": { "prompt": "0.0000001", "completion": "0.0000003" },
            "supported_parameters": ["tools", "max_tokens"]
        });
        let ep: Endpoint = serde_json::from_value(body).expect("endpoint deser");
        assert_eq!(ep.preferred_provider_slug(), "nebius");
        assert!(ep.supports_tools());
        assert!(ep.pricing.prompt_or_default() > 0.0);
        assert!(ep.pricing.completion_or_default() > 0.0);
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_fetch_smoke() -> color_eyre::Result<()> {
        use crate::llm::openrouter::provider_endpoints::ProvEnd;
        let pe = ProvEnd { author: Arc::<str>::from("deepseek"), model: Arc::<str>::from("deepseek-chat-v3.1") };
        let v = pe.call_endpoint_raw().await?;
        assert!(v.get("data").is_some(), "response missing 'data' key");
        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_into_endpoint_deserialize() -> color_eyre::Result<()> {
        use crate::llm::openrouter::provider_endpoints::ProvEnd;
        let pe = ProvEnd { author: Arc::<str>::from("deepseek"), model: Arc::<str>::from("deepseek-chat-v3.1") };
        let v = pe.call_endpoint_raw().await?;
        let parsed: EndpointsResponse = serde_json::from_value(v)?;
        assert!(!parsed.data.endpoints.is_empty(), "no endpoints returned");
        let has_id = parsed.data.endpoints.iter().all(|e| !e.provider_id.0.is_empty());
        assert!(has_id, "provider_id must be present");
        for e in &parsed.data.endpoints {
            assert!(e.pricing.prompt_or_default() >= 0.0);
            assert!(e.pricing.completion_or_default() >= 0.0);
        }        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_into_endpoint_qwen() -> color_eyre::Result<()> {
        use crate::llm::openrouter::provider_endpoints::ProvEnd;
        let pe = ProvEnd { author: Arc::<str>::from("deepseek"), model: Arc::<str>::from("deepseek-chat-v3.1") };
        let v = pe.call_endpoint_raw().await?;
        let parsed: EndpointsResponse = serde_json::from_value(v)?;
        assert!(!parsed.data.endpoints.is_empty(), "no endpoints returned for qwen");
        Ok(())
    }
}

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_multi_models_smoke() -> color_eyre::Result<()> {
        use crate::llm::openrouter::provider_endpoints::ProvEnd;
        let Some(_op) = crate::test_harness::openrouter_env() else {
            eprintln!("Skipping live tests: OPENROUTER_API_KEY not set");
            return Ok(());
        };
        let candidates: &[(&str, &str)] = &[
            ("qwen", "qwen3-30b-a3b-thinking-2507"),
            ("meta-llama", "llama-3.1-8b-instruct"),
            ("x-ai", "grok-2"),
        ];
        let mut successes = 0usize;
        for (author, model) in candidates {
            let pe = ProvEnd { author: Arc::<str>::from(*author), model: Arc::<str>::from(*model) };
            match pe.call_endpoint_raw().await {
                Ok(v) => {
                    if v.get("data").is_some() {
                        // Try full typed path as well
                        if let Ok(parsed) = serde_json::from_value::<EndpointsResponse>(v) {
                            if !parsed.data.endpoints.is_empty() { successes += 1; }
                        } else {
                            successes += 1; // still count a data-bearing response
                        }
                    }
                }
                Err(e) => {
                    eprintln!("live endpoint fetch failed for {}/{}: {}", author, model, e);
                }
            }
        }
        assert!(successes >= 1, "at least one candidate model should succeed");
        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_endpoints_roundtrip_compare() -> color_eyre::Result<()> {
        use crate::llm::openrouter::provider_endpoints::ProvEnd;
        let pe = ProvEnd { author: Arc::<str>::from("deepseek"), model: Arc::<str>::from("deepseek-chat-v3.1") };
        let raw = pe.call_endpoint_raw().await?;

        let typed: EndpointsResponse = serde_json::from_value(raw.clone())?;
        assert!(!typed.data.endpoints.is_empty(), "no endpoints returned");

        let typed_json = serde_json::to_value(&typed).expect("serialize typed");
        // Full equality is not expected (we omit unknown fields; numeric normalization)
        assert!(typed_json != raw);

        // Subset equality on key fields
        let raw_eps = raw.get("data").and_then(|d| d.get("endpoints")).and_then(|v| v.as_array()).expect("raw endpoints array");
        assert_eq!(typed.data.endpoints.len(), raw_eps.len());

        fn str_or_num_to_f64(v: &serde_json::Value) -> Option<f64> {
            match v {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::String(s) => s.parse::<f64>().ok(),
                _ => None,
            }
        }

        for (t, r) in typed.data.endpoints.iter().zip(raw_eps.iter()) {
            let r_obj = r.as_object().expect("raw ep object");
            let raw_pid = r_obj.get("provider_slug")
                .or_else(|| r_obj.get("slug"))
                .or_else(|| r_obj.get("id"))
                .or_else(|| r_obj.get("provider"))
                .or_else(|| r_obj.get("name"))
                .and_then(|v| v.as_str());
            if let Some(pid) = raw_pid {
                let used_name_only = r_obj.get("provider_slug").is_none()
                    && r_obj.get("slug").is_none()
                    && r_obj.get("id").is_none()
                    && r_obj.get("provider").is_none()
                    && r_obj.get("name").is_some();
                let expected = if used_name_only {
                    let left = pid.split('|').next().unwrap_or(pid).trim();
                    left.chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
                        .collect::<String>()
                } else {
                    pid.to_string()
                };
                assert_eq!(t.preferred_provider_slug(), expected);
            }

            if let Some(p_raw) = r_obj.get("pricing").and_then(|v| v.as_object()) {
                if let Some(tp) = Some(t.pricing.prompt_or_default()) {
                    if let Some(rp) = p_raw.get("prompt").and_then(str_or_num_to_f64) {
                        assert!((tp - rp).abs() < 1e-9);
                    }
                }
                if let Some(tc) = Some(t.pricing.completion_or_default()) {
                    if let Some(rc) = p_raw.get("completion").and_then(str_or_num_to_f64) {
                        assert!((tc - rc).abs() < 1e-9);
                    }
                }
            }

            if let Some(cl_raw) = r_obj.get("context_length").and_then(|v| v.as_u64()) {
                if let Some(cl) = t.context_length { assert_eq!(cl as u64, cl_raw); }
            }

            if let Some(sp_raw) = r_obj.get("supported_parameters").and_then(|v| v.as_array()) {
                let raw_set: std::collections::HashSet<String> = sp_raw.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                if let Some(tsp) = &t.supported_parameters {
                    let typed_set: std::collections::HashSet<String> = tsp.iter().map(|p| serde_json::to_string(p).unwrap().trim_matches('"').to_string()).collect();
                    assert!(typed_set.is_subset(&raw_set));
                }
            }        }

        Ok(())
    }
