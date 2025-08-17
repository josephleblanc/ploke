# Tool Calls Plan Review — Current Status

Date: 2025-08-17
Owner: TUI team
Scope: ploke-tui crate (LLM/event routing prototype)

Summary
- We have implemented key parts of Milestones 1 and 3, and a partial version of Milestone 2, directly inside llm/mod.rs.
- The current implementation successfully:
  - Detects tool calls from the LLM provider (OpenAI format).
  - Emits AppEvent::Llm(Event::ToolCall { ... }) and immediately forwards AppEvent::System(SystemEvent::ToolCallRequested { ... }).
  - Performs the request_code_context tool execution by calling RAG hybrid_search and emits SystemEvent::ToolCallCompleted/Failed with structured JSON results.
  - Waits asynchronously (with a 30s timeout) for correlated SystemEvent completions/failures and pushes tool role messages back into the request loop so the model can observe tool outputs.
- However, the dispatcher that actually executes the tool is currently implemented inside llm_manager rather than the system layer, which deviates from the plan’s System-side responsibility (Milestone 3). The RequestSession abstraction (Milestone 2) is also embedded inside prepare_and_run_llm_call rather than as a dedicated type.

Mapping to the Plan Milestones
1) Plumbing: Add SystemEvent variants and routing in llm_manager → DONE
   - llm_manager forwards Event::ToolCall to SystemEvent::ToolCallRequested for telemetry/correlation.

2) RequestSession v1 → PARTIAL
   - The per-request control flow lives inside prepare_and_run_llm_call.
   - It subscribes to EventBus before routing ToolCall, awaits ToolCallCompleted/Failed, handles timeout, appends messages, and continues.
   - Missing: dedicated RequestSession type, encapsulated state (attempts, messages, params), and clean separation from llm_manager.

3) Tool dispatcher in System side → PARTIAL/PLACED INCORRECTLY
   - request_code_context is implemented, but in llm_manager (LLM layer), not a system handler module.
   - Should be moved to System (e.g., a system handler or AgentSystem), decoupled from LLM loop.

4) Remove inline stub handling → NOT DONE (but stub is unused)
   - attempt_request_code_context remains in llm/mod.rs as a dead/stub function. Can be removed once dispatcher is fully in System.

5) Polish (timeouts, metrics, validation) → PARTIAL
   - Timeout (30s) is present. Logging is present, tracing spans can be improved. Argument validation exists for request_code_context. Top-k heuristic implemented.

Key Findings and Cleanups
- Responsibility split (LLM vs System): Move the request_code_context execution (RAG calls and result emission) out of llm_manager into the System handler. Keep llm_manager limited to emitting ToolCallRequested and awaiting ToolCallCompleted/Failed.
- Dead code: Remove attempt_request_code_context from llm/mod.rs after dispatcher is moved; it’s misleading.
- Duplication and cohesion:
  - RAG usage exists in two styles:
    - UI/Operator commands (rag.rs) → SysInfo-formatted messages.
    - Tool results (llm/mod.rs) → Structured JSON for model consumption.
    This is intentional, but consider centralizing the “hybrid_search -> Vec<(Id, Score)> -> JSON” transform in a small utility function to ensure consistency between system dispatcher and any future reuses.
- Configuration cohesion:
  - top_k heuristic is embedded in llm/mod.rs; extract a small shared helper (e.g., tools::calc_top_k_for_budget) so the System dispatcher uses the same logic.
  - tool_max_retries and tool_token_limit are in LLMParameters; ensure they’re surfaced in user config or defaults in one place and documented.
- Event correlation:
  - The design subscribes to the broadcast channel before emitting the ToolCall event → good. Keep that pattern when moving logic to System.
  - Consider structured tracing spans with request_id, parent_id, call_id at the points of send/receive (both LLM and System sides).
- Parallel tool calls:
  - Current loop processes tool_calls sequentially. That’s fine for now; if we enable provider parallel_tool_calls, we may later collect all futures and await them concurrently while preserving correlation.
- Token budget and 30k conversation cap:
  - Current code filters out SysInfo messages, but PromptConstructed can still include significant content (headers + assembled context + user/assistant history).
  - Action items:
    - Cap the number of prior messages included (sliding window by token/char estimate).
    - Bound the RAG context size by budget and add light summarization if needed.
    - For tool outputs, we currently return only ids/scores (small) → good for token economy.
- Errors and UX:
  - Provider HTTP error bodies mapped to user-friendly messages → good.
  - Tool failures map to both system messages and tool JSON error objects for the model → good.
  - Consider surfacing tool timeouts to the UI as well (SysInfo).

Are We Ready To Proceed?
Yes—with targeted cleanups:
- Move tool execution to the System handler (per design).
- Extract a RequestSession abstraction (API-level) to improve readability and future parallelism.
- Add a light prompt-size cap to respect 30k token conversation constraints.
- Remove dead stub function and factor out top_k heuristic.

Likely Files To Change (ask to add them before editing)
- crates/ploke-tui/src/system/... (System event handler/dispatcher module): implement ToolCall dispatcher here.
- crates/ploke-tui/src/llm/session.rs (new): extract RequestSession from prepare_and_run_llm_call.
- crates/ploke-tui/src/llm/mod.rs: remove attempt_request_code_context; reduce dispatcher logic to only emit/await events; move top_k calc to shared helper.
- crates/ploke-tui/src/app_state/handlers/system.rs (or similar): if a system handler exists, integrate the dispatcher; otherwise create a module to own SystemEvent handling for tools.
- crates/ploke-tui/src/event_bus.rs (or wherever EventBus is defined): ensure broadcast channels are Clone, and add tracing hooks if desired.
- crates/ploke-tui/src/user_config/...: expose tool_max_retries/tool_token_limit overrides.
- Tests: crates/ploke-tui/tests/... for dispatcher unit/integration tests and a smoke E2E with canned tool_calls.

Risk/Notes
- Keep AppEvent and nested enums Clone for broadcast.
- Manage race conditions if multiple tool_calls are returned: current loop processes sequentially; acceptable for prototype.
- Monitor size of PromptConstructed. Consider chunking or summarization to stay under 30k tokens.

Next Steps (incremental)
1) System Dispatcher
   - Implement SystemEvent::ToolCallRequested handler that matches name == "request_code_context", validates args, resolves query (hint or last user message), computes top_k, calls rag.hybrid_search, emits ToolCallCompleted/Failed.
   - Add tracing spans (request_id, parent_id, call_id).
2) RequestSession extraction
   - Create llm/session.rs, move attempt loop + event subscription there. Keep public API simple (run() -> Result<String, LlmError>).
3) Cleanup
   - Remove attempt_request_code_context stub.
   - Factor top_k heuristic helper.
4) Token limits
   - Add prompt window cap by message count or char length; ensure RAG context respects budget.
5) Tests
   - Unit test: dispatcher with mocked RAG returns deterministic JSON payload.
   - Integration: fake tool_calls flow; assert event routing and tool result inclusion.
   - E2E smoke: canned provider response → final assistant content, no panics.

Conclusion
We’re close to the intended architecture. With the dispatcher relocation, RequestSession extraction, and prompt-size caps, we’ll be ready to complete Milestones 3–5 and stabilize the prototype.
