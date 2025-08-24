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
// AI: add your response here

## Plan for improvement
// AI: Add your response here

## Questions Requiring Decision
// AI: Add your response here

## Quality Checklist
- [ ] Remove transitive allocations, use iterators
- [ ] Identify and remove unnecessary `.clone()`, use clear ownership
- [ ] Use strong types with Serialize/Deserialize, **NEVER** use stringly types
- [ ] Check again for stringly types
// AI: Add more here

// Address the `AI:` comments above, filling in areas, reviewing the provided code, addressing my initial notes, providing feedback and in-depths analysis, creating a new plan moving forward AI!
