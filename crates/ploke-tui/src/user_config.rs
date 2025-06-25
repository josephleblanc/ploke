use reqwest::{Request, RequestBuilder};
use serde::Deserialize;

use crate::llm::RequestMessage;

pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
pub const DEFAULT_MODEL: &str = "qwen/qwq-32b:free";


#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub provider: ProviderConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// The API key for the provider.
    pub api_key: String,
    /// The base URL for the API endpoint.
    /// For OpenRouter, this would be "https://openrouter.ai/api/v1".
    #[serde(default = "default_base_url")]
    pub base_url: String,
    /// The model to use, e.g., "openai/gpt-4o" or "anthropic/claude-3-haiku".
    #[serde(default = "default_model")]
    pub model: String,
}

impl ProviderConfig {
    // TODO: proper error handling
    pub fn form_request(&self, message: RequestMessage) -> Result<Request, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let request = client.post(chat_url())
            .bearer_auth(self.api_key.clone())
            .json(&message)
            .build()?;
        Ok(request)
    }
}

fn default_base_url() -> String {
    "https://openrouter.ai/api/v1".to_string()
}

fn chat_url() -> String {
    "https://openrouter.ai/api/v1/chat/completions".to_string()
}

fn default_model() -> String {
    "qwen/qwq-32b:free".to_string()
}

// Add a default implementation for when the config file is missing
impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(), // Will be loaded from env var
            base_url: default_base_url(),
            model: default_model(),
        }
    }
}
