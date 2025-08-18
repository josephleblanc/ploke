2025-08-17T00:00:00Z — Tool-call helpers and concurrent execution

Summary
- Implemented a dedicated llm/tool_call.rs module to standardize dispatch and awaiting of tool calls via the EventBus.
- Updated llm/mod.rs to execute multiple provider tool_calls concurrently and append tool messages in a stable order by call_id.
- Kept channel strategy on realtime to remain compatible with session::await_tool_result and existing handlers.

Files changed
- Added: crates/ploke-tui/src/llm/tool_call.rs
- Modified: crates/ploke-tui/src/llm/mod.rs (added mod tool_call; refactored tool_calls handling loop)
- Notes: No external dependencies added; used tokio::task::JoinSet to avoid new crates.

Reasoning
- Aligns with docs/tool-calls.md Decisions: parallel tool calls with deterministic ordering; subscribe-before-send to avoid missed events; keep realtime channel for now.
- Localizes tool-call plumbing to simplify future RequestSession extraction.

Next steps
- Extract the per-request loop into a RequestSession in llm/session.rs as described in docs/tool-calls.md.
- Consider moving SystemEvent::ToolCallRequested routing to background channel once session-side awaiting supports it.
- Add configurable history_char_budget and tighten token/char caps.
- Add tests: multi-call concurrency ordering; failure/timeout paths; error payload shapes.

Questions / inconsistencies
- Both AppEvent::Llm(Event::ToolCall) and direct SystemEvent::ToolCallRequested paths exist. Do we plan to deprecate the AppEvent::Llm(Event::ToolCall) route?
- docs mention RequestSession extraction (Milestone 2) — current change only prepares for it; full extraction pending.

Notable potential issues (tracked)
- Timeout is hardcoded to 30s in prepare_and_run_llm_call; consider making it configurable (LLMParameters?).
- If a tool task panics, JoinSet returns a join error; we currently log and omit that outcome, which may lead to a missing tool response. The model will proceed next cycle; consider surfacing a system error message proactively.
