# Tool UI Payload Overview

This document captures the current state of work on structured tool UI payloads with verbosity and ToolError alignment, plus remaining steps to finish.

## Goal
- Keep LLM tool results as JSON for model/tooling.
- Add structured Rust payloads for UI rendering (tool name/fields/details, verbosity).
- Align tool UI error presentation with ToolError taxonomy.

## Work Completed

### Plan + Docs
- Added local plan file (per user request) at `tool-ui-payload-plan.md`.
- New overview doc: this file.

### New Types
- Added `crates/ploke-tui/src/tools/ui.rs`:
  - `ToolVerbosity` enum (minimal/normal/verbose).
  - `ToolUiField`, `ToolUiPayload` (tool name, call_id, summary, fields, details, verbosity, error fields).
  - `ToolUiPayload::render(verbosity)` for UI text rendering.
  - `ToolUiPayload::from_error` creates a payload from `ToolError`, and adds `code` field (snake-case label) via `error_code_label`.

### Tool Result / Error Plumbing
- `ToolResult` extended to include `ui_payload: Option<ToolUiPayload>` in `crates/ploke-tui/src/tools/mod.rs`.
- `Tool::emit_completed` now accepts `ui_payload` and includes it in `SystemEvent::ToolCallCompleted`.
- `Tool::emit_err` now accepts a `ToolError` (not string), emits `ToolCallFailed` with `ui_payload` created from the error (ToolErrorWire + error code).
- Updated `SystemEvent::ToolCallCompleted`/`ToolCallFailed` to carry `ui_payload: Option<ToolUiPayload>` in `crates/ploke-tui/src/app_state/events.rs`.
- Updated observability handlers to ignore the new field.
- Legacy `ToolCallParams` error helpers now wrap messages into `ToolError` with `ui_payload` in `crates/ploke-tui/src/rag/utils.rs`.
- `CreateFileCtx` error helpers now wrap messages into `ToolError` with `ui_payload` in `crates/ploke-tui/src/tools/create_file.rs`.

### Chat History / Commands / Rendering
- `Message` now has `tool_payload: Option<ToolUiPayload>` in `crates/ploke-tui/src/chat_history.rs`.
- `add_child`/`add_message_tool` updated to accept `tool_payload`, and call sites updated.
- `StateCommand::AddMessageTool` carries `tool_payload`.
- `add_tool_msg_immediate` passes `tool_payload` into chat history.
- `RenderableMessage` and `RenderMsg` trait updated to expose `tool_payload`.
- Renderer now uses `ToolUiPayload::render()`:
  - `crates/ploke-tui/src/app/message_item.rs`: renders tool messages via `tool_payload` if present, otherwise falls back to `content`. This affects both measurement and drawing.
  - `ConversationView::prepare` and `render` now accept `tool_verbosity`.
  - `App` stores `tool_verbosity` (default `Normal`) and passes to `ConversationView`.

### LLM Loop Integration
- `execute_tools_via_event_bus` now returns `ToolCallUiResult`/`ToolCallUiError` structs, carrying `content` and `ui_payload`.
- In `run_chat_session`, `StateCommand::AddMessageTool` uses the structured `tool_payload` (if any).
- For tool failures, UI message is now the JSON tool result content, not a raw formatted error string; tool payload handles user-facing summary.

### Tool Implementations Updated (Success UI Payloads)
- `RequestCodeContextGat`: summary + fields (`search_term`, `top_k`, `returned`).
- `GatCodeEdit`: summary + fields (`staged`, `applied`, `files`, `preview_mode`).
- `CreateFile`: summary + fields (`staged`, `applied`, `files`, `preview_mode`).
- `NsRead`: summary + fields (`exists`, `truncated`, `lines`).
- `CodeItemLookup`: summary + fields (`file_path`, `canon_path`).
- `GetCodeEdges`: summary + fields (`file_path`, `canon_path`, `edges`).
- `apply_code_edit_tool`: now emits UI payload with staged/applied/file counts and preview mode.
- `apply_*_edit` (auto-confirm path): now emits UI payloads for completion/failure with tool name inferred from `is_semantic`.
- `create_file_tool`: now emits UI payload with staged/applied/file counts and preview mode.

### Event Producers Updated
- Direct `ToolCallCompleted/Failed` emissions now include structured payloads for:
  - apply code edits (staging + auto-confirm)
  - create-file staging + approvals/denials
  - `crates/ploke-tui/src/tools/create_file.rs` (legacy path)
  - `crates/ploke-tui/src/test_utils/new_test_harness.rs`

### Tests (TDD)
- New tests in `crates/ploke-tui/tests/tool_ui_payload.rs`:
  - `render_tool_payload_respects_verbosity` (minimal/normal/verbose rendering).
  - `render_error_payload_includes_error_code` (ToolError -> payload includes missing_field).
- Tests now pass.

## Current State of the Feature
- Tool UI payloads flow from tool execution to event bus to chat history to renderer.
- Renderer uses `ToolUiPayload::render(verbosity)` to display tool messages with headings/fields/details.
- Tool errors generate `ToolUiPayload` via `ToolError` and show code/field details.
- LLM tool results remain unchanged (JSON string in `ToolResult.content`).

## Remaining Work / Next Steps

1) UI verbosity control
- Currently `App` has a fixed `tool_verbosity: Normal`.
- Decide a config/command toggle (global setting, per-session, or per-message). Implement UI input handling if desired.

2) Add unit tests for payload construction per tool
- Optional: tests that `ToolResult.ui_payload` is populated for `RequestCodeContext`, `ApplyCodeEdit`, etc.

3) Run full test suite
- `cargo test -p ploke-tui` and address any remaining warnings if desired.

## Commands Used
- `cargo test -p ploke-tui --test tool_ui_payload`

## Files Touched (Key)
- `crates/ploke-tui/src/tools/ui.rs` (new)
- `crates/ploke-tui/src/tools/mod.rs`
- `crates/ploke-tui/src/tools/request_code_context.rs`
- `crates/ploke-tui/src/tools/code_edit.rs`
- `crates/ploke-tui/src/tools/create_file.rs`
- `crates/ploke-tui/src/tools/ns_read.rs`
- `crates/ploke-tui/src/tools/code_item_lookup.rs`
- `crates/ploke-tui/src/tools/get_code_edges.rs`
- `crates/ploke-tui/src/app_state/events.rs`
- `crates/ploke-tui/src/app_state/commands.rs`
- `crates/ploke-tui/src/app_state/dispatcher.rs`
- `crates/ploke-tui/src/app_state/handlers/chat.rs`
- `crates/ploke-tui/src/chat_history.rs`
- `crates/ploke-tui/src/app/types.rs`
- `crates/ploke-tui/src/app/message_item.rs`
- `crates/ploke-tui/src/app/view/components/conversation.rs`
- `crates/ploke-tui/src/app/mod.rs`
- `crates/ploke-tui/src/llm/manager/session.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-tui/src/rag/utils.rs`
- `crates/ploke-tui/src/test_utils/new_test_harness.rs`
- `crates/ploke-tui/tests/tool_ui_payload.rs` (new)

## Notes
- ToolError alignment: `ToolUiPayload::from_error` includes ToolErrorWire and error code field in rendered output.
- Rendering is plain text; styling uses existing MessageKind coloring. Further styling (headers/indent) could be added later.
