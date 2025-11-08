# Diagnosis: `/model search <keyword>` shows “0 results” and stays on “Loading models…”

## Summary
- Symptom: The Model Browser overlay opens, the header shows “0 results for '<keyword>'”, and the body only shows “Loading models…”, never populating results.
- Root cause: The async model search fetch does not publish results back to the UI, and the UI’s event handler for model search responses is effectively a no‑op.
- Impact: All `/model search <keyword>` invocations appear empty regardless of the keyword, even when the OpenRouter API would return matches.

## Reproduction
1. In the TUI, run `/model search kimi` (or any keyword).
2. Observe the overlay header reads `Model Browser — 0 results for "kimi"` and the body shows “Loading models…”. It never updates.

## Affected code paths
- Command parsing maps `/model search <kw>` correctly to `Command::ModelSearch(kw)`:
  - `crates/ploke-tui/src/app/commands/parser.rs` → `Command::ModelSearch(String)`
- Command execution opens an empty overlay then tries to fetch results:
  - `crates/ploke-tui/src/app/commands/exec.rs`:
    - `execute(..)` → `Command::ModelSearch(keyword)`:
      - `app.open_model_browser(keyword.clone(), Vec::new());`
      - `open_model_search(app, &keyword);`
- UI overlay renders “Loading models…” when `mb.items.is_empty()`:
  - `crates/ploke-tui/src/app/view/components/model_browser.rs` → `render_model_browser(..)`
  - Header uses `mb.items.len()` for the results count (so it stays at 0 until items are set).
- The fetch function does not publish results to the UI:
  - `crates/ploke-tui/src/app/commands/exec.rs` → `open_model_search(..)`
    - Fetches models (via `OpenRouter::fetch_models`) and filters them locally.
    - Missing: updating `app.model_browser.items` or emitting an event/state change carrying the filtered list.
    - A commented line hints at the intended event flow:
      ```rust
      // emit_app_event(AppEvent::Llm(llm::LlmEvent::Models(models::Event::Response { ... })))
      ```
- Even if a models response arrived, the UI handler is empty:
  - `crates/ploke-tui/src/app/events.rs` → `handle_llm_models_response(..)` currently does nothing.

## Root cause
Two breaks in the dataflow prevent results from populating the overlay:
1) `open_model_search(..)` never publishes the fetched/filtered results back to the UI thread:
   - It performs an async fetch and computes `filtered: Vec<ResponseItem>` but never updates `App.model_browser.items` and does not emit a models response event.
2) The UI’s models response handler (`handle_llm_models_response`) is a no‑op:
   - Even if a `LlmEvent::Models(Event::Response { .. })` were emitted (either by `open_model_search` or by the LLM manager in response to a `Models::Request`), the handler would not translate that into `ModelBrowserItem`s for the overlay.

Together, the overlay is opened with an empty `items` vector and remains empty indefinitely, keeping the “Loading models…” placeholder and “0 results” header.

## Potential fixes

### Fix A (Preferred): Use the event-driven pipeline end-to-end
Align `/model search` with the existing LLM manager/event bus design and strong typing.

1) Replace the direct HTTP in `open_model_search(..)` with an event emission:
   - Emit `LlmEvent::Models(models::Event::Request { router: RouterVariants::OpenRouter(OpenRouter) })` using `emit_app_event(..)`.
   - Keep opening the overlay immediately (existing behavior) so the UI doesn’t feel laggy.

2) Implement `handle_llm_models_response(..)` in `crates/ploke-tui/src/app/events.rs`:
   - On `models::Event::Response { models: Some(response) }`:
     - Read the current `app.model_browser` (bail if overlay was closed).
     - Filter `response.data` against `mb.keyword` (case-insensitive match on both `id` and `name`).
     - Map the filtered `ResponseItem`s into `ModelBrowserItem`s (pricing scaled to per‑1M tokens as already done in `App::open_model_browser`).
     - Set `mb.items = mapped_items; mb.selected = 0;` and trigger a redraw.
   - On `models::Event::Response { models: None }`, consider showing a `SysInfo` message like “Failed to load models” (the manager already emits None on fetch error) and leave the overlay open (empty) or close it—UX choice.

Pros:
- Single source of truth for fetching (LLM manager).
- Strongly typed event flow consistent with the rest of the system.
- Easier to test by injecting `Models::Response` events under the `test_harness` feature.

### Fix B (Minimal invasive change): Keep fetch in `open_model_search`, publish results
If you prefer not to refactor now:
1) After computing `filtered: Vec<ResponseItem>` in `open_model_search(..)`, emit a `LlmEvent::Models(models::Event::Response { models: Some(Arc<Response { data: filtered }>) })` via `emit_app_event(..)`.
2) Implement the same `handle_llm_models_response(..)` logic as in Fix A to populate `mb.items`.

Pros:
- Smaller change set (no change to manager flow).
Cons:
- Duplicates HTTP logic that already exists in the manager’s `Models::Request` path.

### Fix C (UI-command path): Introduce a dedicated UI event/state command
Add a specific `UiEvent::ModelSearchResults { items: Vec<ResponseItem> }` (or a `StateCommand`) and handle it in the UI thread to populate `mb.items`. This avoids reusing the LLM models event, but adds a new UI event variant.

Pros:
- Very explicit UI-only path.
Cons:
- Introduces another event channel for essentially the same data already typed in the LLM events.

## Edge cases and user feedback
- Missing `OPENROUTER_API_KEY`: Keep the existing preflight message in `open_model_search(..)` or let the manager return `None` and surface a helpful SysInfo message. Either way, ensure the overlay doesn’t hang without feedback.
- Empty results: If no matches after filtering, show a short inline hint in the overlay footer/help, e.g., “No matches. Try a broader keyword.”

## Testing recommendations
- Unit-test `handle_llm_models_response(..)`:
  - Given a synthetic `Models::Response` (with a few entries), assert that filtering by `mb.keyword` produces the expected `mb.items` and the overlay header shows the correct count.
- Integration test (behind `test_harness`):
  - Simulate `Models::Response` arrival after opening model browser; ensure the overlay content updates from “Loading models…” to a populated list.
- Optional live-gated test (requires `OPENROUTER_API_KEY`): verify we get non‑zero results for common keywords; record pass/fail counts under `target/test-output/...` per live gate discipline.

## Implementation notes
- When mapping `ResponseItem` to `ModelBrowserItem`, reuse the pricing and `supports_tools` logic currently in `App::open_model_browser` for consistency:
  - pricing per 1M tokens: `prompt*1_000_000.0`, `completion*1_000_000.0`
  - `context_length`: prefer model-level `context_length` and fall back to `top_provider.context_length`
  - `supports_tools`: `ResponseItem::supports_tools()` is already implemented
- Make sure to set `app.needs_redraw = true` after updating `mb.items`.

## Conclusion
The overlay never updates because results are fetched but not published, and the handler that should receive them is empty. Implementing an event-driven update (preferred) or emitting a response from the current fetch and handling it will fix the issue and bring the results into the Model Browser overlay as intended.


