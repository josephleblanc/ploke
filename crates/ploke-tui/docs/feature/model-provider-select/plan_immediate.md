# Immediate Plan: Model and Provider Selection (Phase 1)

Goal
- Provide user-facing commands to:
  - List available models (aliases and full IDs)
  - Switch the active model/provider
  - Refresh model/provider capabilities from OpenRouter
  - Load/save provider registry settings from/to disk (default path or user-specified path)
  - Configure provider “strictness” policy for allowed providers

Non-goals
- New UI windows/panels
- Streaming tool-call UX improvements
- Persisting secrets by default

Scope Overview
- Extend the command parser with a minimal, ergonomic set of subcommands in “Slash” or “NeoVim” mode.
- Wire commands to the ProviderRegistry (read/write) and broadcast status updates via EventBus.
- Persist registry changes to config file using atomic file writes.

User Commands (syntax)
- /model list
  - Show (id, display_name) for configured providers and a mark for the active one.
  - Include count of OpenRouter-discovered models cached in registry.capabilities (if any).
- /model use <alias|id>
  - Change active provider and broadcast SystemEvent::ModelSwitched(new_model).
- /model refresh [--remote]
  - Reload registry.capabilities from OpenRouter using OPENROUTER_API_KEY if available.
  - Re-resolve API keys from env (registry.load_api_keys()).
- /model load [<path>]
  - Load Config from the default path: ~/.config/ploke/config.toml
  - If <path> is provided, load from relative or absolute path.
  - Merge with curated defaults via .with_defaults()
  - Refresh OpenRouter capabilities if OPENROUTER_API_KEY is present.
- /model save [<path>] [--with-keys]
  - Save current Config (including provider registry) to default path or to <path>.
  - By default, redact API keys on save unless --with-keys is specified.
- /provider strictness <openrouter-only|allow-custom|allow-any>
  - Update a new Config/Registry field that governs which providers are allowed at runtime.
  - openrouter-only: only providers with ProviderType::OpenRouter are selectable.
  - allow-custom: OpenRouter + any custom providers configured by user.
  - allow-any: no restrictions (future-friendly).

Implementation Plan

1. Commands parsing
- Location: crates/ploke-tui/src/app/commands.rs
- Add new subcommands under the existing command system:
  - model list
  - model use <alias|id>
  - model refresh [--remote]
  - model load [<path>]
  - model save [<path>] [--with-keys]
  - provider strictness <mode>
- Reuse existing App::send_cmd and StateCommand to keep the UI thread non-blocking.
- Where possible, respond to the user with AddMessageImmediate for success/fail feedback.

2. State management and config updates
- Add helper methods in user_config.rs:
  - impl Config {
      pub fn save_to_path(&self, path: &std::path::Path, redact_keys: bool) -> color_eyre::Result<()>;
      pub fn load_from_path(path: &std::path::Path) -> color_eyre::Result<Config>;
    }
  - redact_keys will write api_key="" unless explicitly saving with --with-keys.
- Create a small utility for default config path resolution:
  - ~/.config/ploke/config.toml (dirs::config_dir + “ploke/config.toml”)
- Ensure we call registry.load_api_keys() on:
  - startup (after merging defaults)
  - /model refresh
  - /model load
- Broadcast SystemEvent::ModelSwitched when switching providers (already consumed in UI).
- Provide user feedback messages for each command outcome.

3. Startup integration
- In try_main (crates/ploke-tui/src/lib.rs):
  - We already:
    - Read config from default path if present
    - Merge curated defaults: config.registry = config.registry.with_defaults()
    - Refresh from OpenRouter: config.registry.refresh_from_openrouter().await
  - Ensure we also call registry.load_api_keys() (currently commented) after with_defaults().
  - Keep logs at debug level to avoid excessive noise.

4. Persistence details
- Use toml::to_string_pretty for serialization.
- Perform atomic write via tempfile + std::fs::rename or use ploke-io if we want the same code path as edits:
  - Preferred: ploke_io::IoManagerHandle::write_file_atomic for consistency and durability.
- Never log API keys. Redact before serialization unless user explicitly provides --with-keys.

5. Provider strictness policy
- Add ProviderRegistryStrictness enum:
  - OpenRouterOnly
  - AllowCustom
  - AllowAny
- Add field in ProviderRegistry:
  - strictness: ProviderRegistryStrictness (Default: AllowCustom)
- Enforce on model use:
  - If strictness == OpenRouterOnly, reject switching to non-OpenRouter providers with a friendly message.

6. Tests
- Unit tests:
  - registry.set_active (exists) + strictness enforcement
  - config load/save round-trip (with redaction default)
  - command parsing to Verify actions are dispatched
- Integration tests (optional):
  - /model list -> prints providers
  - /model use <alias> -> switches and emits SystemEvent::ModelSwitched
  - /model load/save round-trip with and without --with-keys
- Doc tests for helper functions in user_config.

Acceptance Criteria
- Users can list, select, refresh, load, and save model/provider settings via commands.
- Active provider switch is reflected in UI (indicator at top) and persists when saved.
- Defaults are preserved and merged, capabilities refreshed when possible.
- No secrets are persisted by default; user must opt-in to save keys.

Risks and Mitigations
- Failure to hit OpenRouter /models endpoint:
  - Return a warning; do not block other commands.
- Partial or invalid user config files:
  - Fallback to Default::default() + curated defaults + informative message.
- Concurrency:
  - Use existing StateCommand + EventBus to avoid blocking the UI thread.

Implementation Checklist
- [ ] Add command parsing for /model and /provider
- [ ] Add Config::save_to_path / load_from_path with redaction
- [ ] Add ProviderRegistryStrictness and enforcement in set_active path
- [ ] Wire /model refresh to registry.refresh_from_openrouter() + registry.load_api_keys()
- [ ] Ensure try_main calls load_api_keys() after merging defaults
- [ ] Tests for redaction and command behaviors
