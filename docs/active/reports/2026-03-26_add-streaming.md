## Overall assessment

Adding real streaming will be a **behavioral contract change** across both crates, not just a UI toggle. In `ploke-llm`, the request type already includes `stream`, and the response types already model `chat.completion.chunk` deltas—but the current execution path (`chat_step` + `parse_chat_outcome`) **explicitly rejects streaming deltas**. So “turning on streaming” today would fail, not degrade gracefully.

On the `ploke-tui` side, the architecture is *partially* ready: chat state updates support `append_content`, and the assistant placeholder is created with `MessageStatus::Generating`. However, the current chat loop updates the assistant message with `MessageStatus::Completed` and full replacement content once per step, and the state machine makes `Completed` messages effectively immutable. That means streaming requires changes to the **state transitions and update strategy**, not only wiring.

**Risk level: high**, mainly due to (1) implementing SSE parsing + delta assembly in `ploke-llm`, (2) correctness of tool-call streaming (arguments often arrive in fragments), and (3) preventing UI/state-machine invariants from being violated during frequent partial updates.

## Notable touchpoints (impact / hazards)

1. **`ploke-llm` currently cannot handle streaming deltas**
   - Location: `crates/ploke-llm/src/manager/session.rs`
   - Severity: critical  
   - Comment: `chat_step` reads the entire response via `resp.text().await` and then calls `parse_chat_outcome(&body)`. `parse_chat_outcome` explicitly errors if any choice contains `delta` (“Unexpected streaming delta in non-streaming parser”). Streaming therefore needs a separate streaming-capable codepath and parser; otherwise enabling `stream: true` is a correctness regression.

2. **OpenRouter streaming is SSE, not raw JSON**
   - Location: `crates/ploke-tui/docs/archive/openrouter/streaming.md` (design reference)
   - Severity: high  
   - Comment: The SSE stream includes `data: ...` lines, termination via `data: [DONE]`, and may include comment payloads (e.g. `: OPENROUTER PROCESSING`). A Rust streaming parser must be robust to chunk boundaries where JSON objects don’t align with TCP/HTTP byte frames.

3. **Tool-call streaming assembly is likely the hardest correctness edge case**
   - Location: `crates/ploke-llm/src/response/mod.rs` (`StreamingDelta.tool_calls`) + `crates/ploke-llm/src/response/tool_call.rs`
   - Severity: critical  
   - Comment: For tool calls, streamed deltas commonly deliver `tool_calls[].function.arguments` in multiple fragments. Your `ToolCall` model stores `arguments: String`, but the streaming parser will need to **concatenate arguments per `call_id`** until the final tool-call chunk (and possibly across multiple tool calls). If you assemble tool calls incorrectly, tool execution will fail downstream (bad JSON args) even if the UI “streams” correctly.

4. **`ploke-tui` state-machine forbids updating `Completed` messages**
   - Location: `crates/ploke-tui/src/chat_history.rs` (`MessageUpdate.validate`)
   - Severity: critical  
   - Comment: `MessageStatus::Completed` is terminal for content updates: once completed, updates with `content` or `append_content` are rejected as immutable. Streaming must keep the assistant message in `Generating` until the final chunk, and only then transition to `Completed`.

5. **TUI chat loop currently finalizes messages too early for streaming**
   - Location: `crates/ploke-tui/src/llm/manager/session.rs` (`run_chat_session`, `add_or_update_assistant_message`)
   - Severity: high  
   - Comment: The current flow updates placeholder content using `MessageStatus::Completed` and uses full message content from `chat_step`. Streaming will need to:
     - update via `append_content` while still `Generating`
     - avoid replacing “Pending…” incorrectly
     - transition to `Completed` only when the assembled final content is known
     - ensure tool-call sessions don’t accidentally append partial text after deciding to run tools

6. **TUI already defines a streaming event type, but it’s unused**
   - Location: `crates/ploke-tui/src/llm/manager/events.rs` (`ChatEvt::PartialResponse`)
   - Severity: medium  
   - Comment: There’s a `PartialResponse` variant, but nothing appears to emit/consume it today. Streaming integration is likely to either:
     - use that event path, or
     - bypass it and drive `StateCommand::UpdateMessage` directly with `append_content`.
   - Either choice is fine, but it affects API/architecture consistency.

7. **Cancellation behavior must stop the active HTTP stream**
   - Location: `crates/ploke-tui/src/llm/manager/session.rs` (`tokio::select!` around `ploke_llm::chat_step` and `wait_for_cancel_signal`)
   - Severity: high  
   - Comment: Today, cancellation interrupts between steps because `chat_step` is a single future. With streaming, cancellation must promptly terminate the stream read loop so:
     - no more deltas are processed after cancel
     - you don’t queue a backlog of `UpdateMessage` commands
     - you don’t accidentally leak tasks if the streaming read loop is spawned separately

8. **Performance / backpressure risk from high-frequency UI updates**
   - Location: `crates/ploke-tui/src/app_state/handlers/chat.rs` (`update_message` acquires a write lock and sends events)
   - Severity: medium  
   - Comment: If streaming emits a delta per token, the TUI could generate many state updates. This can cause lock contention and UI lag unless you throttle/batch deltas (e.g., append every N ms or N characters).

## Tests & verification (what should be added/updated)

### `ploke-llm` (required)
- **Unit test: SSE transcript → assembled content**
  - Scenario: feed a representative OpenRouter SSE stream containing multiple `data: { ... "delta": { "content": ... } }` chunks + final stop.
  - Assert: concatenated content matches expected; finish_reason normalized correctly.

- **Unit test: streaming tool-call argument concatenation**
  - Scenario: stream deltas with `tool_calls` fragments (same `call_id`) where `function.arguments` is split across several chunks.
  - Assert: assembled tool-call arguments JSON parses successfully and matches expected final arguments string; tool call execution trigger occurs only at the correct time.

- **Unit test: ignore SSE comments / handle `[DONE]`**
  - Scenario: include `: OPENROUTER PROCESSING` style comments and ensure they don’t break JSON parsing.
  - Assert: parser skips comments and stops at `[DONE]`.

- **Unit test: embedded/stream error handling**
  - Scenario: stream includes `{"error": ...}` in a `data:` payload mid-stream.
  - Assert: returns `LlmError::Api` (or equivalent) and stops processing further deltas.

- **Integration test (mock HTTP/SSE server)**
  - Scenario: run `chat_step`-streaming against a local mock that emits SSE frames with intentional chunking boundaries.
  - Assert: no panics, no missed deltas, and cancellation closes cleanly.

### `ploke-tui` (required)
- **State transition test: Generating → Completed**
  - Scenario: simulate receiving multiple deltas; ensure message remains `Generating` while appending and becomes `Completed` exactly once at the end.
  - Assert: `MessageUpdate.validate` never rejects updates.

- **Cancel test: cancellation mid-stream**
  - Scenario: start streaming, then cancel; ensure no further `append_content` updates occur.
  - Assert: message ends in an expected final state (likely `Error { description: ... }` or an aborted outcome) and no extra events are emitted afterward.

- **Tool-call streaming test**
  - Scenario: streamed tool call is detected; ensure TUI stops rendering partial assistant text (or handles it deterministically) and runs tool execution with fully assembled args.

## Must-fix vs nice-to-have

### Must-fix before merge
- Implement a **streaming-capable path** in `ploke-llm` (SSE parsing + delta assembly); current `chat_step`/`parse_chat_outcome` will reject deltas.
- Update `ploke-tui` to keep assistant messages in `MessageStatus::Generating` during partial updates and only transition to `Completed` at the end (to satisfy `MessageUpdate` immutability rules).
- Correctly assemble **streamed tool-call arguments** per `call_id` before tool execution.
- Ensure **cancellation** aborts the active streaming read promptly and prevents post-cancel state updates.
- Add the tests listed above (especially tool-call argument concatenation and cancel-mid-stream).

### Nice-to-have
- Throttle/batch `append_content` updates to reduce UI lock contention and keep rendering smooth.
- Wire `ChatEvt::PartialResponse` end-to-end (or remove/repurpose it) for architectural clarity.
- Populate streaming performance metrics (`time_to_first_token`, tokens/sec) into `LLMMetadata` once you can measure them from the delta stream.