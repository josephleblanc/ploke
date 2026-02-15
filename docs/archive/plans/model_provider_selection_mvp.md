# Model + Provider Selection MVP – Plan, Files Needed, and Process

Objective
- Deliver a fast, reliable Model Browser that lets the user:
  1) Search models and view providers per model.
  2) See provider-level “tools” capability.
  3) Select exactly one provider for the model.
  4) Persist selection in RuntimeConfig and route requests accordingly.

Scope Freeze
- Only the selection and display aspects; no streaming/usage accounting changes in this iteration.
- Keep the TUI thread non-blocking at all times.

Dataflow (async, non-blocking)
- User: `model search <keyword>`
- UI thread:
  - Opens overlay immediately with keyword and no items.
- Background task:
  - Fetch `/models/user` (filtered by keyword).
  - Emit UI event with results to populate the overlay.
  - Pre-cache per-model `.../endpoints` for currently visible items; emit incremental updates.
- UI:
  - Renders provider rows; indicates `supports_tools` per provider.
  - On confirm, emits `StateCommand::SelectModelProvider { model_id, provider_id }`.
- State/Config:
  - Persist {model_id, provider_id} in RuntimeConfig so subsequent requests use them.

Files to add (please add these so we can wire the async path cleanly)
- crates/ploke-tui/src/app/events.rs
  - Define an AppEvent (or SystemEvent) variant to deliver model search results:
    - e.g., AppEvent::ModelSearchResults { keyword: String, items: Vec<ModelEntry> }
  - Handle toggling overlay open/close and incremental updates.
- crates/ploke-tui/src/llm/mod.rs (or llm/client.rs)
  - Request builder for chat/completions:
    - Accept selected provider; include provider routing preference in request body.
    - Ensure tools[] are passed when caller intends tool use.
- crates/ploke-tui/src/app/view/overlays/model_browser.rs (or similar)
  - Extract overlay widget logic from app/mod.rs to a dedicated component with:
    - Items list, provider expansion, selection handling, help toggle.
    - A method to update items on AppEvent.

Already available and used
- crates/ploke-tui/src/app/commands/exec.rs
  - Will spawn async fetch and emit AppEvent with results; opens overlay immediately in UI.
- crates/ploke-tui/src/app/commands/parser.rs
  - Parses `model search` and `model search` (empty) → help.
- crates/ploke-tui/src/app_state/commands.rs
  - We will add `SelectModelProvider` command to persist selection in RuntimeConfig.
- crates/ploke-tui/src/app_state/dispatcher.rs
  - Will handle `SelectModelProvider` by updating RuntimeConfig.

Next code steps (after files are added)
1) Exec: Replace block_in_place in `model search` with a spawned task:
   - On success: emit AppEvent::ModelSearchResults with filtered items.
   - On error: emit a SysInfo message.
2) Events: Handle the new AppEvent by calling `app.open_model_browser(keyword, items)`.
3) Provider endpoints: Background task pre-fetches `/models/:author/:slug/endpoints` for visible items and emits AppEvent updates.
4) Selection: Implement StateCommand::SelectModelProvider and update RuntimeConfig.
5) Request builder: Include provider routing preference and tools[].

How we avoid failure states
- Ask for missing files before changing behavior (no guessing event/command schemas).
- Keep diffs small and reversible; document intent and expected outcomes with each change.
- Maintain TUI responsiveness: no blocking calls on the UI thread.
- Prefer explicit “unknown/unsupported” UI states over assumptions.
- Validate with snapshot tests where possible (fixed terminal size).

Acceptance Criteria
- Searching is instant (overlay opens immediately).
- Results populate within a moment without blocking input.
- Provider rows show accurate tools capability.
- Selecting a provider updates RuntimeConfig and is used by subsequent requests.
