# Tool Calls — Consolidated Plan and Architecture (v1)

Date: 2025-08-17
Owner: TUI team
Scope: ploke-tui crate (LLM/event routing and tools)

Summary (Current State)
- Event flow:
  - LLM loop detects OpenAI-style tool_calls, emits AppEvent::System(SystemEvent::ToolCallRequested { request_id, parent_id, vendor, name, arguments, call_id }) on the EventBus realtime channel, and awaits a correlated completion/failure via session::await_tool_result (30s timeout).
  - System dispatcher implemented in app_state::handlers::rag::handle_tool_call_requested executes request_code_context by calling RAG hybrid_search and emits ToolCallCompleted/ToolCallFailed with JSON payloads intended for the model.
- Correlation: request_id + call_id are used end-to-end for matching.
- Configuration: tool_max_retries and tool_token_limit live in LLMParameters; cap_messages_by_chars provides a simple prompt window cap.
- Parallelism: The LLM loop currently processes tool_calls sequentially.
- Channels: Tool events currently use EventBus.realtime_tx for both routing and awaiting.
- Helpers: calc_top_k_for_budget(token_budget) centralized in rag handler.

Decisions and Goals
- Standardize LLM-facing code format
  - LLM-facing code snippets must use Ploke Code Snippet Format:
    <code="ABSOLUTE/PATH.rs" #START:END>…code…</code>
  - Both injected context (from tools/RAG) and any assistant-produced code should use this exact format for consistency and deterministic downstream parsing.
  - UI-facing SysInfo formatting may remain more free-form; this document standardizes only LLM-facing messages.
- Parallel tool calls
  - When the provider returns multiple tool_calls in a single completion, we will execute them concurrently and await all results (with per-call timeouts).
  - Deterministic ordering: after all complete, append tool messages to the LLM request in a stable order (e.g., by call_id).
- Channel usage (temporary)
  - Keep using realtime for ToolCallRequested/Completed/Failed to match current await_tool_result implementation.
  - Plan: introduce routing by tool “class” soon:
    - Short/low-latency tools → realtime channel.
    - Long-running/high-latency/expensive tools → background channel; the session will subscribe to background for tools. We’ll move to background by default once the LLM session code supports it.
- RequestSession abstraction
  - Extract the per-request loop into llm/session.rs with a RequestSession struct owning messages, attempts, params, client, provider, parent_id.
  - Responsibilities:
    - Construct OpenAI payloads (with tools) and send.
    - Detect tool_calls, emit SystemEvent::ToolCallRequested for each, concurrently await completions/failures, append tool messages, and continue.
    - Enforce attempt/retry limits and timeouts.
    - Apply prompt window caps before each call.
- Tool dispatcher (System)
  - Continue handling request_code_context in system layer.
  - Add a tool classification field or registry so we can choose channel and execution strategy (spawned tasks for long-running tools).
- Token budgets and caps
  - Keep char-based cap for now; add token-aware caps later (provider-dependent).
  - Bound RAG context by tool_token_limit and use top_k heuristic calc_top_k_for_budget.
- Telemetry and tracing
  - Add spans with request_id, parent_id, call_id on both LLM and System sides.
  - Metrics (future): counts, latency, success/failure per tool, timeouts.

Contracts
- SystemEvent
  - ToolCallRequested { request_id: Uuid, parent_id: Uuid, vendor: ToolVendor, name: String, arguments: serde_json::Value, call_id: String }
  - ToolCallCompleted { request_id: Uuid, parent_id: Uuid, call_id: String, content: String }
  - ToolCallFailed { request_id: Uuid, parent_id: Uuid, call_id: String, error: String }
- Tool: request_code_context
  - Args JSON: { "token_budget": u32, "hint"?: string }
  - Behavior:
    - Query = hint if provided and non-empty; otherwise last user message.
    - top_k = clamp(token_budget / 200, 5, 20).
    - Call rag.hybrid_search(query, top_k).
    - Return LLM-facing JSON payload:
      {
        "ok": true,
        "query": "...",
        "top_k": N,
        "results": [
          { "id": "<DocId>", "score": <f32> }
        ]
      }
    - Future: when returning snippet content instead of ids/scores, embed code using Ploke Code Snippet Format in a compact array (budget-aware).
- LLM-facing message rules
  - Tool outputs are passed as the raw JSON string in a tool role message with tool_call_id matching the provider call id.
  - Any code material (now or in future tools) must use the Ploke Code Snippet Format.

Architecture and Flow (updated)
1) LLM request assembled with conversation window cap and tool definitions.
2) Provider returns tool_calls (possibly multiple).
3) Session subscribes to EventBus, emits SystemEvent::ToolCallRequested per call (subscribe before send), then concurrently awaits ToolCallCompleted/Failed for each.
4) For each completion, push RequestMessage::new_tool(content, call_id); for failures, push a system message and a tool message with { "ok": false, "error": "…" }.
5) After all tool calls finish (or timeout), re-issue the LLM call so the model can incorporate results.
6) When no tool_calls remain, finalize assistant content.

Implementation Plan (incremental)
1) RequestSession extraction (Milestone 2)
   - Introduce struct RequestSession in llm/session.rs with run() -> Result<String, LlmError>.
   - Move loop and event subscription there; keep current realtime channel usage.
   - Implement parallel tool call handling using join_all/select_all; stable-order append by call_id.
2) System dispatcher (Milestone 3)
   - Keep request_code_context in app_state::handlers::rag::handle_tool_call_requested.
   - Add tool classification (short vs long-running) and a dispatch strategy hook: immediate vs spawn background task. For now, continue on realtime.
3) Format unification (LLM-facing code)
   - Update prompts to instruct the assistant to use Ploke Code Snippet Format.
   - Ensure any tool that returns code (future) emits that format.
4) Token limits and context sizing (Milestone 5)
   - Maintain cap_messages_by_chars; add configurable caps in LLMParameters (e.g., history_char_budget).
   - Bound tool context by tool_token_limit and ensure top_k calculation comes from shared helper.
5) Telemetry
   - Add tracing spans on both sides; log timeouts and failures at warn; completions at info.
6) Tests
   - Unit: top_k calc; dispatcher arg parsing and payload shape; snippet format enforcement.
   - Integration: session with fake tool_calls and mocked system responses; parallel calls resolve and messages appended deterministically.
   - E2E: canned provider response → tool outputs → final assistant content, no panics.

Acceptance Criteria
- Multiple tool_calls in a single LLM response are handled concurrently; results appended in stable order; request proceeds without deadlocks.
- Tool events routed reliably; session subscribes before send; 30s per-call timeout with clear user/system feedback.
- LLM-facing code uses standardized Ploke Code Snippet Format everywhere (tool outputs that include code and assistant responses).
- request_code_context returns deterministic JSON with ids/scores and respects tool_token_limit-derived top_k.
- No regressions in UI responsiveness; channel strategy remains compatible (realtime for now).

Migration Notes
- Leave current realtime channel usage in place until RequestSession is extracted and background routing is implemented.
- Once session supports background tool events, switch ToolCallRequested/Completed/Failed to background channel by default; only emit realtime events that directly affect UI rendering.

Risks and Mitigations
- Channel overload on realtime: mitigate by moving tools to background soon after RequestSession supports it.
- Token window overflow: enforce caps and keep tool outputs compact (ids/scores); consider summarization for larger payloads.
- Provider schema differences: currently OpenAI-focused; add adapters as needed.

Appendix
- Ploke Code Snippet Format (LLM-facing)
  <code="ABSOLUTE/PATH.rs" #123:456>
  // code…
  </code>
