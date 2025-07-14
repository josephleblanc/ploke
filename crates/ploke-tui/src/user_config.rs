use ploke_embed::{
    config::{CozoConfig, HuggingFaceConfig, LocalModelConfig, OpenAIConfig},
    indexer::{CozoBackend, EmbeddingProcessor, EmbeddingSource},
    local::{DevicePreference, EmbeddingConfig as LocalEmbeddingConfig, LocalEmbedder},
    providers::{hugging_face::HuggingFaceBackend, openai::OpenAIBackend},
};
use reqwest::Request;
use serde::Deserialize;

use crate::llm::RequestMessage;

pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
pub const DEFAULT_MODEL: &str = "qwen/qwq-32b:free";

#[derive(Debug, Clone, Deserialize, Copy, PartialEq, Eq, Default)]
pub enum CommandStyle {
    NeoVim,
    #[default]
    Slash,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub command_style: CommandStyle,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
}

impl Config {
    pub fn load_embedding_processor(
        &self
    ) -> Result<EmbeddingProcessor, color_eyre::eyre::Error> {
        let processor = match self.embedding {
            EmbeddingConfig {
                local: Some(ref local_config),
                ..
            } => {
                let embedder_config = LocalEmbeddingConfig {
                    model_id: local_config.model_id.clone(),
                    revision: None,
                    device_preference: DevicePreference::Auto,
                    cuda_device_index: 0,
                    allow_fallback: true,
                    approximate_gelu: false,
                    use_pth: false,
                    model_batch_size: 8,
                    max_length: None,
                };
                let embedder = LocalEmbedder::new(embedder_config)?;
                EmbeddingProcessor::new(EmbeddingSource::Local(embedder))
            }
            EmbeddingConfig {
                hugging_face: Some(ref hf),
                ..
            } => EmbeddingProcessor::new(EmbeddingSource::HuggingFace(HuggingFaceBackend::new(hf))),
            EmbeddingConfig {
                openai: Some(ref openai),
                ..
            } => EmbeddingProcessor::new(EmbeddingSource::OpenAI(OpenAIBackend::new(openai))),
            EmbeddingConfig {
                cozo: Some(ref cozo),
                ..
            } => EmbeddingProcessor::new(EmbeddingSource::Cozo(CozoBackend::new(cozo))),
            _ => {
                let embedder_config = LocalEmbeddingConfig {
                    model_id: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                    revision: None,
                    device_preference: DevicePreference::Auto,
                    cuda_device_index: 0,
                    allow_fallback: true,
                    approximate_gelu: false,
                    use_pth: false,
                    model_batch_size: 8,
                    max_length: None,
                };
                let default_embedder = LocalEmbedder::new(embedder_config)?;
                EmbeddingProcessor::new(EmbeddingSource::Local(default_embedder))
            }
        };
        Ok(processor)
    }
}

// NEW: Embedding configuration
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EmbeddingConfig {
    pub local: Option<LocalModelConfig>,
    pub hugging_face: Option<HuggingFaceConfig>,
    pub openai: Option<OpenAIConfig>,
    pub cozo: Option<CozoConfig>,
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
    pub fn form_request(
        &self,
        message: RequestMessage,
    ) -> Result<Request, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let request = client
            .post(chat_url())
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
