2025-08-17T02:05:00Z — Tool-calls: concurrent execution wiring, deprecation stance clarified, and compile fixes

Summary
- Completed concurrent tool-call execution path:
  - RequestSession now executes multiple tool calls in parallel via llm/tool_call::execute_tool_calls, preserving stable ordering by call_id when appending results.
  - subscribe-before-send semantics are enforced inside tool_call::dispatch_and_wait using EventBus.realtime_tx.
- Kept compatibility path and clarified deprecation:
  - SystemEvent::ToolCallRequested remains functional but is explicitly marked DEPRECATED at call and handler sites; will be replaced by dedicated tool events in a future EventBus refactor.
- Compile fixes and hygiene:
  - Resolved missing imports and type paths in llm/mod.rs (AppEvent, EventBus, AppState, StateCommand).
  - Fixed prepare_and_run_llm_call signature to use Option<Event> instead of Option<llm::Event> (self-module path fix).
  - No API changes beyond internal visibility/import adjustments.

Files changed
- Modified: crates/ploke-tui/src/llm/mod.rs
  - Added imports for AppEvent, EventBus, AppState, and StateCommand.
  - Fixed prepare_and_run_llm_call context type to Option<Event>.

Reasoning
- Aligns with docs/tool-calls.md: execute tool calls concurrently, deterministic append by call_id, maintain realtime channel for now, and subscribe-before-send to avoid missing fast responses.
- Keeps legacy SystemEvent route alive but clearly deprecated, reducing risk during incremental refactor.

Deprecation note (decision)
- We continue to deprecate the SystemEvent::ToolCallRequested path. It remains enabled for compatibility but will be replaced by dedicated EventBus tool events. The current implementation logs runtime warnings at both the emission site and the system handler (rag::handle_tool_call_requested).

Next steps
- Milestone 2 (finish): Move all message assembly responsibilities fully into RequestSession::new to narrow prepare_and_run_llm_call or remove it.
- Milestone 3: Introduce a tool classification and route long-running tools over the background channel; RequestSession will subscribe appropriately while keeping subscribe-before-send.
- Tests: add integration tests for multi-call ordering, per-call timeouts driven by LLMParameters.tool_timeout_secs, and error payload shapes.

Notable potential issues (tracked)
- If a spawned tool task panics, JoinSet yields a join error and we skip that outcome; consider surfacing a system-visible error (SysInfo) in the session loop for better UX.
- Realtime channel may become busy during larger tool bursts; will revisit when background routing lands.

Rolling edit-log window
- This file supersedes older entries beyond the last two. Retain edit-log-02.md and edit-log-03.md; archive or delete edit-log-01.md to maintain the rolling window of two logs.
