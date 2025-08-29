use serde::{Deserialize, Serialize};
use reqwest::Url;
use lazy_static::lazy_static;

// Expected response shape, from the official openrouter docs:
// {
//   "data": [
//     {
//       "id": "string",
//       "name": "string",
//       "created": 1741818122,
//       "description": "string",
//       "architecture": {
//         "input_modalities": [
//           "text",
//           "image"
//         ],
//         "output_modalities": [
//           "text"
//         ],
//         "tokenizer": "GPT",
//         "instruct_type": "string"
//       },
//       "top_provider": {
//         "is_moderated": true,
//         "context_length": 128000,
//         "max_completion_tokens": 16384
//       },
//       "pricing": {
//         "prompt": "0.0000007",
//         "completion": "0.0000007",
//         "image": "0",
//         "request": "0",
//         "web_search": "0",
//         "internal_reasoning": "0",
//         "input_cache_read": "0",
//         "input_cache_write": "0"
//       },
//       "canonical_slug": "string",
//       "context_length": 128000,
//       "hugging_face_id": "string",
//       "per_request_limits": {},
//       "supported_parameters": [
//         "string"
//       ]
//     }
//   ]
// }

lazy_static! {
    pub static ref OPENROUTER_MODELS_URL: Url = 
        Url::parse("https://openrouter.ai/api/v1/models").expect("Invalid OpenRouter models URL");
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Typed response to deserialize the response from:
/// https://openrouter.ai/api/v1/models
pub struct ModelEndpointsResponse {
    pub data: ModelEndpointsData,
}

impl ModelEndpointsResponse {
    fn url() -> Url {
        OPENROUTER_MODELS_URL.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEndpointsData {
    #[serde(default)]
    pub endpoints: Vec<ModelEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEndpoint {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub architecture: Architecture,
    #[serde(default)]
    pub top_provider: TopProvider,
    #[serde(default)]
    pub pricing: Pricing,
    #[serde(default)]
    pub canonical_slug: String,
    #[serde(default)]
    pub context_length: u64,
    #[serde(default)]
    pub hugging_face_id: String,
    #[serde(default)]
    pub per_request_limits: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub supported_parameters: Vec<SupportedParameters>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SupportedParameters {
    MaxTokens,
    TopK,
    FrequencyPenalty,
    Seed,
    ToolChoice,
    RepetitionPenalty,
    LogitBias,
    Logprobs,
    StructuredOutputs,
    PresencePenalty,
    MinP,
    TopP,
    Stop,
    TopLogprobs,
    ResponseFormat,
    Temperature,
    Tools,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Architecture {
    pub input_modalities: Vec<InputModality>,
    #[serde(default)]
    pub output_modalities: Vec<OutputModality>,
    #[serde(default)]
    pub tokenizer: String,
    #[serde(default)]
    pub instruct_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputModality {
    Text,
    Image,
    Audio,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputModality {
    Text,
    Image,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TopProvider {
    #[serde(default)]
    is_moderated: bool,
    #[serde(default)]
    context_length: u64,
    #[serde(default)]
    max_completion_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Pricing {
    // Accept either numbers or strings from the API, but keep a numeric type internally.
    #[serde(
        default,
        deserialize_with = "de_f64_from_str_or_num",
        serialize_with = "ser_f64_as_string"
    )]
    prompt: f64,
    #[serde(
        default,
        deserialize_with = "de_f64_from_str_or_num",
        serialize_with = "ser_f64_as_string"
    )]
    completion: f64,
    #[serde(
        default,
        deserialize_with = "de_f64_from_str_or_num",
        serialize_with = "ser_f64_as_string"
    )]
    image: f64,
    #[serde(
        default,
        deserialize_with = "de_f64_from_str_or_num",
        serialize_with = "ser_f64_as_string"
    )]
    request: f64,
    #[serde(
        default,
        deserialize_with = "de_f64_from_str_or_num",
        serialize_with = "ser_f64_as_string"
    )]
    web_search: f64,
    #[serde(
        default,
        deserialize_with = "de_f64_from_str_or_num",
        serialize_with = "ser_f64_as_string"
    )]
    internal_reasoning: f64,
    #[serde(
        default,
        deserialize_with = "de_f64_from_str_or_num",
        serialize_with = "ser_f64_as_string"
    )]
    input_cache_read: f64,
    #[serde(
        default,
        deserialize_with = "de_f64_from_str_or_num",
        serialize_with = "ser_f64_as_string"
    )]
    input_cache_write: f64,
}

// --- serde helpers for flexible number-or-string fields ---

fn de_f64_from_str_or_num<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Number(n) => n.as_f64().ok_or_else(|| {
            <D::Error as serde::de::Error>::custom("number not representable as f64")
        }),
        serde_json::Value::String(s) => s
            .trim()
            .parse::<f64>()
            .map_err(<D::Error as serde::de::Error>::custom),
        serde_json::Value::Null => Ok(0.0),
        other => Err(<D::Error as serde::de::Error>::custom(format!(
            "expected number or string, got {}",
            other
        ))),
    }
}

fn ser_f64_as_string<S>(v: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&v.to_string())
}

// Lightweight getters so callers can compute price hints without poking at serde internals.
impl Pricing {
    pub fn prompt_or_default(&self) -> f64 {
        self.prompt
    }
    pub fn completion_or_default(&self) -> f64 {
        self.completion
    }
}
