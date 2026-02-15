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

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncatePolicy {
    Truncate,
    Reject,
    PassThrough,
}

impl Default for TruncatePolicy {
    fn default() -> Self {
        Self::Truncate
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct OpenRouterConfig {
    /// OpenRouter model id, e.g. `openai/text-embedding-3-small`.
    pub model: String,
    /// Expected embedding dimension. If set, the backend enforces responses match exactly.
    /// This does not necessarily mean the backend will request truncation from OpenRouter.
    pub dimensions: Option<usize>,
    /// Optional router-specific `dimensions` request parameter (OpenRouter-side truncation).
    ///
    /// Leave unset unless you specifically want OpenRouter/providers to return vectors truncated
    /// to this size. Many embedding models/providers do not support this parameter; when set,
    /// requests may fail with "No successful provider responses" from OpenRouter.
    #[serde(default)]
    pub request_dimensions: Option<usize>,
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
    /// Controls how the backend handles overlong snippets.
    #[serde(default)]
    pub truncate_policy: TruncatePolicy,
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            dimensions: None,
            request_dimensions: None,
            max_in_flight: default_openrouter_max_in_flight(),
            requests_per_second: None,
            max_attempts: default_openrouter_max_attempts(),
            initial_backoff_ms: default_openrouter_initial_backoff_ms(),
            max_backoff_ms: default_openrouter_max_backoff_ms(),
            input_type: None,
            timeout_secs: default_openrouter_timeout_secs(),
            truncate_policy: TruncatePolicy::Truncate,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openrouter_config_default_is_safe_for_runtime_use() {
        let cfg = OpenRouterConfig::default();
        assert_eq!(cfg.max_in_flight, 2);
        assert_eq!(cfg.max_attempts, 5);
        assert_eq!(cfg.initial_backoff_ms, 250);
        assert_eq!(cfg.max_backoff_ms, 10_000);
        assert_eq!(cfg.timeout_secs, 30);
        assert_eq!(cfg.request_dimensions, None);
    }
}
