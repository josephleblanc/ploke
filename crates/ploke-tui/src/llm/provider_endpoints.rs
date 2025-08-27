use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEndpointsResponse {
    pub data: ModelEndpointsData,
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
    pub supported_parameters: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SupportedParameters {
    // AI: fill out this enum so it can be a good typed version of these supported values AI!
    // possible values for supported parameters
    //
    //"max_tokens",
    // "top_k",
    // "frequency_penalty",
    // "seed",
    // "tool_choice",
    // "repetition_penalty",
    // "logit_bias",
    // "logprobs",
    // "structured_outputs",
    // "presence_penalty",
    // "min_p",
    // "top_p",
    // "stop",
    // "top_logprobs",
    // "response_format",
    // "temperature",
    // "tools",
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
pub enum InputModality {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "video")]
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputModality {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "audio")]
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
