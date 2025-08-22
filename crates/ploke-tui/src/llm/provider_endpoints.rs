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
    #[serde(default)]
    prompt: f64,
    #[serde(default)]
    completion: f64,
    #[serde(default)]
    image: f64,
    #[serde(default)]
    request: f64,
    #[serde(default)]
    web_search: f64,
    #[serde(default)]
    internal_reasoning: f64,
    #[serde(default)]
    input_cache_read: f64,
    #[serde(default)]
    input_cache_write: f64,
}
