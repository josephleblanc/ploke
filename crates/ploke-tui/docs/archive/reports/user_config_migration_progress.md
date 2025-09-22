User Config Migration to llm2 Types — Progress Log

Context
- Goal: Replace the old, stringly-typed user config/registry with a new version that relies on llm2 types and a trait-based router approach.
- Scope: Migrate src/user_config.rs first; keep changes elsewhere minimal for now. Integration tests are out of scope at this stage.

Design Notes
- Keep `UserConfig` as the top-level persisted config entry point (TOML/env). 
- Replace the ad-hoc `ModelRegistry` with the llm2 concepts:
  - User prefs: `llm2::registry::user_prefs::RegistryPrefs` (profiles, strictness, router prefs)
  - Provider/router definitions: `llm2::types::newtypes::ProviderConfig` and `Transport`
  - Strongly typed identifiers: `ModelKey`, `ModelId`, `ProviderSlug`, `BaseUrl`, etc.
- Retain embedding/editing config unchanged to avoid scope creep.
- Keep compatibility helpers where cheap (e.g., re-export `default_model()`), but avoid re-defining a new `ModelRegistry` in llm2.

Planned Phases
1) Replace `src/user_config.rs` with a version that:
   - Uses `RegistryPrefs` for the registry field
   - Exposes `providers` as `Vec<ProviderConfig>` if/when needed (TBD by call-sites)
   - Keeps save/load and embedding loader
   - Re-exports `default_model()` and exposes `OPENROUTER_URL` via llm2
2) Adjust call sites progressively (runtime config mapping, dispatcher, old `llm` pathway) to the new shapes (may require shims or targeted refactors).
3) Expand router support via traits (beyond OpenRouter) using the existing llm2 router architecture.

Open Questions
- Back-compat shim: Is it acceptable to provide a temporary shim that exposes old names (`ModelRegistry`, `ModelConfig`) mapped to llm2 types, to reduce immediate breakage? Or should we fully replace and fix call sites now?
- Active selection: Old `ModelRegistry` had `active_model_config`. With `RegistryPrefs`, do we treat a Profile selection (or `selected_endpoints`) as the active selection? If so, what should the minimal “active” contract be at runtime?
- API key resolution: llm2’s `Router::resolve_api_key()` centralizes key lookup. Confirm we should remove/redact all app-level key storage and rely entirely on env-based lookup per router.

Status (initial)
- New `src/user_config.rs` drafted to leverage llm2 types and remove redundant ad-hoc registry.
- Some call sites will need updates (expected in mid-migration). This is captured and will be addressed after clarifications.

Next Steps

Decisions (from maintainer)
- Update direct call-sites now (no temporary shim).
- Keep active selection semantics: one active `ModelId`/endpoint; alternates in `selected_endpoints` (OpenRouter will use fallbacks).
- Provider storage: rely on router APIs + prefs for now.
- Secrets: rely entirely on env vars for API keys; no persistence.
- Confirm back-compat shim strategy vs. direct refactor of call sites.
- Update runtime config mapping to read from `RegistryPrefs` instead of `ModelRegistry`.
- Migrate minimal usages in `app_state/dispatcher.rs`, `observability.rs`, and `llm2/manager` to avoid blocking builds.
