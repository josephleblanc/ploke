// NOTE: This todo list applies to both this file and to the `ploke-tui/llm/mod.rs` file
//
//                        Additional steps needed for multi-model support:
//
//  1 - [ ] Model Configuration Schema - Add a way to define multiple model endpoints in config
//  2 - [ ] Provider Registry - Create a registry system to handle different API providers (OpenAI,
//      Anthropic, OpenRouter, etc.)
//  3 - [ ] Model-Specific Request Formatters - Each provider has different request/response formats
//  4 - [ ] Provider Selection UI - Add commands to switch between configured models
//  5 - [ ] Model Capability Detection - Handle different max tokens, context windows, etc.
//  6 - [ ] Rate Limiting per Provider - Different providers have different rate limits
//  7 - [ ] Error Handling per Provider - Different error codes and retry strategies
//  8 - [ ] Streaming Support Variations - Not all providers support streaming the same way
//  9 - [ ] Cost Tracking - Track usage/cost per provider
// 10 - [ ] Fallback Mechanism - Switch to backup providers on failure
//
//
//                                  Questions for clarification:
//
// 1 Priority: Should we focus on OpenRouter (which already supports many models via one API)
//   first, or build direct support for specific providers?
//   - Answer: Yes, we should focus on OpenRouter first.
// 2 Configuration: Do you want model configs in the main config.toml, or separate model-specific
//   config files?
//   - Answer: We want a single config.toml that has the specs for all the models.
// 3 Switching: Should users be able to switch models per-conversation, or only at startup?
//   - Answer: They should be able to switch per-message using a chat command.
//   - 3.1 (Switching) For per-message switching: Should we support model "aliases" (e.g., `!gpt` = `openrouter/gpt-4-turbo`)?
//      - Answer Yes, with user-definable aliases in config
// 4 Defaults: Should we maintain a curated list of "recommended" models with sensible defaults?
//   - Answer: Yes, this is important.
// 5 Environment: Should API keys be per-model or shared across providers?
//   - Answer: Shared across providers.
//
// The current `ProviderConfig` in user_config.rs seems designed for a single provider. Should we
// evolve this into a `Vec<ProviderConfig>` with a way to select the active one?
//
// More information on future development is in `ploke-tui/docs/model_configs.md`

use ploke_embed::{
    config::{CozoConfig, HuggingFaceConfig, LocalModelConfig, OpenAIConfig},
    indexer::{CozoBackend, EmbeddingProcessor, EmbeddingSource},
    local::{DevicePreference, EmbeddingConfig as LocalEmbeddingConfig, LocalEmbedder},
    providers::{hugging_face::HuggingFaceBackend, openai::OpenAIBackend},
};
use reqwest::Request;
use serde::Deserialize;

use crate::llm::{self, RequestMessage};

pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
pub const DEFAULT_MODEL: &str = "kimi-k2:free";

#[derive(Debug, Clone, Deserialize, Copy, PartialEq, Eq, Default)]
pub enum CommandStyle {
    NeoVim,
    #[default]
    Slash,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub registry: ProviderRegistry,
    #[serde(default)]
    pub command_style: CommandStyle,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
}

impl Config {
    pub fn load_embedding_processor(&self) -> Result<EmbeddingProcessor, color_eyre::eyre::Error> {
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
pub struct ProviderRegistry {
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_active_provider")]
    pub active_provider: String,
    #[serde(default)]
    pub aliases: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Unique identifier for this provider configuration
    pub id: String,
    /// The API key for the provider.
    pub api_key: String,
    /// The base URL for the API endpoint.
    /// For OpenRouter, this would be "https://openrouter.ai/api/v1".
    #[serde(default = "default_base_url")]
    pub base_url: String,
    /// The model to use, e.g., "openai/gpt-4o" or "anthropic/claude-3-haiku".
    #[serde(default = "default_model")]
    pub model: String,
    /// Optional display name for UI
    pub display_name: Option<String>,
    /// Provider type for request formatting
    #[serde(default)]
    pub provider_type: ProviderType,
    /// Optional per-provider LLM parameters (temperature, top-p, etc.)
    #[serde(default)]
    pub llm_params: Option<crate::llm::LLMParameters>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub enum ProviderType {
    #[default]
    OpenRouter,
    OpenAI,
    Anthropic,
    Custom,
}

impl ProviderRegistry {
    /// Returns the currently active provider configuration.
    pub fn get_active_provider(&self) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == self.active_provider)
    }

    /// Returns a provider either by id or by alias.
    pub fn get_provider_by_alias(&self, alias: &str) -> Option<&ProviderConfig> {
        if let Some(provider_id) = self.aliases.get(alias) {
            self.providers.iter().find(|p| p.id == *provider_id)
        } else {
            self.providers.iter().find(|p| p.id == *alias)
        }
    }

    /// Merge the curated defaults with user overrides.
    ///
    /// Any user-defined provider with the same `id` *replaces* the corresponding default;
    /// any missing fields in the user config (e.g. `api_key`) are **not** filled from the
    /// default to avoid accidental credential leakage.
    // LLM Generated, reviewed by - JL 25-07-17
    pub fn with_defaults(mut self) -> Self {
        for (id, default) in crate::llm::registry::DEFAULT_MODELS.iter() {
            if !self.providers.iter().any(|p| &p.id == id) {
                self.providers.push(default.clone());
            }
        }
        self
    }

    /// Attempts to switch the active provider.
    ///
    /// # Returns
    /// `true` if the provider id or alias was found and the switch succeeded,
    /// `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ploke_tui::user_config::{ProviderRegistry, ProviderConfig, ProviderType};
    /// # use std::collections::HashMap;
    /// let mut registry = ProviderRegistry {
    ///     providers: vec![
    ///         ProviderConfig {
    ///             id: "gpt4".into(),
    ///             api_key: "key".into(),
    ///             base_url: "https://openrouter.ai/api/v1".into(),
    ///             model: "openai/gpt-4".into(),
    ///             display_name: Some("GPT-4".into()),
    ///             provider_type: ProviderType::OpenRouter,
    ///         },
    ///         ProviderConfig {
    ///             id: "claude".into(),
    ///             api_key: "key".into(),
    ///             base_url: "https://openrouter.ai/api/v1".into(),
    ///             model: "anthropic/claude-3".into(),
    ///             display_name: Some("Claude 3".into()),
    ///             provider_type: ProviderType::OpenRouter,
    ///         },
    ///     ],
    ///     active_provider: "gpt4".into(),
    ///     aliases: HashMap::from([("gpt".into(), "gpt4".into())]),
    /// };
    ///
    /// assert!(registry.set_active("claude"));
    /// assert_eq!(registry.active_provider, "claude");
    ///
    /// // Switch via alias
    /// assert!(registry.set_active("gpt"));
    /// assert_eq!(registry.active_provider, "gpt4");
    ///
    /// // Unknown id fails
    /// assert!(!registry.set_active("unknown"));
    /// ```
    // - LLM Generated, reviewed by - JL 25-07-17
    // AI: Set active defined here
    pub fn set_active(&mut self, id_or_alias: &str) -> bool {
        let provider_id = self
            .aliases
            .get(id_or_alias)
            .map(|s| s.as_str())
            .unwrap_or(id_or_alias);
        if self.providers.iter().any(|p| p.id == *provider_id) {
            tracing::info!("Changing provider from {} to {}", self.active_provider, provider_id);
            self.active_provider = provider_id.to_string();
            true
        } else {
            false
        }
    }

    /// Returns a list of all available providers as `(id, display_name)` tuples.
    // - LLM Generated, reviewed by - JL 25-07-17
    pub fn list_available(&self) -> Vec<(String, String)> {
        self.providers
            .iter()
            .map(|p| {
                let name = p.display_name.as_ref().unwrap_or(&p.id);
                (p.id.clone(), name.clone())
            })
            .collect()
    }
}

fn default_active_provider() -> String {
    "default".to_string()
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
impl Default for ProviderRegistry {
    fn default() -> Self {
        let mut registry = Self {
            providers: vec![ProviderConfig {
                id: "default".to_string(),
                api_key: String::new(),
                base_url: default_base_url(),
                model: default_model(),
                display_name: Some("Default".to_string()),
                provider_type: ProviderType::OpenRouter,
                llm_params: Some(crate::llm::LLMParameters {
                    ..Default::default()
                }),
            }],
            active_provider: "default".to_string(),
            aliases: std::collections::HashMap::new(),
        };
        
        // Always include the curated defaults
        for (id, default) in crate::llm::registry::DEFAULT_MODELS.iter() {
            if !registry.providers.iter().any(|p| &p.id == id) {
                registry.providers.push(default.clone());
            }
        }
        
        registry
    }
}
