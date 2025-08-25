# Summary of Changes — Model/Provider Selection (Phase 1)

Scope
- Implement user-facing commands to manage models/providers:
  - model list, model info, model use <alias|id>, model refresh [--local], model load [<path>], model save [<path>] [--with-keys]
  - provider strictness <openrouter-only|allow-custom|allow-any>
- Persist config with atomic writes and default key redaction.
- Refresh OpenRouter capabilities and cache in registry.

Key Changes
- Command parsing and execution:
  - crates/ploke-tui/src/app/commands/parser.rs — Structured parsing of new subcommands with style-aware normalization.
  - crates/ploke-tui/src/app/commands/exec.rs — Async handlers for save/load/refresh/strictness; non-blocking UI via StateCommand.
- Registry and config:
  - crates/ploke-tui/src/user_config.rs — ProviderRegistry with strictness, alias resolution, capability cache, API key resolution, atomic save/load with redaction.
  - crates/ploke-tui/src/llm/registry.rs — Curated defaults for common OpenRouter models.
  - crates/ploke-tui/src/llm/openrouter_catalog.rs — Minimal client to fetch /models capability/pricing.
- Startup:
  - crates/ploke-tui/src/lib.rs — Merges defaults, refreshes capabilities, resolves API keys.

Behavioral Notes
- By default, saving config redacts API keys; users can opt-in to include keys with --with-keys.
- /model refresh re-resolves API keys and (optionally) rebuilds capability cache from OpenRouter.
- Strictness prevents switching to disallowed providers; attempts are logged and ignored.

Tests
- Config save/load redaction round-trips.
- Provider strictness enforcement.
- Command parsing variants for model/provider operations.

Breaking Changes
- None for persisted config (new fields have sensible defaults). Existing users inherit curated defaults without overriding their entries.

Known Gaps
- No end-to-end tests for model switching path; relies on existing StateCommand::SwitchModel behavior.
- Capability cache is opportunistic; we do not block on OpenRouter failures.
