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
use std::time::Duration;

// llm types and router defaults
use crate::llm::ProviderSlug;
pub use crate::llm::registry::user_prefs::ModelRegistryStrictness;
use crate::llm::router_only::default_model;
use crate::llm::router_only::openrouter::OpenRouter;
use crate::llm::{Router as _, registry::user_prefs::RegistryPrefs};
use crate::tools::ToolVerbosity;
use ploke_rag::{MmrConfig, RetrievalStrategy, RrfConfig};

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
    /// Local embedding execution parameters (device/batch tuning).
    #[serde(default)]
    pub embedding_local: LocalEmbeddingTuning,
    #[serde(default)]
    pub editing: EditingConfig,
    #[serde(default)]
    pub ploke_editor: Option<String>,
    #[serde(default)]
    pub context_management: CtxPrefs,
    #[serde(default)]
    pub tooling: ToolingConfig,
    /// Chat/tool loop behavior and retry/timeouts.
    #[serde(default)]
    pub chat_policy: ChatPolicy,
    /// Retrieval/fusion preferences (advanced).
    #[serde(default)]
    pub rag: RagUserConfig,
    /// Default token limit for tool-initiated LLM requests.
    #[serde(default = "default_token_limit")]
    pub token_limit: u32,
    /// Tool retry count when the model misfires.
    #[serde(default = "default_tool_retries")]
    pub tool_retries: u32,
    /// Request timeout for chat HTTP calls (seconds).
    #[serde(default = "default_llm_timeout_secs")]
    pub llm_timeout_secs: u64,
}

/// Tooling-specific configuration values.
///
/// Timeouts are in seconds. A value of `0` disables the timeout for that command.
///
/// ```rust
/// use ploke_tui::user_config::ToolingConfig;
///
/// let cfg = ToolingConfig {
///     cargo_check_timeout_secs: 120,
///     cargo_test_timeout_secs: 900,
/// };
/// assert_eq!(cfg.cargo_check_timeout_secs, 120);
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolingConfig {
    #[serde(default = "default_cargo_check_timeout_secs")]
    pub cargo_check_timeout_secs: u64,
    #[serde(default = "default_cargo_test_timeout_secs")]
    pub cargo_test_timeout_secs: u64,
}

impl Default for ToolingConfig {
    fn default() -> Self {
        Self {
            cargo_check_timeout_secs: default_cargo_check_timeout_secs(),
            cargo_test_timeout_secs: default_cargo_test_timeout_secs(),
        }
    }
}

fn default_cargo_check_timeout_secs() -> u64 {
    60
}

fn default_cargo_test_timeout_secs() -> u64 {
    600
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CtxPrefs {
    #[serde(default)]
    pub strategy: CtxStrategy,
    #[serde(default = "default_max_leased_tokens")]
    pub max_leased_tokens: usize,
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

impl Default for CtxPrefs {
    fn default() -> Self {
        Self {
            strategy: CtxStrategy::default(),
            max_leased_tokens: default_max_leased_tokens(),
        }
    }
}

impl CtxPrefs {
    pub fn validated(self) -> Self {
        let max_leased_tokens = self.max_leased_tokens.clamp(128, 200_000);
        Self {
            strategy: self.strategy,
            max_leased_tokens,
        }
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
                    device_preference: self.embedding_local.device_preference,
                    cuda_device_index: self.embedding_local.cuda_device_index,
                    allow_fallback: self.embedding_local.allow_fallback,
                    approximate_gelu: self.embedding_local.approximate_gelu,
                    use_pth: self.embedding_local.use_pth,
                    model_batch_size: self.embedding_local.model_batch_size.max(1),
                    max_length: self.embedding_local.max_length,
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
                    device_preference: self.embedding_local.device_preference,
                    cuda_device_index: self.embedding_local.cuda_device_index,
                    allow_fallback: self.embedding_local.allow_fallback,
                    approximate_gelu: self.embedding_local.approximate_gelu,
                    use_pth: self.embedding_local.use_pth,
                    model_batch_size: self.embedding_local.model_batch_size.max(1),
                    max_length: self.embedding_local.max_length,
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
/// Local embedding execution tuning (advanced).
pub struct LocalEmbeddingTuning {
    pub device_preference: DevicePreference,
    pub cuda_device_index: usize,
    pub allow_fallback: bool,
    pub approximate_gelu: bool,
    pub use_pth: bool,
    pub model_batch_size: usize,
    pub max_length: Option<usize>,
}

impl Default for LocalEmbeddingTuning {
    fn default() -> Self {
        Self {
            device_preference: DevicePreference::Auto,
            cuda_device_index: 0,
            allow_fallback: true,
            approximate_gelu: false,
            use_pth: false,
            model_batch_size: 8,
            max_length: None,
        }
    }
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

/// Chat/tool loop policy (timeouts, retries, limits).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatPolicy {
    #[serde(default = "default_tool_call_timeout_secs")]
    pub tool_call_timeout_secs: u64,
    #[serde(default = "default_tool_call_chain_limit")]
    pub tool_call_chain_limit: usize,
    #[serde(default)]
    pub retry_without_tools_on_404: bool,
    #[serde(default = "default_chat_timeout_strategy")]
    pub timeout_strategy: ChatTimeoutStrategy,
    #[serde(default = "default_timeout_base_secs")]
    pub timeout_base_secs: u64,
    #[serde(default = "default_error_retry_limit")]
    pub error_retry_limit: u32,
    #[serde(default = "default_length_retry_limit")]
    pub length_retry_limit: u32,
    #[serde(default = "default_length_continue_prompt")]
    pub length_continue_prompt: String,
}

impl Default for ChatPolicy {
    fn default() -> Self {
        Self {
            tool_call_timeout_secs: default_tool_call_timeout_secs(),
            tool_call_chain_limit: default_tool_call_chain_limit(),
            retry_without_tools_on_404: false,
            timeout_strategy: default_chat_timeout_strategy(),
            timeout_base_secs: default_timeout_base_secs(),
            error_retry_limit: default_error_retry_limit(),
            length_retry_limit: default_length_retry_limit(),
            length_continue_prompt: default_length_continue_prompt(),
        }
    }
}

impl ChatPolicy {
    /// Clamp invalid values while preserving user intent.
    pub fn validated(self) -> Self {
        let tool_call_timeout_secs = self.tool_call_timeout_secs.clamp(5, 600);
        let tool_call_chain_limit = self.tool_call_chain_limit.clamp(1, 500);
        let timeout_base_secs = self.timeout_base_secs.clamp(5, 600);
        let error_retry_limit = self.error_retry_limit.min(10);
        let length_retry_limit = self.length_retry_limit.min(5);
        let timeout_strategy = self.timeout_strategy.validated();
        Self {
            tool_call_timeout_secs,
            tool_call_chain_limit,
            retry_without_tools_on_404: self.retry_without_tools_on_404,
            timeout_strategy,
            timeout_base_secs,
            error_retry_limit,
            length_retry_limit,
            length_continue_prompt: self.length_continue_prompt,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ChatTimeoutStrategy {
    Backoff { attempts: Option<usize> },
    FixedRetry { attempts: usize },
    Strict,
}

impl Default for ChatTimeoutStrategy {
    fn default() -> Self {
        ChatTimeoutStrategy::FixedRetry { attempts: 3 }
    }
}

impl ChatTimeoutStrategy {
    fn validated(self) -> Self {
        match self {
            ChatTimeoutStrategy::Backoff { attempts } => ChatTimeoutStrategy::Backoff {
                attempts: attempts.map(|a| a.min(10)),
            },
            ChatTimeoutStrategy::FixedRetry { attempts } => ChatTimeoutStrategy::FixedRetry {
                attempts: attempts.clamp(0, 10),
            },
            ChatTimeoutStrategy::Strict => ChatTimeoutStrategy::Strict,
        }
    }
}

/// Retrieval/fusion preferences exposed to users.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RagUserConfig {
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default = "default_rag_per_part_max_tokens")]
    pub per_part_max_tokens: usize,
    #[serde(default)]
    pub strategy: RetrievalStrategyUser,
    #[serde(default = "default_bm25_timeout_ms")]
    pub bm25_timeout_ms: u64,
    #[serde(default = "default_bm25_retry_backoff_ms")]
    pub bm25_retry_backoff_ms: Vec<u64>,
    #[serde(default)]
    pub strict_bm25_by_default: bool,
    #[serde(default)]
    pub rrf: RrfConfig,
    #[serde(default)]
    pub mmr: Option<MmrConfig>,
}

impl Default for RagUserConfig {
    fn default() -> Self {
        Self {
            top_k: default_top_k(),
            per_part_max_tokens: default_rag_per_part_max_tokens(),
            strategy: RetrievalStrategyUser::default(),
            bm25_timeout_ms: default_bm25_timeout_ms(),
            bm25_retry_backoff_ms: default_bm25_retry_backoff_ms(),
            strict_bm25_by_default: false,
            rrf: RrfConfig::default(),
            mmr: None,
        }
    }
}

impl RagUserConfig {
    pub fn validated(self) -> Self {
        let top_k = self.top_k.clamp(1, 200);
        let per_part_max_tokens = self.per_part_max_tokens.clamp(32, 4096);
        let bm25_timeout_ms = self.bm25_timeout_ms.clamp(10, 5_000);
        let bm25_retry_backoff_ms = if self.bm25_retry_backoff_ms.is_empty() {
            default_bm25_retry_backoff_ms()
        } else {
            self.bm25_retry_backoff_ms
        };
        let rrf = RrfConfig {
            k: self.rrf.k.clamp(1.0, 2000.0),
            weight_bm25: self.rrf.weight_bm25.clamp(0.0, 10.0),
            weight_dense: self.rrf.weight_dense.clamp(0.0, 10.0),
        };
        let mmr = self.mmr.map(|m| MmrConfig {
            lambda: m.lambda.clamp(0.0, 1.0),
            sim_metric: m.sim_metric,
            candidate_pool: m.candidate_pool.clamp(1, 500),
        });
        Self {
            top_k,
            per_part_max_tokens,
            strategy: self.strategy,
            bm25_timeout_ms,
            bm25_retry_backoff_ms,
            strict_bm25_by_default: self.strict_bm25_by_default,
            rrf,
            mmr,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum RetrievalStrategyUser {
    Dense,
    Sparse {
        strict: bool,
    },
    Hybrid {
        rrf: RrfConfig,
        mmr: Option<MmrConfig>,
    },
}

impl Default for RetrievalStrategyUser {
    fn default() -> Self {
        RetrievalStrategyUser::Hybrid {
            rrf: RrfConfig::default(),
            mmr: None,
        }
    }
}

impl RetrievalStrategyUser {
    pub fn to_runtime(&self) -> RetrievalStrategy {
        match self {
            RetrievalStrategyUser::Dense => RetrievalStrategy::Dense,
            RetrievalStrategyUser::Sparse { strict } => RetrievalStrategy::Sparse {
                strict: Some(*strict),
            },
            RetrievalStrategyUser::Hybrid { rrf, mmr } => RetrievalStrategy::Hybrid {
                rrf: *rrf,
                mmr: *mmr,
            },
        }
    }
}

fn default_tool_call_timeout_secs() -> u64 {
    30
}

fn default_tool_call_chain_limit() -> usize {
    100
}

fn default_chat_timeout_strategy() -> ChatTimeoutStrategy {
    ChatTimeoutStrategy::FixedRetry { attempts: 3 }
}

fn default_timeout_base_secs() -> u64 {
    30
}

fn default_error_retry_limit() -> u32 {
    2
}

fn default_length_retry_limit() -> u32 {
    1
}

fn default_length_continue_prompt() -> String {
    "Continue from where you left off. Do not repeat prior text.".to_string()
}

fn default_top_k() -> usize {
    15
}

fn default_rag_per_part_max_tokens() -> usize {
    256
}

fn default_bm25_timeout_ms() -> u64 {
    250
}

fn default_bm25_retry_backoff_ms() -> Vec<u64> {
    vec![50, 100]
}

fn default_token_limit() -> u32 {
    8_196
}

fn default_tool_retries() -> u32 {
    2
}

fn default_max_leased_tokens() -> usize {
    2_048
}

fn default_llm_timeout_secs() -> u64 {
    ploke_llm::LLM_TIMEOUT_SECS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_round_trip_defaults() {
        let toml = r#"
            command_style = "Slash"
            token_limit = 5000
            tool_retries = 3
            llm_timeout_secs = 40

            [chat_policy]
            tool_call_timeout_secs = 45
            tool_call_chain_limit = 50
            retry_without_tools_on_404 = true
            timeout_base_secs = 20
            error_retry_limit = 4
            length_retry_limit = 2
            length_continue_prompt = "go on"
            timeout_strategy = { FixedRetry = { attempts = 2 } }

            [rag]
            top_k = 20
            per_part_max_tokens = 160
            bm25_timeout_ms = 300
            bm25_retry_backoff_ms = [50, 100]
            strict_bm25_by_default = true
            strategy = { Hybrid = { rrf = { k = 40.0, weight_bm25 = 1.0, weight_dense = 2.0 }, mmr = { lambda = 0.7, sim_metric = { Cosine = {} }, candidate_pool = 30 } } }

            [context_management]
            max_leased_tokens = 2400
        "#;

        let cfg: UserConfig = toml::from_str(toml).expect("toml parses");
        assert_eq!(cfg.tool_retries, 3);
        assert_eq!(cfg.chat_policy.tool_call_chain_limit, 50);
        assert_eq!(cfg.rag.top_k, 20);
        assert_eq!(cfg.rag.per_part_max_tokens, 160);
        assert_eq!(cfg.context_management.max_leased_tokens, 2400);

        let serialized = toml::to_string(&cfg).expect("serialize");
        assert!(serialized.contains("tool_retries"));
    }
}
