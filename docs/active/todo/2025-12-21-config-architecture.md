# Todo for adding user config 2025-12-21

## Categories

- UI aesthetic (e.g. colors)

- UI functional (e.g. tool verbosity displayed)


- Embeddings (e.g. current embedding model selected)

- Database (e.g. auto-swap to embedding model in db if present or not)

- Permissions (e.g. refuse file access outside focus dir or not, edit config auto vs. approval required)

- Keybindings (e.g. all keybindings in Normal Mode, all non-input related keybindings for Insert mode like ctrl + something if present)

- Network settings (e.g. timeout for llm calls, other chat loop settings)

- OpenRouter settings (large category, needs sub-categories)
  - Routing preferences (e.g. fastest/cheapest, zero-data retention policy required or not, etc)
  - API-bound LLMParameters (e.g. temperature, etc)

- RAG (e.g. prefer hybrid/dense/bm25, search knobs for each)

### Not yet configurable, add placeholder (dimmed)

- tools on/off (easy to add, should consider)

- Notifications (e.g. on llm tool loop ending)

- History & sessions: session restore, conversation branching defaults, autosave cadence, “clear sensitive history on exit”.

## Items to track in config:

### consts

crates/ploke-tui/lib.rs
```rust
/// The number of tool retries to allow if model fails to call tool correctly.
// TODO: Add this to user config
pub const TOOL_RETRIES: u32 = 2;

/// The default number of tokens per LLM request.
// TODO: Add this to user config
pub const TOOL_TOKEN_LIMIT: u32 = 512;

/// The default number of seconds for timeout on LLM request loop.
// TODO: Add this to user config
pub const LLM_TIMEOUT_SECS: u64 = 45;
```

crates/ploke-tui/src/user_config.rs
```rust
pub const DEFAULT_CONTEXT_TURNS_TO_LIVE: u16 = 15;
```

crates/ploke-tui/src/lib.rs
```rust
pub const TOP_K: usize = 15;
pub const TOOL_RETRIES: u32 = 2;
pub const TOOL_TOKEN_LIMIT: u32 = 512;
pub const LLM_TIMEOUT_SECS: u64 = 45;
```

### structs

- determine how the following structs should be interpreted as far as
user-facing config are concerned.

crates/ploke-tui/src/user_config.rs
```rust
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
    #[serde(default)]
    pub tooling: ToolingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolingConfig {
    #[serde(default = "default_cargo_check_timeout_secs")]
    pub cargo_check_timeout_secs: u64,
    #[serde(default = "default_cargo_test_timeout_secs")]
    pub cargo_test_timeout_secs: u64,
}

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
```

crates/ploke-tui/tests/editor_command.rs
```rust
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub llm_params: LLMParameters,
    pub model_registry: RegistryPrefs,
    pub active_model: ModelId,
    pub editing: EditingConfig,
    pub command_style: CommandStyle,
    pub tool_verbosity: ToolVerbosity,
    pub embedding: EmbeddingConfig,
    pub ploke_editor: Option<String>,
    pub tooling: ToolingConfig,
}
```

crates/ploke-rag/src/core/mod.rs
```rust
#[derive(Debug, Clone)]
pub struct RagConfig {
    pub bm25_timeout_ms: u64,
    pub bm25_retry_backoff_ms: Vec<u64>,
    pub strict_bm25_by_default: bool,
    pub rrf_default: RrfConfig,
    pub mmr_default: Option<MmrConfig>,
    pub score_norm: ScoreNorm,
    pub search_per_type: HashMap<NodeType, SearchParams>,
    pub assembly_policy: AssemblyPolicy,
    pub token_counter: Arc<dyn TokenCounter>,
    pub reranker: Option<Arc<dyn Reranker>>,
}
```


crates/ploke-rag/src/fusion/mod.rs
```rust
/// Configuration for Maximal Marginal Relevance (MMR).
#[derive(Debug, Clone, Copy)]
pub struct MmrConfig {
    /// Tradeoff between relevance and diversity: score = λ * rel - (1-λ) * max_sim
    pub lambda: f32,
    /// Similarity metric used for the diversity penalty.
    pub sim_metric: Similarity,
    /// Consider only the top-N candidates by relevance when selecting.
    pub candidate_pool: usize,
}

/// Configuration for Reciprocal Rank Fusion (RRF).
#[derive(Debug, Clone, Copy)]
pub struct RrfConfig {
    /// RRF smoothing parameter (typically ~60.0).
    pub k: f32,
    /// Weight for the BM25 modality.
    pub weight_bm25: f32,
    /// Weight for the dense modality.
    pub weight_dense: f32,
}
```

crates/ingest/ploke-embed/src/local/mod.rs
```rust
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub model_id: String,
    pub revision: Option<String>,
    pub device_preference: DevicePreference,
    pub cuda_device_index: usize,
    pub allow_fallback: bool,
    pub approximate_gelu: bool,
    pub use_pth: bool,
    pub model_batch_size: usize,   // NEW: Configurable batch size
    pub max_length: Option<usize>, // NEW: Optional max length override
}
```

crates/ploke-llm/src/manager/session.rs
```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChatHttpConfig {
    referer: &'static str,
    title: &'static str,
    pub timeout: Duration,
}
```

crates/ploke-rag/src/context/mod.rs
```rust
#[derive(Debug, Clone)]
pub struct AssemblyPolicy {
    pub ordering: Ordering,
    pub include_kinds: HashSet<ContextPartKind>,
    /// Optional per-node-type caps for fairness; not yet enforced in this initial version.
    pub per_type_caps: Option<HashMap<NodeType, usize>>,
    /// Allow overlapping snippet ranges; range handling is a no-op in this initial version.
    pub allow_overlap: bool,
    /// If true, IO errors during snippet retrieval are treated as fatal.
    pub strict_io: bool,
}
```

crates/ploke-tui/src/llm/manager/session.rs
```rust
#[derive(Clone, Copy, Debug)]
pub struct TuiToolPolicy {
    pub tool_call_timeout: ToolCallTimeout,
    pub tool_call_chain_limit: usize,
    pub retry_without_tools_on_404: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TuiTimeoutPolicy {
    duration: Option<Duration>,
    strategy: TimeoutStrategy,
}

#[derive(Clone, Copy, Debug)]
pub struct FinishPolicy {
    /// Timeout backoff/limit behavior for FinishReason::Timeout.
    timeout: TuiTimeoutPolicy,
    /// Retry policy for FinishReason::Error.
    error: TuiErrorPolicy,
    /// Retry policy for FinishReason::Length.
    length: TuiLengthPolicy,
    /// System prompt appended when retrying after FinishReason::Length.
    length_continue_prompt: &'static str,
}
```

crates/ploke-tui/src/user_config.rs
```rust
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
```

crates/ploke-llm/src/registry/user_prefs.rs
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelPrefs {
    // canonical id of form {author}/{model},
    // note that it does not include `:{variant}` as in `ModelId`, which varies by provider,
    // and is included under `ModelProfile` in `variant`
    pub model_key: ModelKey,
    pub default_profile: Option<ModelProfile>,
    // name from profiles
    pub profiles: HashMap<ProfileName, ModelProfile>,
    // API routing server, this gives us url, e.g. OpenRouter, OpenAI
    pub allowed_routers: Vec<RouterVariants>,
    // for explicit routing
    pub selected_endpoints: Vec<EndpointKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryPrefs {
    pub global_default_profile: Option<ModelProfile>,
    pub models: HashMap<ModelKey, ModelPrefs>,
    pub strictness: ModelRegistryStrictness,
    pub router_prefs: HashMap<RouterVariants, ProviderPreferences>,
}
```

crates/ploke-llm/src/router_only/openrouter/mod.rs
```rust
/// OpenRouter "provider" routing preferences.
/// The descriptions of each field are taken from OpenRouter documentation,
/// at https://openrouter.ai/docs/guides/routing/provider-selection
///
/// Last updated by JL 2025-12-14
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// List of provider slugs to try in order (e.g. ["anthropic", "openai"])
    pub order: Option<Vec<ProviderSlug>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether to allow backup providers to serve requests
    /// - true: (default) when the primary provider (or your custom providers in “order”) is
    ///   unavailable, use the next best provider.
    /// - false: use only the primary/custom provider, and return the upstream error if it’s
    ///   unavailable.
    pub allow_fallbacks: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Only use providers that support all parameters in your request.
    pub require_parameters: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Control whether to use providers that may store data. Learn more
    pub data_collection: Option<DataCollection>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Restrict routing to only ZDR (Zero Data Retention) endpoints. Learn more
    pub zdr: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Restrict routing to only models that allow text distillation. Learn more
    pub enforce_distillable_text: Option<HashSet<ProviderSlug>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// List of provider slugs to allow for this request.
    pub only: Option<HashSet<ProviderSlug>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// List of provider slugs to skip for this request.
    pub ignore: Option<HashSet<ProviderSlug>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// List of quantization levels to filter by (e.g. ["int4", "int8"])
    pub quantizations: Option<HashSet<Quant>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Sort providers by price or throughput. (e.g. "price" or "throughput")
    pub sort: Option<SortBy>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The maximum pricing you want to pay for this request.
    pub max_price: Option<MaxPrice>,
}

/// Embedding provider preferences with the basic set of provider preferences plus a couple extra
/// fields (min_throughput, max_latency) that seem to be embedding-provider specific.
///
/// Doc comments on fields from OpenRouter docs
///
/// Last updated by JL 2025-12-14
/// from https://openrouter.ai/docs/api/api-reference/embeddings/create-embeddings
pub struct EmbeddingProviderPrefs {
    #[serde(flatten)]
    base_provider_prefs: ProviderPreferences,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The minimum throughput (in tokens per second) required for this request. Only providers
    /// serving the model with at least this throughput will be used.
    min_throughput: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The maximum latency (in seconds) allowed for this request. Only providers serving the model
    /// with better than this latency will be used.
    max_latency: Option<f64>,
}
```

verbosity of tool call rendered to user in UI

crates/ploke-tui/src/llm/manager/loop_error.rs
```rust
#[derive(Clone, Debug)]
pub enum Verbosity {
    Terse,
    Normal,
    Verbose,
}
```

(non-exhaustive) set of commands included in the help commands

crates/ploke-tui/src/app/commands/mod.rs
```rust
pub const HELP_COMMANDS: &str = r#"Available commands:
    index start [directory] - Run workspace indexing on specified directory
                              (defaults to current dir)
    index pause - Pause indexing
    index resume - Resume indexing
    index cancel - Cancel indexing
    check api - Check API key configuration

    model list - List available models
    model info - Show active model/provider settings
    model use <name> - Switch to a configured model by alias or id
    model refresh [--local] - Refresh model registry (OpenRouter) and API keys; use --local to skip network
    model load [path] - Load configuration from path (default: ~/.config/ploke/config.toml)
    model save [path] [--with-keys] - Save configuration; omit --with-keys to redact secrets
    model search <keyword> - Search OpenRouter models and open interactive browser
    embedding search <keyword> - Search OpenRouter embedding models and open interactive browser
    model providers <model_id> - List provider endpoints for a model and show tool support and slugs
    provider strictness <openrouter-only|allow-custom|allow-any> - Restrict selectable providers
    provider tools-only <on|off> - Enforce using only models/providers that support tool calls
    provider select <model_id> <provider_slug> - Pin a model to a specific provider endpoint
    provider pin <model_id> <provider_slug> - Alias for 'provider select'

    bm25 rebuild - Rebuild sparse BM25 index
    bm25 status - Show sparse BM25 index status
    bm25 save <path> - Save sparse index sidecar to file
    bm25 load <path> - Load sparse index sidecar from file
    bm25 search <query> [top_k] - Search with BM25
    hybrid <query> [top_k] - Hybrid (BM25 + dense) search

    preview [on|off|toggle] - Toggle context preview panel
    edit preview mode <code|diff> - Set edit preview mode for proposals
    edit preview lines <N> - Set max preview lines per section
    edit auto <on|off> - Toggle auto-approval of staged edits
    edit approve <request_id> - Apply staged code edits with this request ID
    edit deny <request_id> - Deny and discard staged code edits
    tool verbosity <minimal|normal|verbose|toggle> - Set or cycle tool output verbosity

    help - Show this help
    help <topic> - Topic-specific help, e.g. 'help model', 'help edit', 'help bm25', 'help provider', 'help index'

    Keyboard shortcuts (Normal mode):
    q - Quit
    i - Enter insert mode
    : - Enter command mode (vim-style)
    m - Quick model selection
    ? - Show this help
    / - Quick hybrid search prompt
    P - Toggle context preview
    v - Cycle tool verbosity (minimal -> normal -> verbose)
    j/↓ - Navigate down (selection)
    k/↑ - Navigate up (selection)
    J - Page down (scroll)
    K - Page up (scroll)
    G - Go to bottom (scroll)
    gg - Go to top (scroll)
    h/← - Navigate branch previous
    l/→ - Navigate branch next
    Del - Delete selected conversation item
    Ctrl+n - Scroll down one line
    Ctrl+p - Scroll up one line

    Model Browser (opened via 'model search <keyword>'):
      ↑/↓ or j/k - Navigate
      Enter/Space - Expand/collapse details
      s - Select and set active model
      q/Esc - Close

    Embedding Browser (opened via 'embedding search <keyword>'):
      ↑/↓ or j/k - Navigate
      Enter/Space - Expand/collapse details
      s - Select embedding model (records selection in UI log)
      q/Esc - Close

    Insert mode history:
      ↑/↓ - Navigate your previous user messages in this conversation
      PageUp/PageDown - Jump to oldest/newest user message in history
"#;

## Current user-config surface (2025-12-21 pass)

- **chat_policy**: tool_call_timeout_secs (default 30s), tool_call_chain_limit (100), retry_without_tools_on_404, timeout_strategy (fixed/backoff/strict, base 30s), error_retry_limit (2), length_retry_limit (1), length_continue_prompt (string, static-leaked once).
- **rag**: top_k (15), strategy (dense | sparse {strict} | hybrid {rrf, mmr}), bm25_timeout_ms (250), bm25_retry_backoff_ms ([50,100]), strict_bm25_by_default, rrf (k, weight_bm25/dense), mmr (lambda, sim_metric, candidate_pool).
- **embedding_local**: device_preference, cuda_device_index, allow_fallback, approximate_gelu, use_pth, model_batch_size (>=1), max_length.
- **execution defaults**: token_limit (8196), tool_retries (2), llm_timeout_secs (45), cargo_check/test timeouts unchanged.
- All fields are serde-defaulted and validated/clamped on load; invalid user inputs fall back to defaults without panicking.
- Runtime → persisted round-trip via `RuntimeConfig::to_user_config`; live changes can be saved back to TOML without losing new fields.
```
