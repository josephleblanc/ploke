# Implementation log 002 — M0 initial hardening code changes (2025-08-19)

Summary
- Enforced Indexing SSoT: Only the EventBus forwards IndexingStatus → AppEvent.
- Atomic chat history save path implemented in FileManager.
- Event lag surface: Indexing lag now emits a UI-visible warning event.

Changes
- crates/ploke-tui/src/app_state/handlers/indexing.rs
  - Removed direct emissions of AppEvent::IndexingCompleted/Failed from the indexer task handler.
  - Rationale: run_event_bus() is the single source of truth for translating IndexingStatus to AppEvents.

- crates/ploke-tui/src/event_bus/mod.rs
  - On RecvError::Lagged for indexing channel, now logs a warning and emits AppEvent::Error with Warning severity to the realtime channel.
  - Maintains existing forwarding: Running → IndexingProgress, Completed → IndexingCompleted, Cancelled → IndexingFailed.

- crates/ploke-tui/src/file_man.rs
  - save_content() now performs atomic write: write to .ploke_history.md.tmp, fsync, then rename to .ploke_history.md.
  - Logs the final file path on success; on failures, emits AppEvent::Error so the UI shows a SysInfo message.
  - Replaced an expect() on realtime send with a fallible send + warning.

Notes
- Typed LLM tool events are kept as a design target; SystemEvent::ToolCallRequested bridge is retained for M0 compatibility.
- No external API surface changed for consumers; focus remains on observability and correctness.

Next steps
- Add an EventBus SSoT test (spawn run_event_bus, inject IndexingStatus::Completed, assert a single AppEvent::IndexingCompleted).
- Wire correlation fields (request_id/call_id) across the tool-call path with spans.
