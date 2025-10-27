Review: 59c5ea13 — try to fix errors, run in circles

Scope
- Files touched in this commit include event routing, dispatcher, commands, model browser, endpoint types, router providers, and llm2 manager events. This introduced both progress and inconsistencies across type usage.

Actions I Performed
- Fixed compile blockers and type mismatches across UI and events; gated live-only modules; removed stray file.
- Brought ploke-tui back to a successful `cargo check` under features `test_harness,llm_refactor` (warnings remain; see below).

Changes Applied (targeted)
- Remove stray file `'` created by accident at repo root.
- Gate live modules: `app/commands/mod.rs` now wraps `exec_live_tests` and `exec_real_tools_live_tests` behind `cfg(feature = "live_api_tests")`.
- Provider selection (CLI): `provider select <model_id> <provider_slug>`
  - Build `ProviderKey` via `ProviderKey::new(&provider_slug)` and send `Option<ProviderKey>` with `model_id_string` to `StateCommand::SelectModelProvider`.
- Model search filtering (typed ids):
  - Search uses `m.id.to_string().to_lowercase()` and `m.name.as_str().to_lowercase()`.
  - Sorting uses `a.id.to_string().cmp(&b.id.to_string())`.
- llm2 events routing (UI):
  - Route `AppEvent::Llm2` to handlers; implemented `handle_llm_endpoints_response` to update model browser provider rows.
  - Fixed warning/error emitters to use `app.send_cmd(...)` (non-async path).
- Endpoint/provider types:
  - `Endpoint.provider_name` now uses `openrouter::providers::ProviderName` (enum with Display) instead of a string newtype.
  - `ModelBrowserRow` provider fields updated accordingly; new constructor `from_id_endpoint(model_id, &ProviderKey, Endpoint)` derives `ProviderKey` from `Endpoint.tag.provider_name`.
- Model browser presentation:
  - Provider display uses enum Display; pricing label clarified to `pricing (USD/1M tok)` and removed double 1e6 scaling.
  - Completed doc comment for top-level item pricing units.
- Test harness cleanup: removed calls to now-missing `RegistryPrefs::with_defaults()` and `load_api_keys()`.

Build & Evidence
- cargo check: success (0 errors) with features `test_harness,llm_refactor`.
- Warnings: deprecation warnings for legacy AppEvent variants and many `private_interfaces` visibility warnings remain (non-blocking for now).

Notable Remaining Warnings / Follow-ups
- Visibility mismatches: UI structs (`ModelBrowserItem`, `ModelProviderRow`) expose `pub` fields of `pub(crate)` types (e.g., `ModelId`, `ProviderKey`). Options:
  - Make UI fields `pub(crate)`; or
  - Expose wrapper/newtype re-exports with `pub` in llm2; or
  - Implement accessor methods instead of public fields.
- Deprecated AppEvent variants are still present in `lib.rs` and `app/events.rs` (legacy path). Plan to migrate callers to `AppEvent::Llm2(LlmEvent::...)` and remove deprecated variants when practical.
- Minor: remove unused `mut` variables flagged by warnings.
- Consistency: `apply_model_provider_selection` now takes a `String` (`model_id_string` at the StateCommand). This matches the dispatcher parsing path; confirm this is the intended API or migrate end-to-end to typed `ModelId` if/when the UI can carry the typed id through.

Suggested Next Steps
- Pick a visibility strategy to eliminate `private_interfaces` warnings for llm2 types used in app modules (prefer keeping llm2 types `pub(crate)` and changing UI fields to `pub(crate)` too).
- Replace deprecated AppEvent variants with llm2 events throughout (`lib.rs` routing table, event producers/consumers), then delete deprecated variants.
- Consider implementing `Display` for typed newtypes where frequent UI formatting occurs (e.g., `ModelName`) to trim `.as_str()` plumbing.
- Optionally, unify pricing unit newtypes (e.g., `UsdPerToken`, `UsdPerMillionTokens`) to encode scale and avoid double-scaling bugs.

Summary
- ploke-tui compiles again with the typed llm2 adjustments in the UI/event path. The changes keep scope tight: they fix the immediate inconsistencies (types, provider mapping, display) and defer broader cleanups (visibility, deprecation) to a follow-up.
