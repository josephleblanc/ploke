#![allow(missing_docs)]
//! User configuration (migrated to llm types and router traits).
//! Focuses on persistence, embedding/editing, and re-exports for router defaults.

use lazy_static::lazy_static;
use ploke_embed::{
    config::{CozoConfig, HuggingFaceConfig, LocalModelConfig, OpenAIConfig},
    indexer::{CozoBackend, EmbeddingProcessor, EmbeddingSource},
    local::{DevicePreference, EmbeddingConfig as LocalEmbeddingConfig, LocalEmbedder},
    providers::{hugging_face::HuggingFaceBackend, openai::OpenAIBackend},
};
use reqwest::Url;
use serde::{Deserialize, Serialize};

// llm types and router defaults
use crate::llm::ProviderSlug;
pub use crate::llm::registry::user_prefs::ModelRegistryStrictness;
use crate::llm::router_only::default_model;
use crate::llm::router_only::openrouter::OpenRouter;
use crate::llm::{Router as _, registry::user_prefs::RegistryPrefs};
use crate::tools::ToolVerbosity;

lazy_static! {
    // Parsed from llm OpenRouter BASE_URL
    pub static ref OPENROUTER_URL: Url =
        Url::parse(OpenRouter::BASE_URL).expect("Invalid OpenRouter base URL");
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
    // llm registry preferences (profiles, strictness, router prefs)
    #[serde(default)]
    pub registry: RegistryPrefs,
    #[serde(default)]
    pub command_style: CommandStyle,
    #[serde(default)]
    pub tool_verbosity: ToolVerbosity,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    #[serde(default)]
    pub editing: EditingConfig,
    #[serde(default)]
    pub ploke_editor: Option<String>,
    #[serde(default)]
    pub context_management: CtxPrefs,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CtxPrefs {
    strategy: CtxStrategy,
    // more here...
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CtxStrategy {
    /// Automatically prunes context items after turns to live runs out.
    Automatic(u16),
    /// Asks LLM to repin context items after turns to live runs out.
    Ask(u16),
    /// Never unpins context items - only manual command from user clears context.
    Unlimited,
}

pub const DEFAULT_CONTEXT_TURNS_TO_LIVE: u16 = 15;

impl Default for CtxStrategy {
    fn default() -> Self {
        Self::Automatic(DEFAULT_CONTEXT_TURNS_TO_LIVE)
    }
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
    /// Note: API keys are not stored in this config (llm resolves keys from env via Router).
    pub fn save_to_path(
        &self,
        path: &std::path::Path,
        _redact_keys: bool,
    ) -> color_eyre::Result<()> {
        let toml_str = toml::to_string_pretty(self)?;
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

// Embedding configuration (unchanged)
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
