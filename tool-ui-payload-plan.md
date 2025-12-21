# Plan

Implement structured tool UI payloads end-to-end so tool calls can render with tool name/fields and adjustable verbosity, while keeping LLM-facing tool results unchanged and aligned with the new ToolError taxonomy.

## Requirements
- Keep LLM tool results as structured JSON for model/tooling.
- Add a structured Rust payload for UI rendering with tool name, key fields, and details.
- Support verbosity levels (minimal/normal/verbose) in the UI renderer.
- Ensure tool UI/diagnostics align with the new ToolError type and categories.
- Write TDD tests that fail until the feature is implemented.

## Scope
- In: tool execution output, tool event payloads, message storage/rendering, UI formatting, tests.
- Out: provider/tool schema changes, UI redesign beyond tool message rendering.

## Files and entry points
- crates/ploke-tui/src/tools/mod.rs (Tool trait extension + processing)
- crates/ploke-tui/src/tools/* (per-tool UI payload mapping)
- crates/ploke-tui/src/llm/manager/session.rs (tool-call flow to UI)
- crates/ploke-tui/src/app_state/commands.rs (message command payload)
- crates/ploke-tui/src/app_state/handlers/chat.rs (tool message insertion)
- crates/ploke-tui/src/chat_history.rs (message storage)
- crates/ploke-tui/src/app/message_item.rs (rendering)
- crates/ploke-tui/tests/... (new tests)

## Data model / API changes
- Add ToolUiPayload struct and ToolVerbosity enum.
- Add MessageContent::Tool(ToolUiPayload) or add a tool_payload: Option<ToolUiPayload> to Message.
- Extend Tool trait with to_ui_payload(...) and surface through ToolCallCompleted (or a parallel event).
- Ensure ToolError mapping is available for UI payload construction (summary, fields, severity).

## Action items
[x] Decide storage model for UI payload (message enum vs optional field) and verbosity source.
[x] Add ToolUiPayload + ToolVerbosity types and update tool trait to emit UI payloads.
[x] Update tool execution to produce UI payloads and pass them through event/command to UI.
[x] Add renderer for tool payloads (minimal/normal/verbose) in message_item.rs.
[x] Write TDD tests that assert:
     - tool UI payload is created from a tool call
     - renderer respects verbosity level
     - tool name/fields appear in UI output
     - ToolError alignment (kind/summary fields appear in UI output)
[x] Implement functionality to satisfy tests; iterate until tests pass.
[ ] Add verbosity control (config/command toggle) if desired.
[ ] Run the full `ploke-tui` test suite if needed.

## Testing and validation
- Add unit tests in crates/ploke-tui/tests/... for:
  - tool UI payload mapping from tool result
  - rendering output for minimal/normal/verbose
- Run cargo test -p ploke-tui.

## Risks and edge cases
- Tool result formats differ across tools; mapping must be per-tool.
- Avoid breaking LLM tool results; ensure UI payload is separate.
- Verbosity toggles must not affect message ordering or selection.
- ToolError mapping must not leak sensitive data to the UI.

## Open questions
- Final decision on payload storage in Message and verbosity control location.
