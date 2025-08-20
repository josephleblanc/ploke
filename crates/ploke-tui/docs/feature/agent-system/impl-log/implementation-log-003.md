# Implementation log 003 — Eventing SSoT and typed tool events (2025-08-19)

Summary
- Completed remaining items in “Eventing: SSoT for indexing, typed tool events”.
- Introduced typed ToolEvent with an AppEvent::LlmTool variant and wired a compatibility bridge.
- Added SSoT test to ensure EventBus is the single source of truth for IndexingStatus → AppEvent.

Changes
- crates/ploke-tui/src/lib.rs
  - Added AppEvent::LlmTool(llm::ToolEvent).
  - priority(): LlmTool::Requested → Background; LlmTool::{Completed,Failed} → Realtime.

- crates/ploke-tui/src/llm/mod.rs
  - Added public enum ToolEvent { Requested, Completed, Failed } carrying request_id/call_id and payloads.
  - llm_manager: Now handles AppEvent::LlmTool(ToolEvent::Requested) and dispatches to rag_handlers::handle_tool_call_requested.
  - Kept legacy bridges:
    - AppEvent::Llm(Event::ToolCall { .. }) still bridged to SystemEvent::ToolCallRequested.
    - AppEvent::System(SystemEvent::ToolCallRequested { .. }) still spawns the handler (compatibility).

- crates/ploke-tui/src/llm/tool_call.rs
  - dispatch_and_wait now emits AppEvent::LlmTool(ToolEvent::Requested) via EventBus::send (priority-aware).
  - Await path unchanged; session awaits completion/failure.

- crates/ploke-tui/src/llm/session.rs
  - await_tool_result now accepts both typed LlmTool::{Completed,Failed} and legacy SystemEvent::{ToolCallCompleted,ToolCallFailed}.

- crates/ploke-tui/src/app/events.rs
  - Added a no-op handler for AppEvent::LlmTool(_), keeping the UI loop lean.

- crates/ploke-tui/src/event_bus/mod.rs
  - Added test ssot_forwards_indexing_completed_once to assert exactly one AppEvent::IndexingCompleted is forwarded when IndexStatus::Completed is injected.

Notes
- SystemEvent::ToolCallRequested remains supported for M0; will be removed in M1.
- EventBus remains the sole translator for IndexingStatus → AppEvent (SSoT).
- No external API changes for consumers.

Next steps
- Extend telemetry spans on tool-call path with %request_id and %call_id.
- Add E2E tests that go through rag_handlers for tool calls and assert completions.
