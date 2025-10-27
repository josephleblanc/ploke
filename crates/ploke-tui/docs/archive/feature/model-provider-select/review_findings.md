# Technical Review — Findings and Improvements

Flaws / Code Smells
- Command duplication: Legacy string-matched commands and new structured parser overlap (model list, model use). Consolidate once migration completes.
- Capability cache coupling: `refresh_from_openrouter` lives in `ProviderRegistry`; consider extracting to a service to isolate network from config.
- Strictness feedback: Enforcement only logs and returns false; surface explicit user-facing messages in executor on failure to switch.
- Env resolution order: `resolve_api_key` checks multiple env vars; document precedence and consider caching resolution results.
- Config mutation: `load_api_keys` mutates all providers in-place. Consider immutable pattern returning a new registry to simplify reasoning in tests.

Gaps / Improvements
- Alias management UX: No commands to add/remove aliases at runtime. Add `/provider alias add <alias> <id>` and `/provider alias rm <alias>`.
- Capability-aware defaults: When a chosen model lacks tool support, inform users proactively in `model info` or upon switch.
- Persistence integration: Provide a single command `/config save` consolidating save paths across subsystems.

Testing Debt
- End-to-end tests verifying `StateCommand::SwitchModel` triggers `SystemEvent::ModelSwitched`.
- Wiremock-based tests for OpenRouter catalog parsing (resiliency to optional fields).

Documentation
- Add a developer guide on config layering (defaults + toml + env) and how to safely extend the registry.

Action Items
- Remove legacy handlers after full parser migration.
- Introduce a registry service trait (Network + Cache) with in-memory impl for tests.
- Expand curated defaults and tag each with expected capabilities.
