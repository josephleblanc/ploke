# User Configuration Loading Process - Detailed Analysis

*Generated: 2025-09-06*

This document provides a comprehensive analysis of how user configuration is loaded, processed, and applied during Ploke TUI startup.

## 1. Configuration Architecture Overview

**Primary Location**: `crates/ploke-tui/src/user_config.rs`

### Key Data Structures:
- `UserConfig`: Top-level configuration container
- `ModelRegistry`: Provider management and model selection
- `EmbeddingConfig`: Embedding backend configuration
- `EditingConfig`: Editor behavior and agent settings

## 2. Configuration Loading Sequence

### Phase 1: Configuration Sources Assembly
**Location**: `crates/ploke-tui/src/lib.rs:135-148`

```rust
let mut config = config::Config::builder()
    .add_source(
        config::File::with_name(
            &dirs::config_dir()
                .unwrap()
                .join("ploke/config.toml")
                .to_string_lossy(),
        )
        .required(false),  // Optional - won't fail if missing
    )
    .add_source(config::Environment::default().separator("_"))
    .build()?
    .try_deserialize::<UserConfig>()
    .unwrap_or_else(|_| UserConfig::default());
```

#### Source Priority (highest to lowest):
1. **Environment Variables**: `PLOKE_*` with `_` separators
2. **TOML Config File**: `~/.config/ploke/config.toml`
3. **Hardcoded Defaults**: `UserConfig::default()`

### Phase 2: Configuration Merging & Validation
**Location**: `crates/ploke-tui/src/lib.rs:150-161`

1. **Registry Defaults Merging**: `config.registry = config.registry.with_defaults()`
2. **OpenRouter Capabilities Refresh**: Updates model capabilities cache
3. **API Key Loading**: Resolves keys from environment variables

## 3. Configuration Structure Details

### 3.1 UserConfig Fields

```rust
pub struct UserConfig {
    pub registry: ModelRegistry,           // Provider and model management
    pub command_style: CommandStyle,       // NeoVim vs Slash commands  
    pub embedding: EmbeddingConfig,        // Embedding backend config
    pub editing: EditingConfig,            // Edit behavior settings
    pub ploke_editor: Option<String>,      // External editor command
}
```

### 3.2 ModelRegistry Configuration

**Purpose**: Manages LLM providers, model selection, and API routing

#### Core Fields:
- `providers: Vec<ModelConfig>`: List of configured LLM endpoints
- `active_model_config: String`: Currently selected provider ID
- `aliases: HashMap<String, String>`: Model aliases for easier switching
- `capabilities: HashMap<String, ModelCapabilities>`: Cached model info (skip in serialization)
- `strictness: ModelRegistryStrictness`: Provider selection policy
- `require_tool_support: bool`: Filter models by tool calling capability

#### ModelConfig Structure:
```rust
pub struct ModelConfig {
    pub id: String,                        // Unique provider identifier
    pub api_key: String,                   // API key (resolved at runtime)
    pub provider_slug: Option<ProviderSlug>, // OpenRouter routing preference
    pub api_key_env: Option<String>,       // Environment variable name for key
    pub base_url: String,                  // API endpoint URL
    pub model: String,                     // Model identifier (e.g., "openai/gpt-4o")
    pub display_name: Option<String>,      // UI display name
    pub provider_type: ProviderType,       // OpenRouter/OpenAI/Anthropic/Custom
    pub llm_params: Option<LLMParameters>, // Temperature, top-p, etc.
}
```

### 3.3 Embedding Configuration

**Purpose**: Configures vector embedding backend for RAG search

#### Backend Options (exactly one should be set):
```rust
pub struct EmbeddingConfig {
    pub local: Option<LocalModelConfig>,        // Local sentence-transformers
    pub hugging_face: Option<HuggingFaceConfig>, // HF Inference API
    pub openai: Option<OpenAIConfig>,           // OpenAI embeddings
    pub cozo: Option<CozoConfig>,               // CozoDB embeddings
}
```

#### Default Behavior:
If no backend specified, defaults to local sentence-transformers model:
- Model: "sentence-transformers/all-MiniLM-L6-v2"
- Device: Auto-detection (GPU preferred)
- Batch size: 8

### 3.4 EditingConfig

**Purpose**: Controls editing behavior and agent automation

```rust
pub struct EditingConfig {
    pub auto_confirm_edits: bool,          // Skip manual approval for edits
    pub agent: EditingAgentConfig,         // Agent-specific settings
}

pub struct EditingAgentConfig {
    pub enabled: bool,                     // Enable agent-driven editing
    pub min_confidence: f32,               // Minimum confidence for auto-edits (default: 0.8)
}
```

## 4. Configuration Resolution Process

### 4.1 Environment Variable Mapping

Environment variables use `PLOKE_` prefix with `_` for nesting:

**Examples**:
- `PLOKE_COMMAND_STYLE=NeoVim` → `UserConfig.command_style`
- `PLOKE_REGISTRY__ACTIVE_MODEL_CONFIG=gpt4` → `UserConfig.registry.active_model_config`
- `PLOKE_EMBEDDING__LOCAL__MODEL_ID=custom-model` → `UserConfig.embedding.local.model_id`

### 4.2 API Key Resolution Strategy

**Location**: `ModelConfig::resolve_api_key()` - `crates/ploke-tui/src/user_config.rs:261-296`

#### Resolution Order (first found wins):
1. **Provider-specific env var**: If `api_key_env` field set, use that variable
2. **Provider-type env vars**:
   - OpenRouter: `OPENROUTER_API_KEY`
   - OpenAI: `OPENAI_API_KEY`
   - Anthropic: `ANTHROPIC_API_KEY`
   - Custom: `LLM_API_KEY`
3. **Explicit config value**: Falls back to configured `api_key` field

### 4.3 Default Model Registry Population

**Location**: `ModelRegistry::with_defaults()` - `crates/ploke-tui/src/user_config.rs:333-340`

Process:
1. **User providers preserved**: Any user-defined provider with same `id` replaces default
2. **Missing defaults added**: Curated defaults from `crate::llm::registry::DEFAULT_MODELS`
3. **No credential leakage**: Missing fields in user config don't inherit from defaults

## 5. Configuration Validation & Error Handling

### 5.1 Parsing Resilience
- **TOML parsing errors**: Falls back to `UserConfig::default()`
- **Missing config file**: Continues with defaults (file marked as `required(false)`)
- **Environment variable type errors**: Individual fields fall back to defaults

### 5.2 Runtime Validation
- **API key validation**: Not validated at startup, resolved at request time
- **Model existence**: Capabilities refreshed from OpenRouter if possible
- **Embedding backend**: Validated during `load_embedding_processor()`

## 6. Configuration Persistence

### 6.1 Save Operations
**Location**: `UserConfig::save_to_path()` - `crates/ploke-tui/src/user_config.rs:108-136`

**Features**:
- **Atomic writes**: Uses temporary file + rename for consistency
- **Secret redaction**: Optional `redact_keys` parameter clears API keys before save
- **Pretty TOML**: Human-readable output format

### 6.2 Load Operations  
**Location**: `UserConfig::load_from_path()` - `crates/ploke-tui/src/user_config.rs:139-143`

**Default path**: `~/.config/ploke/config.toml`

## 7. OpenRouter Integration

### 7.1 Capabilities Refresh
**Location**: `ModelRegistry::refresh_from_openrouter()` - `crates/ploke-tui/src/user_config.rs:463-519`

**Process**:
1. **API key resolution**: Uses any OpenRouter provider or `OPENROUTER_API_KEY`
2. **Model catalog fetch**: Queries OpenRouter `/models` endpoint
3. **Capabilities parsing**: Extracts tool support, context length, pricing
4. **Cache update**: Populates `capabilities` HashMap

**Tool Support Detection**:
```rust
let supports_tools = m.supported_parameters
    .as_ref().is_some_and(|p| p.supports_tools());
```

### 7.2 Fallback Tool Support
When OpenRouter refresh fails, hardcoded fallbacks for known tool-capable models:
- `moonshotai/kimi-k2`, `openai/gpt-4o`, `anthropic/claude-3-5-sonnet`, etc.

## 8. Command-Time Configuration Usage

### 8.1 Model Switching
**Location**: `ModelRegistry::set_active()` - `crates/ploke-tui/src/user_config.rs:388-431`

**Validation**:
1. **Alias resolution**: Check aliases HashMap first
2. **Provider existence**: Verify provider exists in registry
3. **Strictness enforcement**: Check against `ModelRegistryStrictness` policy
4. **Tool requirements**: Optional filtering by tool support capability

### 8.2 Strictness Policies
```rust
pub enum ModelRegistryStrictness {
    OpenRouterOnly,    // Only OpenRouter providers allowed
    AllowCustom,       // OpenRouter + Custom (default)  
    AllowAny,          // No restrictions
}
```

## 9. Configuration State After Loading

### System Defaults Applied:
- **Active model**: "kimi-k2" (moonshotai/kimi-k2)
- **Command style**: Slash (default)
- **Embedding**: Local sentence-transformers
- **Editing**: Manual confirmation required
- **Registry**: Populated with curated defaults + user overrides

### Runtime State:
- **API keys resolved**: All providers have keys from environment
- **Capabilities cached**: OpenRouter model metadata loaded (if API call succeeded)
- **Registry validated**: Active model exists and passes strictness check

### Memory Layout:
- **Configuration immutable**: Config converted to `RuntimeConfig`, original not retained
- **Registry mutable**: Can be updated by commands during runtime
- **Capabilities cache**: Skipped in serialization, rebuilt on each startup

This detailed analysis shows how Ploke balances flexible configuration with secure defaults and runtime adaptability.