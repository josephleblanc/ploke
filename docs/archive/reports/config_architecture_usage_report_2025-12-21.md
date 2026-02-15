# Config Architecture Usage Report (2025-12-21)

This report summarizes where the key configuration data structures live, how they are wired at
runtime, and where to tweak defaults and guardrails.

## User-facing configuration outline

This outline is intended for a future user guide. It describes each configuration area, what it
does, and the defaults as implemented in code.

### Core UI and editing

- `command_style`: UI command mode.
  - Values: `NeoVim` | `Slash`
  - Default: `Slash`
- `tool_verbosity`: how much tool output is shown to the user.
  - Values: `minimal` | `normal` | `verbose`
  - Default: `normal`
- `ploke_editor`: override editor command for file editing or inspection.
  - Default: unset
- `editing.auto_confirm_edits`: auto-apply edits without approval.
  - Default: `false`
- `editing.agent.enabled`: allow edit agent suggestions.
  - Default: `false`
- `editing.agent.min_confidence`: minimum confidence required for agent suggestions.
  - Default: `0.8`

### Chat loop and retries

- `chat_policy.tool_call_timeout_secs`: tool call deadline per step.
  - Default: `30`
  - Guardrails: 5..600
- `chat_policy.tool_call_chain_limit`: max tool calls per session.
  - Default: `100`
  - Guardrails: 1..500
- `chat_policy.retry_without_tools_on_404`: retry a tool call when providers return 404 for tool
  support.
  - Default: `false`
- `chat_policy.timeout_strategy`: retry strategy for request timeouts.
  - Values:
    - `FixedRetry { attempts = N }`
    - `Backoff { attempts = N | null }`
    - `Strict`
  - Default: `FixedRetry { attempts = 3 }`
  - Guardrails: attempts <= 10
- `chat_policy.timeout_base_secs`: timeout duration used by the timeout strategy.
  - Default: `30`
  - Guardrails: 5..600
- `chat_policy.error_retry_limit`: number of retries for provider errors.
  - Default: `2`
  - Guardrails: <= 10
- `chat_policy.length_retry_limit`: number of retries for length stops.
  - Default: `1`
  - Guardrails: <= 5
- `chat_policy.length_continue_prompt`: system prompt appended on length retries.
  - Default: `"Continue from where you left off. Do not repeat prior text."`
- `llm_timeout_secs`: HTTP timeout for chat requests.
  - Default: `45`

### Token budgets

- `token_limit`: default token limit for tool-initiated LLM requests.
  - Default: `8196`
- `tool_retries`: tool retry count for model misfires.
  - Default: `2`

### Model registry and routing

These settings control model profiles and OpenRouter routing preferences.

- `registry.global_default_profile`: default LLM parameters for unqualified models.
- `registry.models`: per-model preferences and profiles.
- `registry.strictness`: provider restriction policy (OpenRouter-only vs custom).
- `registry.router_prefs`: routing preferences for providers (OpenRouter).

### Embeddings

- `embedding`: provider selection (exactly one of `local`, `hugging_face`, `openai`, `cozo`).
  - Default: local MiniLM (`sentence-transformers/all-MiniLM-L6-v2`)

Local runtime tuning (advanced):
- `embedding_local.device_preference`: `Auto` | `ForceCpu` | `ForceGpu`
  - Default: `Auto`
- `embedding_local.cuda_device_index`
  - Default: `0`
- `embedding_local.allow_fallback`
  - Default: `true`
- `embedding_local.approximate_gelu`
  - Default: `false`
- `embedding_local.use_pth`
  - Default: `false`
- `embedding_local.model_batch_size`
  - Default: `8`
  - Guardrails: >= 1
- `embedding_local.max_length`
  - Default: unset

### RAG and retrieval

- `rag.top_k`: number of snippets retrieved per query.
  - Default: `15`
  - Guardrails: 1..200
- `rag.strategy`: retrieval mode.
  - Values: `Dense` | `Sparse { strict = bool }` | `Hybrid { rrf, mmr }`
  - Default: `Hybrid` with defaults below
- `rag.bm25_timeout_ms`
  - Default: `250`
  - Guardrails: 10..5000
- `rag.bm25_retry_backoff_ms`
  - Default: `[50, 100]`
- `rag.strict_bm25_by_default`
  - Default: `false`
- `rag.rrf`: reciprocal rank fusion weights.
  - `k`: default `60.0` (guardrails 1..2000)
  - `weight_bm25`, `weight_dense`: default `1.0` (guardrails 0..10)
- `rag.mmr`: optional maximal marginal relevance config.
  - `lambda`: default `0.7` if set (guardrails 0..1)
  - `sim_metric`: `Cosine`
  - `candidate_pool`: default `50` (guardrails 1..500)

### Tooling and cargo integration

- `tooling.cargo_check_timeout_secs`
  - Default: `60` (0 disables timeout)
- `tooling.cargo_test_timeout_secs`
  - Default: `600` (0 disables timeout)

### Context management

- `context_management.strategy`
  - `Automatic(N)`: prunes context after N turns
  - `Ask(N)`: asks LLM to repin after N turns
  - `Unlimited`: manual only
  - Default: `Automatic(15)`

### Example TOML snippet

```toml
command_style = "Slash"
tool_verbosity = "normal"
token_limit = 8196
tool_retries = 2
llm_timeout_secs = 45

[chat_policy]
tool_call_timeout_secs = 30
tool_call_chain_limit = 100
timeout_base_secs = 30
timeout_strategy = { FixedRetry = { attempts = 3 } }
error_retry_limit = 2
length_retry_limit = 1
length_continue_prompt = "Continue from where you left off. Do not repeat prior text."

[rag]
top_k = 15
bm25_timeout_ms = 250
bm25_retry_backoff_ms = [50, 100]
strict_bm25_by_default = false
strategy = { Hybrid = { rrf = { k = 60.0, weight_bm25 = 1.0, weight_dense = 1.0 }, mmr = null } }

[embedding_local]
device_preference = "Auto"
model_batch_size = 8
```

## Core entry points

- `crates/ploke-tui/src/user_config.rs`
  - `UserConfig`: persisted TOML config root.
  - `ChatPolicy`, `ChatTimeoutStrategy`: tool loop and retry/timeout knobs.
  - `RagUserConfig`, `RetrievalStrategyUser`: retrieval/fusion knobs (RRF/MMR).
  - `EmbeddingConfig`: provider selection (local/HF/OpenAI/Cozo).
  - `LocalEmbeddingTuning`: local device/batch/length settings.
  - `ToolingConfig`: cargo tool timeouts.
  - Defaults + guardrails: `default_*` fns and `validated()` methods.
- `crates/ploke-tui/src/app_state/core.rs`
  - `RuntimeConfig`: in-memory config for live use.
  - `From<UserConfig>`: applies validation and maps persisted config to runtime.
  - `RuntimeConfig::to_user_config`: round-trip for persistence.

## Chat loop policies

- `crates/ploke-tui/src/llm/manager/session.rs`
  - `TuiToolPolicy`, `FinishPolicy`: runtime policies for tool loop and finish-reason handling.
  - `tool_policy_from_chat`, `finish_policy_from_chat`: converts `ChatPolicy` into runtime policy.
- `crates/ploke-tui/src/llm/manager/mod.rs`
  - Pulls `chat_policy` and `llm_timeout_secs` from `RuntimeConfig`.
  - Passes tool/finish policies + HTTP timeout to `run_chat_session`.

## RAG wiring

- `crates/ploke-tui/src/lib.rs`
  - `build_rag_config`: maps `RagUserConfig` to `ploke_rag::RagConfig`.
  - RAG service initialized with user-configurable defaults.
- `crates/ploke-tui/src/rag/context.rs`
  - `process_with_rag` uses `RuntimeConfig.rag` (`top_k`, `strategy`).
- `crates/ploke-tui/src/app/commands/exec.rs`
  - `open_context_search` uses runtime `rag` settings.
- `crates/ploke-tui/src/tools/request_code_context.rs`
  - Tool uses runtime token limit and RAG strategy for context requests.
- `crates/ploke-rag/src/core/mod.rs`
  - `RagConfig`: low-level RAG config used by `RagService`.
- `crates/ploke-rag/src/fusion/mod.rs`
  - `RrfConfig`, `MmrConfig`, `ScoreNorm`, `Similarity` (serde-enabled).

## Embedding config

- `crates/ploke-tui/src/user_config.rs`
  - `EmbeddingConfig` (provider selection) + `LocalEmbeddingTuning`.
  - `load_embedding_processor` uses the local tuning for local embeddings.
- `crates/ingest/ploke-embed/src/local/mod.rs`
  - `DevicePreference`: now serde-enabled so it can be persisted in user config.

## Token limits and timeouts

- `crates/ploke-tui/src/user_config.rs`
  - `token_limit`, `tool_retries`, `llm_timeout_secs` stored under `UserConfig`.
- `crates/ploke-tui/src/tools/request_code_context.rs`
  - Uses `RuntimeConfig.token_limit` for default tool budget.

## Documentation and tracking

- `docs/active/todo/2025-12-21-config-architecture.md`
  - Updated summary of current user-config surface for reference.

## Where to tweak

- **Defaults**: edit `default_*` functions in `crates/ploke-tui/src/user_config.rs`.
- **Guardrails**: edit `validated()` methods in `ChatPolicy` and `RagUserConfig`.
- **Runtime mapping**: edit `From<UserConfig> for RuntimeConfig` in `crates/ploke-tui/src/app_state/core.rs`.
- **RAG mapping**: edit `build_rag_config` in `crates/ploke-tui/src/lib.rs`.
