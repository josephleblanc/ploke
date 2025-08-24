# Notes on Tool Call Flow

## Initial Notes

Going over the flow of control/data through the API call to the LLM, focusing on the new tool calling feature we have been implementing. The purpose of this review is to have a single document to undestand the life-cycle of the LLM tool call, and identify where we can improve the system.

Currently the system is semi-functional. Some tests are passing, some tests are failing. We are passing all internal tests for segments of the overall flow, but for full e2e tests on a real API endpoint we are failing on some tests. Some of this may be due to poor test execution, but there are areas for improvement within the flow itself and in the way the individual parts are or are not achieving their intended purpose.

Below we focus specifically on the `handle_request_context` tool, which was significantly failing in the intended functionality of providing the LLM with additional context, because it is not utilizing our robust system for precisely reading exact byte offsets of the code items stored in our code graph database, and instead performing ad hoc file reads or returning node ids instead of utilizing the existing dataflows we have established for retrieving exact snippets to provide targeted context for the LLM.

Another area for improvement is the fields the tool call is returning, and the weak stringly types used in the dataflow. 
- We should **NEVER** use stringly types
- **ALWAYS** add a new struct to Serialize and Deserialize for more formal verification of the structure of the json and for more concise, precise, maintainable code.

## Rough overview

Tool call event flow:
1. `ploke-tui/src/llm/mod.rs:llm_manager`
- receives `AppEvent::LlmTool(ToolEvent::Requested { .. }` via `event_bus`
- starts new thread with `rag::dispatcher::handle_tool_call_requested(ToolCallParams { .. })`

2. `ploke-tui/src/rag/dispatcher.rs`
- defines `handle_tool_call_requested`
- matches on "name" (stringly typed, need to make more type-safe with serde Serialize/Deserialize)
- routes tool to locally defined functions, e.g. `handle_request_context`
  - suppose we call `handle_request_context`

3. `ploke-tui/src/rag/tools.rs`
- defines tool functions, e.g. `handle_request_context`
- currently only gets the node ids and match score
  - should get full code snippets
    - **should change from hybrid_search to assemble_context now**.
  - `ploke-rag/src/core/mod.rs`
    - defines `hybrid_search`, also `get_context`
    - returns `AssembledContext` via `assemble_context`
    - `ploke-rag/src/context/mod.rs` defines `assemble_context`
      - correctly uses `io.get_snippets_batch`
      - trims results to token limit
      - returns typed vec of `ContextPart` (defined in `ploke-core/src/rag_types.rs`)
- sends new event to `AppEvent::System(SystemEvent::ToolCallCompleted { .. }`

4. `ploke-tui/llm/session.rs`
- defines function `await_tool_result`
- receives `AppEvent::System(SystemEvent::ToolCallCompleted { .. }`
  - checks matching request_id & call_id
  - uses timeout

5. `ploke-tui/llm/tool_call.rs`
- calls `await_tool_result` from function `dispatch_and_wait`
- `dispatch_and_await` called within new thread within function `execute_tool_calls`

6. `ploke-tui/llm/session.rs`
- within `RequestSession.run` loop calls `execute_tool_calls`
- checks to see if model called supports tools
- processes returned value (currently `String`, bad)
  - Should change return types of tools to be strongly typed, perhaps with marker trait or make generic function over tool call return types, which **MUST** all be `Serialize` and `Deserialize` for conversion to `json` via `serde_json::to_value` for tool calling requests via `RequestMessage`.
- pushes tool output to `self.messages`
- loops again if llm response has tool calls, otherwise returns string.
- Q: **Not exactly clear** on the intended flow here. Overall it seems like this loops on tool calls to feed the LLM the output of the tool call and see if it wants to send another tool call? Or maybe its just going over the tool calls and giving the LLM a retry if it sent a bad tool call or something?
- Q: Once the LLM tool calls have been completed, returns a full string of the tool call attempts along with the LLM response in a single string?
- returns `Result<String, LlmError>`

7. `ploke-tui/llm/mod.rs`
- within `prepare_and_run_llm_call`, at the end of the function, calls `session.run().await` as returned value
- `prepare_and_run_llm_call` is itself called from `process_llm_request`
- sends an update to the `StateCommand` with a message that completes an earlier message


## In-depth Analysis

What is working well
- Event flow: LlmTool::Requested is emitted, dispatcher routes by tool name, tool handlers emit SystemEvent::ToolCallCompleted/Failed; await_tool_result correlates by (request_id, call_id) with timeout. Concurrency via JoinSet with deterministic ordering by call_id is solid.
- Session loop: RequestSession::run enforces a character-budgeted message history, builds OpenAI-compatible payloads, supports tools, and appends tool outputs back into messages to allow iterative tool-call cycles.
- Error reporting: Clear mapping of HTTP status codes to user-friendly messages with extra guidance for 404 "tool not supported" endpoints. Provider 200-bodies containing error objects are detected.
- Request payloads: build_openai_request produces stable, snapshot-tested JSON, including provider preferences when tools are in play.

Gaps and risks
- Tool fallback policy: We retry once without tools on a 404 “support tool” response and add a system message with remediation; make this configurable later.
- Stringly typed tool IO: Eliminated for request_code_context and get_file_metadata; apply_code_edit now returns a typed ApplyCodeEditResult with staging info. Next: add serde round‑trips and versioning fields.
- Context quality: request_code_context assembles real snippets via RagService::get_context; further refinements like richer metadata and range stitching remain.
- History budgeting: Token-based capping (approx chars/4) with char-based fallback; wire real tokenizer adapters later.
- Deprecated path: SystemEvent::ToolCallRequested remains as a compatibility path; schedule removal after downstream migration.
- DB query in apply_code_edit_tool: CozoScript still inlined JSON; prefer parameterization when feasible.
- Cloning and allocations: Reasonable for now; audit hot paths post-migration.

Observability and test coverage
- Good tracing coverage with concise, structured logs for dispatch and API responses.
- Snapshot tests for request payloads exist. Missing are end-to-end tests for the tool-call cycle and typed tool IO round-trips.

## Plan for improvement

1) Make tool IO strongly typed
- Define typed structs for tool inputs/outputs in a shared module (e.g., ploke_core::rag_types or a new ploke_core::tool_io).
- For request_code_context:
  - Input: RequestCodeContextArgs { token_budget: u32, hint: Option<String> } already exists; reuse it end-to-end.
  - Output: RequestCodeContextResult { ok: bool, query: String, top_k: usize, context: AssembledContext }.
- Serialize outputs with serde_json::to_string before RequestMessage::new_tool, but validate types at boundaries.

2) Return assembled snippets, not just IDs
- Replace rag.hybrid_search() in handle_request_context with rag.get_context(query, top_k, budget, RetrievalStrategy::Hybrid { .. }).
- Thread a TokenBudget derived from token_budget, and use RagService defaults for ordering and policy.
- Send back AssembledContext in the tool output so the LLM can cite exact text.

3) Decide and implement tool fallback policy
- Option A (strict): keep current fail-fast on 404 with user guidance.
- Option B (lenient): retry once without tools if 404 "tool unsupported" and tools are required.
- Gate behavior by a config flag (e.g., provider_registry.require_tool_support or a new flag), remove unused tools_fallback_attempted var.

4) Improve budgeting
- Add a token-based history budgeting function (cap_messages_by_tokens) using an injected TokenCounter; default to ApproxCharTokenizer for now.
- Keep char-budgeting as a fallback for providers without tokenizer info.

5) Clean up deprecated paths and sharp edges
- Migrate all call sites to LlmTool events and remove SystemEvent::ToolCallRequested once downstreams are updated.
- Audit clones in tool_call.rs and session.rs; prefer &str and small borrows where practical.

6) Tests and docs
- Add end-to-end tests for the tool-call cycle, including timeout and failure propagation.
- Add tests for typed tool IO (serde round-trip), and for assemble_context integration.
- Update docs with the final policy decisions and typed schemas.

## Questions Requiring Decision

- Tool fallback policy: Should we retry once without tools on 404, or remain strict and surface guidance? Should this be configurable?
  - Retry once, should be configurable, outside scope of current plan.
- Tool output schema: Do we standardize on RequestCodeContextResult carrying AssembledContext? Any versioning or “type” field needed for forward compatibility?
  - "type" field not yet fully designed, can leave mostly stub for now.
  - See following answer.
- Presentation to the LLM: Send structured JSON as the tool role content (current), or pre-render textual snippets? Recommendation: structured JSON; let the prompt instruct the model to read fields.
  - Use structured JSON, but do not expose details that would confuse the LLM. Consider: What is useful to the LLM in the context response? It only needs:
    - snippets for context
    - name + canonical path of code item
    - local file location
  - Any more does not add anything useful to the LLM, and provides opportunity for failure. Agreed?
- Concurrency limits: Keep unbounded JoinSet or cap concurrent tool calls (e.g., N at a time)? If capped, what default?
- Auto-approval of edits: Should auto_confirm_edits default to off in production profiles? Who is the authority to approve (explicit user command vs. policy rule)?
  - User config, outside current scope of plan.
- Provider preference: Include provider.order even when tools are disabled, or keep current behavior where it’s only set with tools?
  - Keep current behavior
- Token budgeting: Adopt token-based budgeting for history by default, retaining char-based as fallback?
  - Yes, agreed

## Quality Checklist
- [ ] Remove transitive allocations, use iterators
- [ ] Identify and remove unnecessary `.clone()`, use clear ownership
- [ ] Use strong types with Serialize/Deserialize, **NEVER** use stringly types
- [ ] Check again for stringly types
- [ ] Replace handle_request_context hybrid_search with get_context returning AssembledContext
- [ ] Introduce RequestCodeContextResult and other typed tool IO structs; add serde tests
- [ ] Decide and implement tool 404 fallback policy; remove unused tools_fallback_attempted
  - DECISION: Try again without tool call, use `add_message_immedaite` to let user know the API provider endpoint does not support tool calls (even if the model does)
  - Add placeholder for config variable behavior, full implementation out of scope for this plan.
- [ ] Add token-based history budgeting with a TokenCounter; keep char-budget as fallback
- [ ] Add end-to-end tests for tool-call cycle (success, failure, timeout, ordering)
- [ ] Remove deprecated SystemEvent::ToolCallRequested flow once migration is complete
- [ ] Review Cozo query construction in apply_code_edit_tool; prefer parameterization when feasible
- [ ] Ensure deterministic ordering of tool outcomes and stable call_id semantics
- [ ] Expand tracing with request_id/parent_id/call_id spans; ensure no PII/API keys in logs
- [ ] Validate file paths and namespaces rigorously; verify tracking hashes before edits
- [ ] Document provider preference rules and when provider.order is applied

