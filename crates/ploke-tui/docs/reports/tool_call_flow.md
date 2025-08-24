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

<!-- Start New Instructions -->
I've added some more files to review so we can plan the next steps to create e2e tests against a real API and parsed code graph with dense vector embeddings pre-loaded into a database. Here is a description of the files I've just added:

`crates/ploke-io/src/handle.rs`
- short file that defines `IoManagerHandle` used broadly for abstracting IO from other systems in `ploke-tui`.
- Underutilized in current implementation of some tool calls.
- Note that the `IoManagerHandle::new()` command starts a new thread.

`crates/ploke-tui/src/observability.rs`
- A relatively new file that we are hoping to use for persisting information about the tool calls for later analysis, and so we can build a growing store of data on tool calling for use in both later memory systems and solution path exploration and recording in the future.

`crates/ploke-db/src/lib.rs`
`crates/ploke-db/src/get_by_id/mod.rs`
- provided `lib.rs` for info on available database functions
- newly added trait `GetNodeInfo` with method `paths_from_id` and struct `NodePaths` provide helpful abstraction for constructing useful tool output for the tool responses to the LLM.
- **should** integrate new database method into tool handling where helpful.
- provides an example of how to abstract desired database queries from tool call functions:
  - Create desired query output
  - Use strong types
  - Construct appropriate queries, using `@ NOW` in last field of stored relations for up-to-date data
  - Adds tests and adapters (`TryFrom` here, could be `From` as well)
  - Informative test that shows clear information on intermediate steps and returned data for manual review during tests.
  - Provides modular and pluggable implementation that has been verified independantly and can be used more broadly by other systems.

`crates/ploke-core/src/io_types.rs`
`crates/ploke-core/src/rag_types.rs`
- Shared types that are used in crates across the project.

`crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs`
- Previous tests, some of which are good and passing, some are bad and passing. This is a mixed bag, and I would rather we write a new set of tests and then compare them to the implementation here, using a pre-loaded `App` and `Database` for a realistic test against a live API endpoint.

`crates/ploke-tui/src/test_harness.rs`
- Test harness so we don't need to instantiate the `App` and other types for each test. May want to use as-is, may want to modify slightly for loading the `Database` that has been pre-loaded with the dense vector embeddings of the target `fixture_nodes`, since creation of the embeddings can take minutes.

`crates/ploke-tui/src/tracing_setup.rs`
- Tracing framework used in various locations across the crate. Good to rely on for tests. We should add an argument to make the console printing optional, since printing to console when we run the actual app results in the output writing over the `ratatui` application display. Console output still useful during tests, however.

`tests/fixture_crates/fixture_nodes/src/const_static.rs`
`tests/fixture_crates/fixture_nodes/src/enums.rs`
`tests/fixture_crates/fixture_nodes/src/lib.rs`
`tests/fixture_crates/fixture_nodes/src/structs.rs`
- Test fixture, used as a target by 
  - `syn_parser` tests
  - `ploke-transform` tests
  - `ploke-db` tests
  - `ploke-embed` tests
  - probably more
- Each and every code item in the fixture has been verified to be parsed into expected values for all fields in exhaustive `syn_parser` tests.
- Similarly, all code items have been tested for unique node ids, unique and correct relations
- We have other fixtures, but this fixture has been verified by so many tests that it is a good target for new features. It provides more files than those included here (to keep us focused I'm just adding these files), and should provide varied targets for testing different target node types.

**test output**
I've also included the test output from our current test suite to provide some context on what is and is not working currently. As noted above, some of the tests are good and some are less good. A good live test should:
- Verify whether our system is functioning correctly when used in a realistic scenario
- Use a real OpenRouter API endpoint and model for the tests to assess the e2e efficacy of our tests
- Provide varying levels of detailed output by using `tracing` with logging statements, `span` entering, `instrument`, to provide detailed output that can provide assistence when calling the tests to see when discrete steps complete, to debug the tests and underlying functionality, and save logged output for later review.
- **It is OK for tests to fail**: This teaches us something about our program. We write the tests to assert the desired behavior in our program, and then the tests help us discover the gaps or mistakes in our implementation.
- **Tests should be informative**: A test should verify that our functions do or do not have certain properties. For example, does our function have the property that 
  - a live LLM endpoint can receive our full instructions and provided context, and respond with a correctly formed tool request that can be successfully applied by our system?
  - API endpoints are correctly called?
- **Tests for failure paths**: We should not only test happy paths, but also ensure that failure states are handled as expected. e.g.
  - Does calling a target API provider that does not support tool calls with a tool call request respond with the expected API error? When we receive the error, is it being processed by our functions as expected?
  - If we send a request with a very small maximum tokens such that the token request is impossible to complete, what happens? We can write a test that shows desired behavior (probably we should send a message for the UI to alert the user that the token on sent requests is too small for tool calls to complete successfully with instructions that the user can use to increase the tokens allowed in the request, or alternatively turn off tool-calling). Then the test will fail until we have implemented the desired behavior. We can also add comments describing the situation so no one comes along later to change the test in a misinformed effort to improve our tests (making them pass but invalidating the purpose of the test)
- **The purpose of a test is NOT to pass**: The purpose of a test is to provide us with information about what our program does and does not do, what properties our program does or does not have, and sometimes to provide an example of what we would like our program to do. The purpose of a test is **NOT** to pass. A test should only pass when it correctly validates a property or behavior of our program, thereby providing information we can use to continue building our system with certainty in the behavior of the underlying systems.

Review the implementation of the plan in `tool_call_flow.md`. What has been implemented, what has yet to be implemented? Are there any crucial gaps or style violations? What was implemented well? What are the next steps? Are there any open questions blocking further steps? Are there any clear improvements that could be made? What is your critique of the overall system? Add your response to a new document in the same directory as `tool_call_flow.md`. Soon we will continue the implementation, but this is a good chance to reflect on what has been done so far.

In addition, identify which steps should come next, and create a list of files required for each step. Create a new document with a short description of each file, a list of relevant functions or code items with short descriptions of each, and when it would be useful to reference the file. This should help provide us with a condensed source of information that will allow us to keep our cognitive overhead lean, but still have a good reference for when we need more precise information while building out the next steps in the implementation.

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
- Deprecated path: Removed in favor of unified LlmTool::Requested dispatch in llm_manager.
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

