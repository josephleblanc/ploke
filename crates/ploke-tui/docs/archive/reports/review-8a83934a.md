Review: 8a83934a — propagate llm2 changes to state dispatcher

Scope
- Files:
  - crates/ploke-tui/src/app_state/dispatcher.rs
  - crates/ploke-tui/src/app_state/events.rs
- Intent: Move state/dispatcher wiring to strongly typed llm2 identifiers and preferences.

Build & Lints
- cargo check (-p ploke-tui, features: test_harness,llm_refactor): 36 errors, 19 warnings (expected mid-refactor).
  - Breakages cluster around: string→typed ModelId migration, endpoint field renames, provider selection types, and deprecated AppEvent variants.
- rustfmt (file-only): both touched files have small ordering/whitespace diffs.
  - dispatcher.rs: reorder `use` lines, unify crate import group.
  - events.rs: expand long use into block.
- clippy: skipped (crate fails to compile).

Code Review Notes

dispatcher.rs
- Good: `StateCommand::SelectModelProvider` now uses `ModelId` and `ProviderKey` and constructs `EndpointKey { model, variant, provider }`.
- Good: Avoids stringly-typed plumbing; sets `cfg.active_model = model_id.clone()`.
- Suggestion: `use crate::llm2::ProviderSlug;` appears unused and can be removed.
- Suggestion: When cloning `model_id.key`, consider `let key = model_id.key.clone();` and reuse (minor).
- Suggestion: Message text uses `provider.slug.as_str()` which is clear. Keep consistent units/labels elsewhere (pricing displays).
- Concurrency: The write lock is held only for needed mutations; message emission happens after. Looks fine.

events.rs
- Type migration started: `SystemEvent::ModelSwitched(ModelId)` introduced. Downstream still assumes `String`.
  - Places to update (from errors):
    - `app_state/models.rs`: send `mid` (ModelId) not `mid.to_string()`.
    - `app/events.rs` SystemEvent handler: fields `active_model_id` and `active_model_indicator` expect `String`; consider migrating to `ModelId`.
- Endpoints mapping in `ModelsEndpointsResults` handler uses old names and types:
  - `p.tool_use` → `p.supports_tools()`.
  - `p.ep_name` → choose one: `p.provider_name.as_str()` (preferred for provider display) or `p.name` ("Provider | author/slug").
  - `p.ep_context_length` → `p.context_length` (f64; convert: `as u32`/`min(u32::MAX as f64)` as needed).
  - `p.ep_pricing_prompt` → `p.pricing.prompt`.
  - `p.ep_pricing_completion` → `p.pricing.completion`.
  - `ModelProviderRow` now has `key: ProviderKey`; fill with the endpoint’s provider: `key: ProviderKey { slug: p.tag.provider_name }` or via a helper.
- Selection logic:
  - Instead of `provider_choice = .map(|p| p.name.clone())`, prefer `.map(|p| p.key.clone())` so `apply_model_provider_selection(model_id, Some(provider_key))` compiles.

model_browser.rs (indirectly surfaced)
- Rendering error: `p.name.to_string()` fails because `ProviderNameStr` doesn’t implement Display. Use `p.name.as_str()` or implement `impl Display for ProviderNameStr`.
- Units: When printing per‑1M token pricing, label explicitly, e.g., `pricing (USD/1M tok)`.
- Doc comment: Complete `/// input cost converted from USD/token to...`.

Suggested Changes (preview; not applied)
- events.rs:
  - Map endpoints with new fields:
    - `supports_tools: p.supports_tools()`
    - `name: p.provider_name` (or use `ArcStr::from(p.provider_name.as_str())` if needed)
    - `context_length: p.context_length as u32`
    - `input_cost: p.pricing.prompt`
    - `output_cost: p.pricing.completion`
    - `key: ProviderKey { slug: p.tag.provider_name.clone() }` (or via `ProviderKey::new(..)` if available)
  - Selection: choose `.map(|p| p.key.clone())` and call `app.apply_model_provider_selection(model_id, Some(provider_key))`.
- dispatcher.rs:
  - Remove unused `ProviderSlug` import.
- model_browser.rs:
  - Replace `p.name.to_string()` with `p.name.as_str()`; add unit label.

Follow-ups
- Migrate `App` fields and UI to use `ModelId` consistently:
  - `App.active_model_id: String` → `ModelId` (and any associated display to use `to_string()` or `Display`).
  - `active_model_indicator: (String, Instant)` → `(ModelId, Instant)`.
- Consider `Display` for `ProviderNameStr` for easier formatting.
- Consider newtypes for pricing units (`UsdPerToken`, `UsdPerMillionTokens`) to enforce scale correctness.

Evidence
- Check run: cargo check (-p ploke-tui, features: test_harness,llm_refactor) → 36 errors, 19 warnings.
- Formatting check: rustfmt showed small diffs on both files.

Readiness
- Not established (expected mid-refactor). I can re-run once downstream type changes land.
