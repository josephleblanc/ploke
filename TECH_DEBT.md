# Technical Debt and Future Improvements

## LLM Outcome Summaries UI
- For now, per-request outcomes are surfaced via SysInfo messages.
- Future improvement: add a subtle, always-visible context window indicator with color and cost hints.

Ideas for minimal context fill display:
- Thin line at the far right edge of the screen that grows with context usage; color transitions: blue → green → yellow → red; shows approximate cost next to it.
- Small progress bar above the input area, displaying percentage of context used and a compact token count.
- Tiny inline meter in the statusline (bottom) with a color dot and a short string like `ctx 62% · ~$0.03`.
- Discrete corner indicator (top-right) that expands on hover/focus to reveal more detail.

Ways to get more details at different levels of granularity:
- Key hint (e.g., `Ctrl-i`) toggles an expanded readout: breakdown of user/assistant/context tokens, estimated cost, and tool payload sizes.
- Command `:ctx detail` opens a modal with current/average tokens per turn, recent high-water marks, and model pricing.
- Drilldown overlay listing recent requests with their token/latency/cost stats, sortable and filterable.

Notes:
- Implement token accounting centrally (e.g., at request assembly in LLM module) to feed both SysInfo and any UI indicators.
- Color semantics should be accessible and theme-aware; consider patterns for low-contrast terminals.

## Tool Dispatcher Error Handling
- Add dispatcher-level tests that drive `tools::process_tool` with malformed ApplyCodeEdit payloads (missing `edits`, wrong schema, etc.) and assert that the full event pipeline (ToolCallRequested → ToolCallFailed) fires as expected. This covers suggestion 3 once we have time for more comprehensive testing.
- Propagate `Tool::deserialize_params` failures through `SystemEvent::ToolCallFailed` (and add structured tracing) instead of only returning `ToolError::DeserializationError`, so malformed payloads become observable; this tracks suggestion 4.
