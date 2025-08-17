# Plan: Prototype Tool Handling via EventBus → SystemEvent → RAG Hybrid Search

Status: Draft v0.1  
Owner: TUI team  
Scope: ploke-tui crate (LLM/event routing prototype)

Goals
- Route LLM-originated tool calls through the EventBus into the system layer (SystemEvent), rather than handling inline in the LLM request loop.
- Prototype a single tool, request_code_context, and implement it by calling our RAG hybrid search to retrieve relevant code/doc context.
- Preserve correlation across events (request_id, parent_id, call_id) and feed tool results back to the ongoing LLM request loop so the model can “observe” tool outputs.
- Keep the change surface area minimal and incremental.

Non-goals
- Full agent framework or Long-running tool orchestration.
- Streamed tool outputs.
- Full generalization of tools across providers; we only support the OpenAI-like function calling schema for now.

Current state (as of this plan)
- LLM tool definition exists: request_code_context in llm/mod.rs (OpenAI tools format).
- prepare_and_run_llm_call detects tool_calls in API responses and currently handles them inline by calling a stub (attempt_request_code_context), then pushing a tool message back to messages and looping.
- llm_manager logs AppEvent::Llm(Event::ToolCall { ... }) but does not route it onward.
- RAG service is available via state.rag and exposes async hybrid_search, bm25_* APIs, and get_context. The RAG handler currently formats user-facing SysInfo messages; for tool results we will return structured JSON instead.

High-level design
- Introduce a system-level event to request tool invocation. The llm manager will route Event::ToolCall to SystemEvent and an async system handler will execute the tool.
- Introduce a per-request “RequestSession” in llm, which owns the conversation request loop and subscribes to EventBus to receive the asynchronous tool results.
- The RequestSession pauses when it emits a SystemEvent::ToolCallRequested and waits for the corresponding SystemEvent::ToolCallCompleted/Failed (correlated via request_id + call_id). When received, it appends a tool result message to the local messages vec and continues the loop to ask the model to incorporate the tool output.

Event flow
1) LLM API returns tool_calls.  
2) The RequestSession constructs an AppEvent::System(SystemEvent::ToolCallRequested { request_id, parent_id, vendor, name, arguments, call_id }).  
3) EventBus delivers event to a System handler that dispatches to concrete tools.  
4) For request_code_context, the handler queries RAG (hybrid_search) with a query derived from the last user message and/or the function’s hint argument, and a top_k derived from token_budget.  
5) The handler responds by emitting AppEvent::System(SystemEvent::ToolCallCompleted { request_id, parent_id, call_id, content }), where content is a JSON string describing the result. On error, emit ToolCallFailed with an error string.  
6) The RequestSession, subscribed to realtime events, receives the completion event matching its (request_id, call_id), appends a tool message to messages (RequestMessage::new_tool(content, call_id)), and continues the loop.  
7) If no tool_calls remain, it finalizes the assistant content and updates the assistant message to Completed.

Proposed data contracts

New SystemEvent variants (in crate::system):
- ToolCallRequested {
    request_id: Uuid,
    parent_id: Uuid,
    vendor: ToolVendor,       // mirrors llm::ToolVendor
    name: String,             // function name e.g., "request_code_context"
    arguments: serde_json::Value,
    call_id: String,          // provider tool call id
}
- ToolCallCompleted {
    request_id: Uuid,
    parent_id: Uuid,
    call_id: String,
    content: String,          // JSON payload string to pass to LLM as tool output
}
- ToolCallFailed {
    request_id: Uuid,
    parent_id: Uuid,
    call_id: String,
    error: String,            // User-friendly error
}

Notes
- We leverage SystemEvent to orchestrate tools; we do not add a new llm::Event for ToolResult in this prototype to keep changes contained. The RequestSession will subscribe to AppEvent::System and filter for its correlation ids.
- content for ToolCallCompleted should be the exact JSON to pass in the tool role message (RequestMessage::new_tool(content, call_id)).

request_code_context tool: prototype behavior
- Input args: { token_budget: u32, hint?: string }.
- Query selection:
  - If hint present and non-empty: use hint as the search query.
  - Else: use last user message content (chat_history.last_user_msg()).
- top_k heuristic:
  - top_k = clamp(token_budget / 200, 5, 20)
  - Rationale: coarse estimate assuming ~200 tokens per snippet. Tunable later.
- Execution:
  - Call state.rag.as_ref().ok_or_else(...) if missing -> ToolCallFailed.
  - Use rag.hybrid_search(&query, top_k).await -> Vec<(DocId, Score)>.
  - Return JSON payload:
    {
      "ok": true,
      "query": "...",
      "top_k": N,
      "results": [
        { "id": "<DocId>", "score": <f32> },
        ...
      ]
    }
- On error:
  - Emit ToolCallFailed with a friendly error string.
  - The RequestSession converts the failure to a system message and also sends a tool message with { "ok": false, "error": "<msg>" } to let the model recover.

LLM side changes (RequestSession)
- Extract the inline tool handling from prepare_and_run_llm_call into a new per-request actor:
  - Owns: messages Vec<RequestMessage>, attempts counter, params, client, provider_config, parent_id, assistant_message_id.
  - Subscribes to EventBus realtime receiver to listen for SystemEvent tool completions/failures.
  - When tool_calls are returned:
    - For each call: emit SystemEvent::ToolCallRequested with correlation ids.
    - Await ToolCallCompleted/Failed with matching (request_id, call_id) or timeout.
    - On completion: messages.push(RequestMessage::new_tool(content, call_id)).
    - On failure: messages.push(RequestMessage::new_system("...error message...")); messages.push(RequestMessage::new_tool(json!({"ok": false, "error": "..."}).to_string(), call_id)).
    - Continue loop. Stop when attempts > max_retries or the API returns a content response without tool_calls.
- Backpressure: use CommandSender for final UpdateMessage send; for session-level operations we directly await as needed.
- Timeout: consider a per-tool timeout (e.g., 10s-30s) so the RequestSession can retry or fail fast with a helpful message.

System side changes
- Add SystemEvent variants listed above.
- Implement a tool dispatcher (match name) in system handler module:
  - "request_code_context" => call rag.hybrid_search(query, top_k).
  - Build JSON content and emit ToolCallCompleted or ToolCallFailed.
- For now, implement inside an existing system handler; later we can extract to AgentSystem with a registry of Tool implementations.

Telemetry and tracing
- Add spans:
  - request_id, parent_id, call_id on RequestSession and SystemEvent handlers.
- Log levels:
  - info for ToolCallRequested/Completed.
  - warn for ToolCallFailed and timeouts.
- Metrics (future): count of tool calls, latency, success/failure rates.

Testing plan
- Unit tests:
  - Tool dispatcher: given arguments, returns deterministic JSON payload.
  - Top_k heuristic correctness.
- Integration tests:
  - Simulate a RequestSession receiving a tool_calls response; assert SystemEvent is emitted; mock system handler emits ToolCallCompleted; assert RequestSession pushes tool output and continues to next API call.
- E2E smoke test:
  - Configure provider to return a canned tool_calls response; run through to final assistant completion; verify final message content is updated and no panics.
- Failure tests:
  - Missing RAG, invalid args, tool timeout.

Incremental milestones
1) Plumbing: Add SystemEvent variants and routing in llm_manager when Event::ToolCall is received (route to SystemEvent::ToolCallRequested). Keep inline tool handling in prepare_and_run_llm_call temporarily.
2) RequestSession v1: Refactor prepare_and_run_llm_call into a session that subscribes to EventBus and handles ToolCallCompleted by appending tool messages; keep dispatcher stubbed.
3) Tool dispatcher: Implement request_code_context -> RAG hybrid_search with JSON result. Emit Completed/Failed.
4) Remove inline stub handling; switch fully to event-based tool execution.
5) Polish: timeouts, metrics, more robust argument validation.

Open questions / risks
- Broadcast channel cloning: ensure AppEvent and nested enums remain Clone where required; avoid including uncloneable types (e.g., oneshot senders) inside events.
- Race conditions: multiple tool_calls per response require correlating multiple call_ids; ensure RequestSession tracks outstanding calls and waits for all or returns on first failure based on strategy.
- Tool output size: large JSON payloads might hit token limits—consider summarizing.
- Provider differences: non-OpenAI schemas may need adapters.

Appendix: Example JSON payload returned to the model for request_code_context
{
  "ok": true,
  "query": "how to handle tool calls",
  "top_k": 10,
  "results": [
    { "id": "/abs/path/src/lib.rs#132:486", "score": 0.823 },
    { "id": "/abs/path/src/app.rs#10:200", "score": 0.792 }
  ]
}

Mapping to model messages (tool role)
- role: "tool"
- tool_call_id: "<provider-call-id>"
- content: the JSON string above (no additional wrapping)
- The session then continues with an API call using the augmented messages (tools enabled).

Change summary (to be implemented in future PRs)
- Add SystemEvent::ToolCallRequested/Completed/Failed.
- llm_manager: on AppEvent::Llm(Event::ToolCall{...}), forward to AppEvent::System(SystemEvent::ToolCallRequested{...}).
- Create RequestSession abstraction from prepare_and_run_llm_call; subscribe to EventBus realtime; correlate tool results.
- Implement tool dispatcher; for prototype, only request_code_context using RagService::hybrid_search.
- Remove inline attempt_request_code_context once event path is stable.
