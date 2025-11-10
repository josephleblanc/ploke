# Model Browser Loading Overlay Fix Report — 2025-11-09

## Summary
- Confirmed the previously logged root cause (missing event routing plus an empty handler) still explains the stuck overlay described in `model_browser_loading_overlay_investigation_2025-11-08.md`.
- Implemented the fix described there: model search now emits a payload tagged with the initiating keyword, and the UI handler hydrates the overlay state once results arrive.
- Added a focused unit test for the keyword filter so we have executable evidence tied to the ongoing plan in `docs/plans/agentic-system-plan/comprehensive-e2e-testing-plan.md`.

## Root Cause Confirmation
- The `/model search` command still opened the overlay with an empty `items` vector and spawned `open_model_search`.
- `open_model_search` emitted an event containing the full API response, but `handle_llm_models_response` remained a stub, so the overlay never received data. This matches the “async fetch pipeline is incomplete” diagnosis.
- Therefore the overlay being stuck on “Loading models…” is a direct consequence of the unimplemented handler, not a new regression.

## Implementation Notes
1. **Typed event metadata** — Extended `models::Event::Response` with an optional `search_keyword: ArcStr`. Every search response now includes the initiating query so the UI can drop stale payloads if the user submits another search before the previous fetch returns. Other model refresh paths populate `None`.
2. **UI hydration path** — Replaced the stub `handle_llm_models_response` with logic that:
   - captures the current overlay keyword immutably, rejects stale payloads (keyword mismatch), and filters the API payload via `filter_models_for_keyword`;
   - repopulates `ModelBrowserState.items` through a new `App::build_model_browser_items` helper so both the initial overlay open and async refresh share the same mapping code;
   - clamps selection + scroll indices and posts a user-visible warning if no models match the query.
3. **Safety for concurrent searches** — If the overlay keyword changes between snapshot and mutation, or if the overlay closes, the handler now bails out quietly to avoid corrupting UI state.
4. **Targeted test coverage** — Added `filter_models_for_keyword` unit tests (case-insensitive matching + empty keyword behavior) as a first regression guard while we work toward the broader e2e plan referenced above.

## Validation
- `cargo test filter_models_for_keyword --package ploke-tui` (pass; see console log for existing unrelated warnings about unreachable patterns).

## Follow-ups
- The rendering path still treats “no matches” the same as “still loading” because the overlay displays a loading line whenever `items.is_empty()`. Consider adding a dedicated empty-state copy once the gating plan in `comprehensive-e2e-testing-plan.md` reaches the UI snapshot workstream.
- Extend coverage toward the report’s longer-term ask (provider list hydration + snapshot tests) after the current OpenRouter API/tool-calling milestones.
