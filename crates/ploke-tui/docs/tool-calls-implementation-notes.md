# Tool Calls — Implementation Progress and Notes

Date: 2025-08-17
Owner: TUI team

Milestone Checklist
- [x] SystemEvent variants: ToolCallRequested, ToolCallCompleted, ToolCallFailed.
- [x] LLM manager routes Event::ToolCall -> SystemEvent::ToolCallRequested.
- [x] Await tool result via EventBus with correlation (request_id, call_id) + timeout.
- [x] request_code_context prototype behavior implemented (currently in LLM layer).
- [x] Move dispatcher to System handler (Milestone 3 proper).
- [ ] Extract RequestSession from prepare_and_run_llm_call (Milestone 2 proper).
- [x] Remove inline stub attempt_request_code_context.
- [x] Factor top_k heuristic into shared helper.
- [ ] Add prompt-size caps (message window + RAG context budget).
- [ ] Add tracing spans with request_id, parent_id, call_id across LLM/System.
- [ ] Tests: unit dispatcher, integration session, E2E smoke.
- [ ] Metrics/telemetry (optional): counts, latency, success/failure.

Design Notes
- Event contracts (as implemented/assumed)
  - SystemEvent::ToolCallRequested { request_id, parent_id, vendor, name, arguments, call_id }
  - SystemEvent::ToolCallCompleted { request_id, parent_id, call_id, content }
  - SystemEvent::ToolCallFailed { request_id, parent_id, call_id, error }
- Tool: request_code_context
  - Args: { token_budget: u32, hint?: string }
  - Query selection: hint if non-empty else last user message
  - top_k: clamp(token_budget / 200, 5, 20)
  - Returns JSON: { ok, query, top_k, results: [{id, score}, ...] }
- Token budget strategy (30k window)
  - For PromptConstructed: cap prior messages (e.g., last N or char budget) and cap RAG context by token_budget.
  - Tool results are lightweight (ids/scores) → suited for budget.

Proposed Refactors
1) System-side dispatcher
   - Create/extend System handler module to match name and call rag.hybrid_search.
   - Validate args; on error, emit ToolCallFailed with friendly msg.
   - Build JSON payload exactly as tool role message expects (stringified).
2) RequestSession
   - New llm/session.rs with struct RequestSession { messages, attempts, params, client, provider_config, parent_id, assistant_message_id }
   - Methods: run(), handle_tool_calls(), await_tool_results().
3) Helpers
   - tools/top_k.rs: calc_top_k_for_budget(token_budget: u32) -> usize
   - prompt/window.rs: select_conversation_window(history, approx_token_limit) -> Vec<Message>
4) Cleanup
   - Remove attempt_request_code_context from llm/mod.rs after dispatcher move.
   - Keep rag.rs user-facing commands as-is (SysInfo formatting) — different audience than tool JSON.

Testing Plan
- Unit
  - Dispatcher: parse args, hint vs user query; deterministic JSON payload assembly.
  - Top_k calc helper clamps correctly.
- Integration
  - Session emits ToolCallRequested; mocked System emits Completed; session appends tool output and continues.
- E2E
  - Canned provider response → single tool call → tool output → final assistant message.

Operational Notes
- Subscribe to realtime EventBus before sending ToolCall to avoid missing fast completion events.
- Use 30s timeout for tool completion; surface a helpful system message and send a tool error JSON to model.
- Ensure AppEvent enums remain Clone for broadcast; avoid including channels inside events.

Open Questions
- Do we need parallel tool call handling now? Probably later; keep sequential for simplicity.
- Should the UI show tool outputs explicitly? For now, only the LLM sees tool JSON; UI gets final assistant content.

Changelog (to be updated)
- 2025-08-17: Moved dispatcher to system handler (app_state::handlers::rag::handle_tool_call_requested); removed inline stub; added top_k helper.
- 2025-08-17: Initial notes created based on current implementation.
