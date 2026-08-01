# Tool Output Truncation Investigation

Date: 2026-04-21

## Scope

Question investigated: during eval runs, are tool outputs getting overly truncated before they reach the LLM, especially for `read_file`/`ns_read` and similar tools?

I traced the path:

1. tool execution
2. stored tool result / event artifacts
3. normalized or inspector-facing payloads
4. model-facing request body

I used the eval CLI first, then checked run artifacts/logs, then confirmed the code paths.

## Short Answer

For `read_file` specifically, there are two different truncation mechanisms and they are easy to confuse:

- Real upstream truncation can happen inside the tool itself. `ns_read` defaults to a 32 KiB byte cap and marks the result with `truncated: true` when the returned content is clipped. That truncation would propagate all the way into storage and into the next LLM request. See [ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs:19), [ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs:186), and [actor.rs](/home/brasides/code/ploke/crates/ploke-io/src/actor.rs:388).
- Separate inspector/UI truncation also happens later. The eval CLI inspector pretty-prints payloads and then truncates the display only, explicitly annotating that the stored raw payload is larger. See [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:8703) and [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:9075).

For the sampled eval run `tokio-rs__tracing-1015`, the `read_file` payload I inspected was **not** truncated upstream before reaching the LLM:

- CLI inspection showed `ui.truncated: false` and a large stored raw payload.
- The event trace stored the full `ToolCompleted.content`.
- The API request log showed the next `role: "tool"` message containing the full JSON payload.

So for that run, the “too short” appearance was inspector/log/UI preview truncation, not pre-LLM truncation.

## CLI Inspection First

I ran:

```bash
cargo run -q -p ploke-eval -- inspect tool-calls --instance tokio-rs__tracing-1015 4
cargo run -q -p ploke-eval -- inspect tool-calls --instance tokio-rs__tracing-1015 4 --full
```

Observed result:

- The CLI reported a `read_file` call with `ui.truncated: false`.
- The non-`--full` view printed:
  `Inspector display truncated the normalized payload: 217/7601 source chars shown (+ ellipsis), 7384 elided; stored raw payload is 7401 bytes.`
- The `--full` view still truncated for display, but at a much larger threshold, and again printed an explicit inspector-only truncation note.

That behavior matches the inspector implementation in [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:8723), [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:9093), and [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:9122).

## Pipeline Trace

### 1. Tool execution

`read_file` in the eval agent is the TUI `NsRead` tool:

- It applies a default byte cap of 32 KiB when `max_bytes` is omitted: [ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs:19)
- It always sends `max_bytes: Some(byte_cap)` into `IoManager`: [ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs:186)
- It serializes the full `NsReadResult` into `ToolResult.content`: [ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs:232)

The underlying IO layer is where real clipping happens:

- `read_plain_file` reads the whole file, records original `byte_len`, then truncates content to `max_bytes`: [actor.rs](/home/brasides/code/ploke/crates/ploke-io/src/actor.rs:389)
- `truncate_to_limit` clips UTF-8 safely and returns `truncated = true`: [actor.rs](/home/brasides/code/ploke/crates/ploke-io/src/actor.rs:488)

Implication:

- If a file exceeds the requested/default cap, upstream truncation is real and intentional.
- The tool emits enough metadata to prove it: `byte_len`, `content`, and `truncated`.

### 2. Stored tool result

After tool execution, the exact tool JSON result is appended to the in-memory request and also sent through state/event plumbing:

- `req.core.messages.push(RequestMessage::new_tool(tool_result.content.clone(), ...))`: [session.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/session.rs:867)
- The same content is sent into `StateCommand::AddMessageTool { msg: tool_result.content, ... }`: [session.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/session.rs:876)

The wire-level request message type is a literal `role: tool` message with raw `content` plus `tool_call_id`: [manager/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/mod.rs:43) and [manager/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/mod.rs:94).

Eval persistence reconstructs tool calls from observed events without summarizing them:

- `ToolCompleted` / `ToolFailed` are copied into `ToolExecutionRecord`: [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:508)
- `record.json.gz` is written by serializing the whole `RunRecord` directly: [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1529)

### 3. Artifact evidence: full content vs preview content

In the sampled run, the event trace shows both the full payload and the preview payload side by side:

- Full raw tool result in `ToolCompleted.content`: [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/agent-turn-trace.json:1107)
- Full tool message passed through `AddMessageTool { msg: ... }`: [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/agent-turn-trace.json:1075)
- Truncated UI/event preview in `MessageUpdated.content_preview`: [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/agent-turn-trace.json:1141)

This is the first key distinction:

- `ToolCompleted.content` and `AddMessageTool.msg` are upstream/raw.
- `MessageUpdated.content_preview` is observer/UI-only.

The same pattern is visible for tool failures: full JSON in `AddMessageTool.msg`, shorter `content_preview` for display in [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/agent-turn-trace.json:1461).

### 4. Model-facing request

`ploke-llm::chat_step` serializes the request and logs the exact request JSON before POSTing it:

- `serde_json::to_string_pretty(req)` is computed: [session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:105)
- That exact request body is logged via `log_api_request_json`: [session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:110)
- `log_api_request_json` writes the request payload verbatim to the `api_json` log target: [session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:320)

The run log confirms the provider-bound request carried the full tool JSON for the sampled `read_file` result:

- `role: "tool"` request entry: [ploke_eval_20260417_094500_831901.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log:348331)
- The content includes the full serialized `read_file` payload with `truncated:false`: [ploke_eval_20260417_094500_831901.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log:348332)

That is the decisive evidence that, for this run, the `read_file` output was not being shortened between storage and the model-facing request.

## Where Truncation Is Real vs Cosmetic

### Cosmetic / inspector-only

1. `ploke-eval inspect tool-calls`
   - Pretty-prints JSON and truncates the rendered display only.
   - It explicitly tells you that the inspector elided content while the stored raw payload is larger.
   - Code: [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:8727), [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:9085), [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:9122)

2. UI/event previews
   - `MessageUpdated.content_preview` in trace/log artifacts is truncated for display.
   - Artifact evidence: [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/agent-turn-trace.json:1141)

3. Optional token logging in the TUI manager
   - If enabled, `Outgoing chat request (truncated when large)` uses `truncate_for_tokens_log`, which is logging-only.
   - Code: [session.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/session.rs:650)

### Real / upstream

1. `read_file` / `ns_read`
   - Default 32 KiB cap unless caller overrides `max_bytes`.
   - Raw returned JSON exposes `truncated`.
   - Code: [ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs:19), [ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs:186), [actor.rs](/home/brasides/code/ploke/crates/ploke-io/src/actor.rs:488)

2. `cargo`
   - Explicit response-size enforcement drops rendered diagnostics / tails / entries until the result fits.
   - It records `raw_messages_truncated`.
   - Code: [cargo.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/cargo.rs:663) and [cargo.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/cargo.rs:1070)

## Conclusion

For the inspected eval run, I do **not** see evidence that `read_file` outputs were being overly truncated before they reached the LLM. The raw tool result survived intact through:

- tool execution
- event storage
- eval record storage
- tool-message construction
- provider-bound request serialization

What does get shortened later is:

- eval CLI inspector rendering
- UI/event `content_preview` fields
- optional token/request logs

The real upstream truncation risk for `read_file` is the tool’s own default 32 KiB cap. If users are seeing unexpectedly short `read_file` content in evals, the first thing to check is whether the payload itself has `truncated: true`. If it does not, the likely culprit is inspector/display truncation rather than loss before the model request.

## Practical Diagnostic Rule

To distinguish the layers quickly in future runs:

1. Inspect the stored raw tool payload.
   - If `truncated: true`, the tool clipped it upstream.
2. Compare `ToolCompleted.content` / `AddMessageTool.msg` against `MessageUpdated.content_preview`.
   - If only the preview is short, that is UI/log truncation.
3. Check the `api_json` request log.
   - If the `role: "tool"` request entry still has the full JSON, the model saw it.

Useful artifacts for that workflow:

- [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/agent-turn-trace.json)
- [record.json.gz](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/record.json.gz)
- [ploke_eval_20260417_094500_831901.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log)
