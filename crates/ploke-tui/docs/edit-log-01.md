2025-08-17T00:30:00Z — RequestSession extraction (Milestone 2, partial) and deprecation notice

Summary
- Implemented RequestSession in llm/session.rs to own the per-request loop, including:
  - OpenAI request construction, provider error handling, and response parsing.
  - Parallel tool_calls via llm/tool_call::execute_tool_calls with stable ordering by call_id.
  - Retry cycle for tool calls based on LLMParameters.tool_max_retries.
  - Prompt window char-cap applied before each round via cap_messages_by_chars.
- Refactored llm/mod.rs:
  - prepare_and_run_llm_call now delegates the loop to RequestSession::run().
  - Marked SystemEvent::ToolCallRequested routing as DEPRECATED with runtime warnings (kept for compatibility).
  - Exposed selected OpenAI response types and cap_messages_by_chars to the session module with pub(super).

Files changed
- Modified: crates/ploke-tui/src/llm/mod.rs
  - Added deprecation warnings for SystemEvent::ToolCallRequested path.
  - Exposed internal response parsing types and cap_messages_by_chars to llm::session.
  - Refactored prepare_and_run_llm_call to use RequestSession.
- Modified: crates/ploke-tui/src/llm/session.rs
  - Added RequestSession struct and run() implementation; reuses await_tool_result helper and tool_call helpers.
- Modified: crates/ploke-tui/src/app_state/handlers/rag.rs
  - Added deprecation warning at entry for SystemEvent::ToolCallRequested handler (compatibility path).

Reasoning
- Aligns with docs/tool-calls.md Milestone 2: extract the per-request loop to RequestSession and keep current realtime tool event routing.
- Localizes the LLM request lifecycle into a dedicated object for easier future evolution (tool classification, background routing).

Decision follow-up (from previous edit-log questions)
- Deprecation: SystemEvent::ToolCallRequested path is deprecated and will be replaced by dedicated event types routed by EventBus in a future refactor. The legacy path remains functional for now and is instrumented with warnings.
- We retain the AppEvent::Llm(Event::ToolCall) injection point only as a shim that forwards to the deprecated SystemEvent path until the refactor lands.

Next steps
- Milestone 2 (finish): Move message assembly (system prompt + conversation) into RequestSession::new and narrow prepare_and_run_llm_call further, or replace it entirely.
- Milestone 3: Introduce tool classification and allow routing to background channel; keep subscribe-before-send semantics in RequestSession.
- Add configurable history_char_budget and per-tool timeout in LLMParameters.
- Tests: add integration tests covering multi-call parallel execution and deterministic ordering; timeout and error surfaces; response parsing robustness.

Notable potential issues (tracked)
- Per-call timeout remains hard-coded at 30s inside RequestSession tool execution; make configurable.
- If a tool task panics, JoinSet returns a join error and we skip that outcome; session proceeds next cycle but may benefit from surfacing a system error message proactively.

Compatibility
- No external dependencies added.
- Public API preserved; internal visibility adjusted with pub(super) to enable session reuse.
