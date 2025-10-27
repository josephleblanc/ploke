# LLM Code Edit Feedback — Findings and Recommendations (2025-10-25)

Purpose
- Investigate why the LLM calls the code edit tool correctly, then continues issuing edits instead of returning control to the user.
- Determine whether the LLM receives explicit feedback that its edit has been staged as a proposal.

Summary
- The LLM does not receive sufficient, explicit feedback in the next request that the edit was staged and user action is now required. We append the tool result JSON (with counts and flags) as a `role: "tool"` message, but we do not include the richer “staged proposal” summary or a directive to stop. We also keep `tool_choice = auto`, allowing further tool calls, and a “remember to stop” System message is added to chat history but not included in the subsequent API request. These combine to encourage continued tool calls.

What happens today (relevant paths)
- Tool-call dispatch and results
  - `tools/mod.rs::process_tool` routes incoming tool calls.
  - For `apply_code_edit`:
    - `tools/code_edit.rs::GatCodeEdit::execute` builds a typed request and calls `rag::tools::apply_code_edit_tool`, then returns `ToolResult { content: ApplyCodeEditResult as JSON }`.
    - `rag/tools.rs::apply_code_edit_tool` stages the proposal (diff/preview; registry) and emits:
      - A SysInfo chat message summarizing staged files and approval commands (Approve/Deny).
      - `SystemEvent::ToolCallCompleted { content: ApplyCodeEditResult as JSON }`.
  - After `execute` returns, `process_tool` also emits `ToolCallCompleted` with the same content. This likely duplicates the completion event for the same tool call.

- Session loop behavior
  - `llm/manager/session.rs::RequestSession::run`:
    - Parses provider response → if `tool_calls`, it:
      - Updates the initial assistant message with e.g. "Calling Tools".
      - Awaits `SystemEvent::ToolCallCompleted` per call.
      - For each completion, appends a `role: "tool"` message to the next request: `RequestMessage::new_tool(content, call_id)`.
      - If `finish_reason == ToolCalls`, it posts a local System chat message: "Tool Call completed. Remember to end with a 'stop' finish reason..." and continues the loop.
    - Critically, the System reminder is added to chat history only; the request payload for the next API call is not rebuilt from chat history. The request uses the original `self.req.core.messages` plus appended tool messages only. The new System reminder is not included in the next request to the model.
    - The loop keeps `use_tools = true` and `tool_choice = Auto` for subsequent calls, so the model is allowed (and likely) to keep calling tools.

- Prompt construction and what the model actually sees
  - Initial request messages: `rag/context.rs::process_with_rag` builds messages as [System (headers + code context), User, Assistant, ...], then sends `ChatEvt::PromptConstructed`.
  - Subsequent requests (after tool calls) do not reconstruct messages from chat history; only `role: "tool"` messages containing the tool JSON are appended in `session.rs`. Any SysInfo/System messages added during tool execution are not included unless they were part of the original prompt.
  - Tool result content for edits is the typed `ApplyCodeEditResult { ok, staged, applied, files, preview_mode, auto_confirmed }` with no `request_id` or explicit “awaiting user approval” directive.

Why the model keeps editing
- No “stop” instruction in the next request: the local System reminder is not included in the request payload. The model never sees it.
- Tools remain enabled: `tool_choice` remains `auto` after tool completion, so the model is free to keep calling tools.
- Tool result lacks actionable status cues: The JSON confirms staging but does not include the proposal request_id, nor a clear directive that human approval is required before further edits.
- SysInfo summary omitted from model input: The detailed SysInfo message (files preview, approve/deny commands) is added to chat history but filtered out (SysInfo is not included) in payload construction. It is also not explicitly injected into the in-flight request payload within `session.rs`.

Additional issue
- Duplicate `ToolCallCompleted` emission for `apply_code_edit`:
  - `rag/tools.rs::apply_code_edit_tool` emits ToolCallCompleted.
  - `tools/mod.rs::process_tool` calls `GatCodeEdit::emit_completed` again.
  - This can lead to extra events on the realtime channel. The session loop consumes only one completion per tool call, but duplication is a correctness risk and may confuse other subscribers.

Evidence (key code references)
- Tool completion emission:
  - `crates/ploke-tui/src/rag/tools.rs` (see: ToolCallCompleted emitted after staging; SysInfo summary added)
  - `crates/ploke-tui/src/tools/code_edit.rs` (GAT wrapper returning ToolResult)
  - `crates/ploke-tui/src/tools/mod.rs::process_tool` (calls `emit_completed` after `execute`)
- Session and next-request assembly:
  - `crates/ploke-tui/src/llm/manager/session.rs` — collects tool results, appends `role: "tool"` messages, does NOT add the System reminder into `self.req.core.messages`, and leaves `tool_choice` as `auto`.
- Message selection for requests:
  - `crates/ploke-tui/src/chat_history.rs::current_path_as_llm_request_messages` filters SysInfo; Tool messages are included when present.
  - Subsequent request payload is not rebuilt from chat history between tool-call iterations; only the tool messages are appended.

Conclusion
- We are not giving the model the necessary, explicit feedback and gating. The model gets only a terse tool JSON result and is allowed to keep using tools. The System reminder to stop is not part of the actual API request. Result: models often continue issuing edits.

Recommended changes (targeted and minimal)
1) Gate tools after successful code edit
   - After collecting tool results for `apply_code_edit`, disable tools for the next API call: set `use_tools = false` and/or `self.req.tool_choice = Some(ToolChoice::None)`. This forces the model to produce a final assistant message instead of another tool call.
   - Location: `crates/ploke-tui/src/llm/manager/session.rs` (in the branch handling ParseOutcome::ToolCalls, after join_set completes and before continuing the loop).

2) Inject an explicit System instruction into the next request payload
   - Push a System message into `self.req.core.messages` acknowledging the result and desired behavior, e.g.:
     - "The code edit tool completed: edits have been staged as a proposal and are awaiting user approval. Do not call tools again for further edits unless asked. Return control to the user with a final answer."
   - Avoid relying on SysInfo; add a real System role message to the request payload right after appending the tool result.
   - Location: `crates/ploke-tui/src/llm/manager/session.rs` alongside the `new_tool(...)` pushes.

3) Include proposal identifiers in the tool result (optional but helpful)
   - Add `request_id: Uuid` to `ApplyCodeEditResult` (and `CreateFileResult`) so the model can reference the staged proposal deterministically. Keep strong typing consistent across crates.
   - Location: `ploke_core::rag_types::{ApplyCodeEditResult, CreateFileResult}` and tool emitters in `rag/tools.rs` and GAT wrappers.

4) Remove duplicate ToolCallCompleted for `apply_code_edit`
   - Choose a single emitter:
     - Option A: Let `rag/tools.rs::apply_code_edit_tool` be the sole source of `ToolCallCompleted`; remove `emit_completed` from the GAT wrapper path for this tool.
     - Option B: Have the GAT wrapper exclusively emit the completion; make `rag/tools.rs::apply_code_edit_tool` return its typed result without emitting.
   - Location: `crates/ploke-tui/src/tools/mod.rs` (remove the extra `emit_completed` for `ApplyCodeEdit`) or adjust `rag/tools.rs` accordingly.

5) Align stop semantics with finish_reason
   - If the prior finish reason was `tool_calls` and a code edit completed, prefer forcing a non-tool completion on the next turn (see #1) unless a policy/flag opts into multi-step chains.

Non-goals / out of scope for this pass
- Rewriting the broader agent loop or adding multi-tool planning. The above are surgical changes to provide clear feedback and prevent unnecessary repeated edits.

Verification (post-change)
- Add an e2e test (gated) exercising: user asks for a code edit → model issues one tool call → we respond with tool result + System instruction + `tool_choice = none` → model returns final assistant message (no further tool calls) → conversation control is back to user.
- Under live-gates discipline, record pass/fail summary and artifacts under `target/test-output/...`.

Appendix — File Pointers
- Tool flow: `crates/ploke-tui/src/tools/{mod.rs, code_edit.rs, create_file.rs}`
- Staging/proposals: `crates/ploke-tui/src/rag/tools.rs`
- Session loop: `crates/ploke-tui/src/llm/manager/session.rs`
- Chat filtering: `crates/ploke-tui/src/chat_history.rs`
- Tool choice enum: `crates/ploke-tui/src/llm/request/endpoint.rs`