#![allow(missing_docs)]
//! User configuration and provider registry.
//!
//! Dataflow:
//! - `UserConfig` is loaded at startup (toml/env), then merged with curated defaults.
//! - `ModelRegistry` manages providers, aliases, strictness, capabilities cache,
//!   and API key resolution from env or config.
//! - Commands (`/model *`, `/provider strictness`) mutate `ModelRegistry` at runtime,
//!   while save/load provide persistence with optional secret redaction.

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
// The current `ModelConfig` in user_config.rs seems designed for a single provider. Should we
// evolve this into a `Vec<ModelConfig>` with a way to select the active one?
//
// More information on future development is in `ploke-tui/docs/model_configs.md`

use lazy_static::lazy_static;
use ploke_embed::{
    config::{CozoConfig, HuggingFaceConfig, LocalModelConfig, OpenAIConfig},
    indexer::{CozoBackend, EmbeddingProcessor, EmbeddingSource},
    local::{DevicePreference, EmbeddingConfig as LocalEmbeddingConfig, LocalEmbedder},
    providers::{hugging_face::HuggingFaceBackend, openai::OpenAIBackend},
};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

use crate::llm::{self, openrouter_catalog, provider_endpoints::{SupportedParameters, SupportsTools}, providers::ProviderSlug, RequestMessage};

lazy_static! {
    pub static ref OPENROUTER_URL: Url = 
        Url::parse("https://openrouter.ai/api/v1/").expect("Invalid OpenRouter base URL");
}

pub fn openrouter_url() -> reqwest::Url {
    OPENROUTER_URL.clone()
}

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Default)]
pub enum CommandStyle {
    NeoVim,
    #[default]
    Slash,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct UserConfig {
    #[serde(default)]
    pub registry: ModelRegistry,
    #[serde(default)]
    pub command_style: CommandStyle,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    #[serde(default)]
    pub editing: EditingConfig,
    #[serde(default)]
    pub ploke_editor: Option<String>,
}

impl UserConfig {
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

    /// Save the configuration to the specified path.
    /// If `redact_keys` is true, provider API keys are removed before saving.
    pub fn save_to_path(
        &self,
        path: &std::path::Path,
        redact_keys: bool,
    ) -> color_eyre::Result<()> {
        let mut cfg = self.clone();

        if redact_keys {
            for p in &mut cfg.registry.providers {
                p.api_key.clear();
            }
        }

        let toml_str = toml::to_string_pretty(&cfg)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let mut tmp = tempfile::NamedTempFile::new_in(&dir)?;
        use std::io::Write as _;
        tmp.write_all(toml_str.as_bytes())?;
        tmp.flush()?;
        tmp.as_file().sync_all()?;
        tmp.persist(path)?;
        Ok(())
    }

    /// Load configuration from the specified path.
    pub fn load_from_path(path: &std::path::Path) -> color_eyre::Result<UserConfig> {
        let content = std::fs::read_to_string(path)?;
        let cfg: UserConfig = toml::from_str(&content)?;
        Ok(cfg)
    }

    /// Default config.toml path: ~/.config/ploke/config.toml
    pub fn default_config_path() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("ploke")
            .join("config.toml")
    }
}

// NEW: Embedding configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
/// Embedding backend configuration. Exactly one provider should be set.
pub struct EmbeddingConfig {
    pub local: Option<LocalModelConfig>,
    pub hugging_face: Option<HuggingFaceConfig>,
    pub openai: Option<OpenAIConfig>,
    pub cozo: Option<CozoConfig>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
/// Agent-specific editing controls used by proposal workflows.
pub struct EditingAgentConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_agent_min_confidence")]
    pub min_confidence: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
/// Editing UI and agent behavior configuration.
pub struct EditingConfig {
    #[serde(default)]
    pub auto_confirm_edits: bool,
    #[serde(default)]
    pub agent: EditingAgentConfig,
}

fn default_agent_min_confidence() -> f32 {
    0.8
}

#[derive(Debug, Clone, Deserialize, Serialize)]
/// Registry of configured providers with active selection, aliases,
/// cached capabilities, and selection policy (strictness).
pub struct ModelRegistry {
    pub providers: Vec<ModelConfig>,
    #[serde(default = "default_active_model_config")]
    pub active_model_config: String,
    #[serde(default)]
    pub aliases: std::collections::HashMap<String, String>,
    #[serde(skip)]
    pub capabilities: std::collections::HashMap<String, ModelCapabilities>,
    #[serde(default = "default_strictness")]
    pub strictness: ModelRegistryStrictness,
    #[serde(default)]
    pub require_tool_support: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
/// Concrete provider configuration (one model endpoint).
pub struct ModelConfig {
    /// Unique identifier for this provider configuration
    pub id: String,
    /// The API key for this specific provider
    pub api_key: String,
    /// Optional: upstream provider slug (for OpenRouter routing preferences)
    #[serde(default)]
    pub provider_slug: Option<ProviderSlug>,
    /// Optional: provider-specific environment variable name for API key
    #[serde(default)]
    pub api_key_env: Option<String>,
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

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
/// Provider type used for request formatting and env-var resolution.
pub enum ProviderType {
    #[default]
    OpenRouter,
    OpenAI,
    Anthropic,
    Custom,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
/// Policy for allowed providers when switching the active provider.
pub enum ModelRegistryStrictness {
    /// Only allow selecting OpenRouter providers
    OpenRouterOnly,
    /// Allow OpenRouter and Custom providers (default)
    #[default]
    AllowCustom,
    /// No restrictions (future-friendly)
    AllowAny,
}

pub fn default_strictness() -> ModelRegistryStrictness {
    ModelRegistryStrictness::AllowCustom
}

impl ModelConfig {
    /// Resolve the actual API key to use, considering env vars and defaults
    pub fn resolve_api_key(&self) -> String {
        // 1. Check provider-specific env var if specified
        if let Some(env_var) = &self.api_key_env {
            if let Ok(key) = std::env::var(env_var) {
                return key;
            }
        }

        // 2. Check provider-type specific env vars
        match self.provider_type {
            ProviderType::OpenRouter => {
                if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
                    return key;
                }
            }
            ProviderType::OpenAI => {
                if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                    return key;
                }
            }
            ProviderType::Anthropic => {
                if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
                    return key;
                }
            }
            ProviderType::Custom => {
                // Check generic env vars
                if let Ok(key) = std::env::var("LLM_API_KEY") {
                    return key;
                }
            }
        }

        // 3. Fall back to the explicitly configured key
        self.api_key.clone()
    }
    pub fn with_api_key(mut self) -> Self {
        self.api_key = self.resolve_api_key();
        self
    }
}

#[derive(Debug, Clone, Default)]
/// Cached model capabilities/pricing for quick lookup in the UI and routing.
pub struct ModelCapabilities {
    pub supports_tools: bool,
    pub context_length: Option<u32>,
    pub input_cost_per_million: Option<f64>,
    pub output_cost_per_million: Option<f64>,
}

impl ModelRegistry {
    /// Returns the currently active provider configuration.
    pub fn get_active_model_config(&self) -> Option<&ModelConfig> {
        self.providers.iter().find(|p| p.id == self.active_model_config)
    }

    /// Returns a provider either by id or by alias.
    pub fn get_model_config_by_alias(&self, alias: &str) -> Option<&ModelConfig> {
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

    // TODO: Update doc tests with new fields
    /// Attempts to switch the active provider.
    ///
    /// # Returns
    /// `true` if the provider id or alias was found and the switch succeeded,
    /// `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use ploke_tui::user_config::{ModelRegistry, ModelConfig, ProviderType};
    /// # use std::collections::HashMap;
    /// let mut registry = ModelRegistry {
    ///     providers: vec![
    ///         ModelConfig {
    ///             id: "gpt4".into(),
    ///             api_key: "key".into(),
    ///             base_url: "https://openrouter.ai/api/v1".into(),
    ///             model: "openai/gpt-4".into(),
    ///             display_name: Some("GPT-4".into()),
    ///             provider_type: ProviderType::OpenRouter,
    ///         },
    ///         ModelConfig {
    ///             id: "claude".into(),
    ///             api_key: "key".into(),
    ///             base_url: "https://openrouter.ai/api/v1".into(),
    ///             model: "anthropic/claude-3".into(),
    ///             display_name: Some("Claude 3".into()),
    ///             provider_type: ProviderType::OpenRouter,
    ///         },
    ///     ],
    ///     active_model_config: "gpt4".into(),
    ///     aliases: HashMap::from([("gpt".into(), "gpt4".into())]),
    /// };
    ///
    /// assert!(registry.set_active("claude"));
    /// assert_eq!(registry.active_model_config, "claude");
    ///
    /// // Switch via alias
    /// assert!(registry.set_active("gpt"));
    /// assert_eq!(registry.active_model_config, "gpt4");
    ///
    /// // Unknown id fails
    /// assert!(!registry.set_active("unknown"));
    /// ```
    // - LLM Generated, reviewed by - JL 25-07-17
    pub fn set_active(&mut self, id_or_alias: &str) -> bool {
        let provider_id = self
            .aliases
            .get(id_or_alias)
            .map(|s| s.as_str())
            .unwrap_or(id_or_alias);

        let provider = if let Some(p) = self.providers.iter().find(|p| p.id == *provider_id) {
            p
        } else {
            return false;
        };

        // Enforce strictness policy
        let allowed = match self.strictness {
            ModelRegistryStrictness::OpenRouterOnly => {
                matches!(provider.provider_type, ProviderType::OpenRouter)
            }
            ModelRegistryStrictness::AllowCustom => {
                matches!(
                    provider.provider_type,
                    ProviderType::OpenRouter | ProviderType::Custom
                )
            }
            ModelRegistryStrictness::AllowAny => true,
        };

        if !allowed {
            tracing::warn!(
                "Provider '{}' not allowed by current strictness setting: {:?}",
                provider_id,
                self.strictness
            );
            return false;
        }

        tracing::info!(
            "Changing provider from {} to {}",
            self.active_model_config,
            provider_id
        );
        self.active_model_config = provider_id.to_string();
        true
    }

    /// Ensure all providers have their API keys loaded from environment variables
    pub fn load_api_keys(&mut self) {
        for provider in &mut self.providers {
            provider.api_key = provider.resolve_api_key();
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

    /// Query OpenRouter for current model capabilities and pricing and cache them.
    pub async fn refresh_from_openrouter(&mut self) -> color_eyre::Result<()> {
        // Find an API key to use
        let api_key = self
            .providers
            .iter()
            .find(|p| matches!(p.provider_type, ProviderType::OpenRouter))
            .map(|p| p.resolve_api_key())
            .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
            .unwrap_or_default();

        if api_key.is_empty() {
            tracing::warn!(
                "OPENROUTER_API_KEY not set and no OpenRouter provider configured; skipping model registry refresh"
            );
            return Ok(());
        }

        let client = Client::new();
        let models =
           openrouter_catalog::fetch_models(&client, openrouter_url(), &api_key).await?;

        self.capabilities.clear();
        for m in models {
            // Determine tool support with robust fallbacks:
            // - provider.supported_parameters contains "tools" (preferred)
            // - provider.capabilities.tools == true (fallback)
            // - model.supported_parameters contains "tools" (hint)
            // - model.capabilities.tools (legacy)
            let model_level_tools = m
                .supported_parameters
                .as_ref()
                .map(|v| v.supports_tools())
                .unwrap_or(false);

            let provider_tools = m
                .providers
                .as_ref()
                .map(|ps| {
                    ps.iter().any(|p| {
                        p.supported_parameters
                            .as_ref()
                            .map(|v| v.iter().any(|s| s.eq_ignore_ascii_case("tools")))
                            .unwrap_or_else(|| {
                                p.capabilities
                                    .as_ref()
                                    .and_then(|c| c.tools)
                                    .unwrap_or(false)
                            })
                    })
                })
                .unwrap_or(false);

            let supports_tools = model_level_tools
                || provider_tools
                || m.capabilities
                    .as_ref()
                    .and_then(|c| c.tools)
                    .unwrap_or(false);

            let caps = ModelCapabilities {
                supports_tools,
                context_length: m
                    .context_length
                    .or_else(|| m.top_provider.as_ref().and_then(|tp| tp.context_length)),
                input_cost_per_million: m.pricing.as_ref().map(|p| p.prompt),
                output_cost_per_million: m.pricing.as_ref().map(|p| p.completion),
            };
            self.capabilities.insert(m.id, caps);
        }
        Ok(())
    }

    /// Helper to check if a specific model is known to support tools.
    pub fn model_supports_tools(&self, model: &str) -> Option<bool> {
        self.capabilities.get(model).map(|c| c.supports_tools)
    }
}

pub fn default_active_model_config() -> String {
    "kimi-k2".to_string()
}

fn default_base_url() -> String {
    "https://openrouter.ai/api/v1".to_string()
}

fn chat_url() -> String {
    "https://openrouter.ai/api/v1/chat/completions".to_string()
}

pub fn default_model_id() -> String {
    "kimi-k2".to_string()
}

pub fn default_model() -> String {
    "moonshotai/kimi-k2".to_string()
}

// Add a default implementation for when the config file is missing
impl Default for ModelRegistry {
    fn default() -> Self {
        let mut registry = Self {
            providers: vec![ModelConfig {
                id: default_model_id(),
                api_key: String::new(),
                provider_slug: None,
                base_url: default_base_url(),
                model: default_model(),
                display_name: Some("Default".to_string()),
                provider_type: ProviderType::OpenRouter,
                llm_params: Some(crate::llm::LLMParameters {
                    ..Default::default()
                }),
                api_key_env: Some("OPENROUTER_API_KEY".to_string()),
            }],
            active_model_config: default_active_model_config(),
            aliases: std::collections::HashMap::new(),
            capabilities: std::collections::HashMap::new(),
            strictness: default_strictness(),
            require_tool_support: false,
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
