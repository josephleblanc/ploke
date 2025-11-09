# Model Browser Loading Overlay Investigation — 2025-11-08

## Summary
- `/model search <keyword>` opens the Model Browser overlay but the UI never receives model rows, so it remains stuck on “Loading models…” indefinitely.
- No prior report in `docs/reports/` or `crates/ploke-tui/docs/reports/` covers this failure, so this document records the root cause and next actions.

## Observed Symptom
- Triggering `/model search gpt` (or any keyword) shows the overlay with the correct title, but the body never populates with results or providers.
- The loading banner never clears, preventing model selection or provider lookup.

## Reproduction
1. Launch the TUI with a valid `OPENROUTER_API_KEY`.
2. Enter `/model search gpt`.
3. Observe that the overlay stays on “Loading models…” forever.

## Findings
- `Command::ModelSearch` opens the overlay with an empty `Vec` and immediately spawns `open_model_search` (`crates/ploke-tui/src/app/commands/exec.rs:50-54`). The view renders the loading indicator whenever `items.is_empty()` (`crates/ploke-tui/src/app/view/components/model_browser.rs:121-129`).
- `open_model_search` fetches models from OpenRouter, filters them into `filtered`, but drops the data without sending any event or state update. The only code that would push results into the UI is commented out, so the overlay is never hydrated (`crates/ploke-tui/src/app/commands/exec.rs:570-621`).
- Even if we re-enabled the commented event emission, `handle_llm_models_response` is an empty stub, so responses are ignored (`crates/ploke-tui/src/app/events.rs:233-236`). No other code path updates `App.model_browser.items` after the initial empty vector.

## Root Cause
The async fetch pipeline for `/model search` is incomplete: results are retrieved and filtered but never emitted to the event bus, and the UI handler that should consume `models::Event::Response` is unimplemented. Because the overlay’s state is never populated, it perpetually displays the loading message.

## Fix Direction
1. Re-introduce an event/command that hands the filtered `Vec<ResponseItem>` to the UI (e.g., emit `AppEvent::Llm(LlmEvent::Models(...))` or a dedicated `SystemEvent`). Include the keyword or reuse the stored `ModelBrowserState.keyword` when filtering.
2. Implement `handle_llm_models_response` to check `app.model_browser`, filter against the active keyword, populate `items`, and trigger a redraw. Guard against stale responses (keyword mismatch) before mutating state.
3. Add regression coverage: a unit test for `open_model_search`’s event emission and a UI snapshot asserting that model rows replace the loading banner when results arrive.

## Evidence
- `crates/ploke-tui/src/app/commands/exec.rs:50-54,570-621`
- `crates/ploke-tui/src/app/view/components/model_browser.rs:121-129`
- `crates/ploke-tui/src/app/events.rs:233-236`

## References
- `crates/ploke-tui/docs/plans/agentic-system-plan/comprehensive-e2e-testing-plan.md` (current OpenRouter API + tool calling focus)
