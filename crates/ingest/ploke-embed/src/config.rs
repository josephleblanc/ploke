use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct LocalModelConfig {
    pub model_id: String,
}

// NEW: Backend config structs
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct HuggingFaceConfig {
    pub api_key: String,
    pub model: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct OpenRouterConfig {
    /// OpenRouter model id, e.g. `openai/text-embedding-3-small`.
    pub model: String,
    /// Expected embedding dimension. If set, the backend enforces responses match exactly.
    /// If you want OpenRouter-side truncation, set this and the backend will also request it
    /// via the router-specific `dimensions` field.
    pub dimensions: Option<usize>,
    /// Max in-flight embedding requests.
    #[serde(default = "default_openrouter_max_in_flight")]
    pub max_in_flight: usize,
    /// Optional requests/second cap.
    pub requests_per_second: Option<u32>,
    /// Max attempts for 429/529 retry.
    #[serde(default = "default_openrouter_max_attempts")]
    pub max_attempts: u32,
    /// Initial backoff in milliseconds.
    #[serde(default = "default_openrouter_initial_backoff_ms")]
    pub initial_backoff_ms: u64,
    /// Max backoff in milliseconds.
    #[serde(default = "default_openrouter_max_backoff_ms")]
    pub max_backoff_ms: u64,
    /// Optional hint to OpenRouter about the input type.
    pub input_type: Option<String>,
    /// Per-request timeout in seconds for embeddings.
    #[serde(default = "default_openrouter_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_openrouter_max_in_flight() -> usize {
    2
}
fn default_openrouter_max_attempts() -> u32 {
    5
}
fn default_openrouter_initial_backoff_ms() -> u64 {
    250
}
fn default_openrouter_max_backoff_ms() -> u64 {
    10_000
}
fn default_openrouter_timeout_secs() -> u64 {
    30
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct CozoConfig {
    pub api_key: Option<String>,
}
