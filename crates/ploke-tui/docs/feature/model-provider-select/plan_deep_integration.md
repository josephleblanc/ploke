# Deep Integration Plan: GUI Model/Provider Selector (Phase 2)

Goal
- Provide a rich TUI window for browsing, filtering, and selecting models/providers.
- Support fuzzy-search, side-panel filters, and inline editing of model parameters.
- Allow saving customized configurations as aliases for future reuse.

High-Level UX
- Entry point:
  - Command: /model ui or a keybinding (e.g., g m) to open the Model Browser.
- Layout (ratatui):
  - Top: Search bar (live-updating fuzzy search)
  - Left: Scrollable model list (OpenRouter catalog + configured aliases)
  - Right: Side panel with filters and selected model details
  - Bottom: Action bar (hints: Enter=Select, a=Save as alias, e=Edit params, Esc=Close)
- Item expand/collapse:
  - Selecting a model expands details: display_name, full id, context_length, tools support, pricing, provider_type, configured params (temperature, top_p, etc).
- Actions:
  - Set Active
  - Save as alias (prompt for alias, optional copy of params)
  - Edit parameters (temperature, max_tokens, top_p, response_format, etc.)
  - Open in browser (optional, e.g., model docs if URL available in future)
  - Filter toggles on side panel

Data Sources
- ProviderRegistry.providers: user-defined providers and curated defaults.
- ProviderRegistry.capabilities: fetched from OpenRouter (id -> capabilities/pricing/tools).
- OpenRouter catalog refresh:
  - Run in background task; update the list when complete.
  - Show spinner or “Refreshing…” indicator with timestamp.

Filtering & Search
- Filters (right panel):
  - Provider type: OpenRouter | OpenAI | Anthropic | Custom
  - Tools support: Yes/No
  - Context length: min/max slider (discrete bins)
  - Pricing tier: input/output cost ranges
  - Strictness policy: OpenRouterOnly / AllowCustom / AllowAny
- Fuzzy search:
  - Live filtering of the left list based on search input.
  - Potential dependency: fuzzy-matcher (skim or clangd-like scoring), or simple substring match at first.
  - Keep dependencies minimal; design to degrade gracefully if fuzzy matcher is not available.

Editing and Persistence
- Inline parameter editor:
  - Numeric fields: temperature, top_p, max_tokens
  - Enums: response_format
  - Toggles: parallel_tool_calls
  - Tool retry/timeouts: tool_max_retries, tool_timeout_secs, history_char_budget
- Save as alias:
  - Prompt user for alias id (validate uniqueness).
  - Write updated ProviderConfig into registry.providers with the alias id and display_name.
  - Persist via the same config save pipeline as in Phase 1.
  - Redact API keys on save by default.

Architecture & Modules
- New module: app/view/components/model_browser.rs
  - State:
    - search_query: String
    - filters: struct with booleans and ranges
    - selected_index: usize
    - expanded: Option<usize>
    - list_items cache: Vec<ModelListItem> combining providers + OpenRouter capabilities
    - fetch_status: Idle | Loading | Error(String) | Loaded(Instant)
  - Methods:
    - prepare(&ProviderRegistry, &CapabilitiesCache)
    - apply_filters_and_search()
    - render(frame, area)
    - handle_key(Action) / handle_mouse(MouseEvent)
- EventBus integration:
  - Listen for refresh completion event
  - Broadcast changes: ModelSwitched, RegistryChanged (optional)
- StateCommand additions:
  - SaveAlias { id, ProviderConfig }
  - SetStrictness { policy }
  - UpdateProviderParams { id, LLMParametersPatch }
- LLMParametersPatch:
  - Partial update to parameters; only set fields provided by the editor.

Keybindings (tentative)
- Esc: Close model browser
- Enter: Set Active on selected item
- e: Edit parameters (overlay modal)
- a: Save as alias (prompt)
- /: Focus search bar
- Tab/Shift-Tab: Move focus between list and side panel
- Arrow keys/PageUp/PageDown/Home/End: Navigate list
- f: Toggle filter sidebar or focus the filter controls
- r: Refresh models (OpenRouter fetch)
- s: Save registry to disk (redacted by default)

Visual Details
- List entries show:
  - [alias] display_name (id) — badges: [tools], [ctx:128k], [in:$3/M], [out:$15/M], [OpenRouter]
  - Highlight active model with a special color/marker
- Expanded entry (selected):
  - Detailed info and actions (set active, edit, save alias)
- Side panel filters:
  - Checkbox toggles (e.g., supports_tools)
  - Dropdown-like selection (provider type) using keys
  - Numeric ranges for context length/costs (coarse bins)

Concurrency and Performance
- Initial load: show configured providers instantly (aliases + curated defaults)
- Spawn background task to fetch OpenRouter capabilities; update the UI list when done
- Cache results in ProviderRegistry.capabilities for fast future loads
- Debounce search input to avoid thrashing on every keystroke

Error Handling
- If refresh fails, display a non-blocking banner in the browser
- Do not prevent selection or editing of local providers on network failure
- Never display or log API keys

Testing Strategy
- Unit tests:
  - Filtering and fuzzy-search ranking with sample data
  - Parameter patch application and alias save logic
- UI snapshot tests (optional):
  - Use ratatui test utilities to assert basic layout invariants
- Integration tests:
  - Simulate refresh events and verify the list updates and remains navigable
  - Persist alias and confirm it appears after reload

Phased Delivery
- 2.1: Basic browser (list + search + set active)
- 2.2: Filters panel + details expansion
- 2.3: Edit parameters modal + save alias
- 2.4: Background refresh + incremental loading indicators

Dependencies (optional)
- fuzzy-matcher = "0.3"
- Consider keeping Phase 2.1 implementation without new deps by using substring matching first.

Acceptance Criteria
- Users can open the model browser, search/filter models, and set active model without leaving the main UI.
- Users can edit parameters and save aliases, and these persist via the config pipeline.
- Browser reflects OpenRouter capability refreshes when available.
