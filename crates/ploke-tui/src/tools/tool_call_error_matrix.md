# Tool Call Error Coverage Matrix

Scope: ploke-tui tool calls as wired in `crates/ploke-tui/src/tools/mod.rs`.
Goal: track error coverage for tool invocation (deserialize/validation/exec), audience-aware payloads, and actionable retry guidance.

Legend:
- adapt_error: per-tool override of `Tool::adapt_error` (controls error classification + retry hints).
- retry_hint: explicit hint attached to `ToolError` (LLM recovery guidance).
- direct ToolCallFailed: tool emits `SystemEvent::ToolCallFailed` itself (may bypass Tool::emit_err or double-emit).
- ui/io helpers: uses `tool_ui_error` / `tool_io_error` to encode domain errors (converted to ToolError via adapt_error).

## Matrix

| Tool | File | Error sources (deserialize/validation/exec) | adapt_error override | retry_hint coverage | direct ToolCallFailed emission | Notes / gaps |
| --- | --- | --- | --- | --- | --- | --- |
| request_code_context | `crates/ploke-tui/src/tools/request_code_context.rs` | deserialize via `Tool::deserialize_params`; validation inside execute (parse failure, missing rag/search term); exec errors from rag | No | None | No | Errors are `tool_ui_error` or internal errors; no explicit retry hint. |
| apply_code_edit (GatCodeEdit) | `crates/ploke-tui/src/tools/code_edit.rs` + `crates/ploke-tui/src/rag/tools.rs` | deserialize via `Tool::deserialize_params`; validation in rag tool; exec errors from editing | No | None | Yes (rag utils ToolCallParams emits) | Custom ToolCallFailed in `rag/utils.rs`; potential double-emission if execute later returns Err. |
| create_file | `crates/ploke-tui/src/tools/create_file.rs` | deserialize via `Tool::deserialize_params`; validation in create_file_tool; exec errors from staging | No | None | Yes (CreateFileCtx emits) | Emits ToolCallFailed directly; then `execute` returns Internal error if proposal missing (double-emission risk). |
| list_dir | `crates/ploke-tui/src/tools/list_dir.rs` | deserialize; validation in execute; exec errors from fs | Yes | Yes (path + io hints) | No | Good retry hints for path format; handles missing dir as ok result. |
| ns_read | `crates/ploke-tui/src/tools/ns_read.rs` | deserialize; validation in execute; exec errors from IO manager | Yes | Yes (path + file/dir hint) | No | Has explicit hint text for path + file/dir. |
| ns_patch | `crates/ploke-tui/src/tools/ns_patch.rs` | deserialize; validation in execute; exec errors from patching | Yes | Yes (path hint) | No | Hint covers path format but not diff format issues. |
| code_item_lookup | `crates/ploke-tui/src/tools/code_item_lookup.rs` | deserialize; validation in execute (empty fields, node_kind, .rs, path) | No | None | No | Validation errors are user-facing strings but no retry_hint field; relies on message text. |
| get_code_edges | `crates/ploke-tui/src/tools/get_code_edges.rs` | deserialize; validation in execute (empty fields, node_kind, .rs, path) | No | None | No | Similar to code_item_lookup; no retry_hint. |
| cargo | `crates/ploke-tui/src/tools/cargo.rs` | deserialize; validation/exec; IO/process errors | No | None | No | Uses `tool_ui_error`/`tool_io_error`, but no `adapt_error` override for hints. |

## Observations

- The error infrastructure exists (`crates/ploke-tui/src/tools/error.rs`), but only 3 tools add retry hints via `adapt_error` overrides.
- `create_file` and `apply_code_edit` bypass the Tool trait emission by sending `ToolCallFailed` directly, which can cause duplicate failure events when the tool also returns an Err upstream.
- Several tools surface clear user-facing error strings, but do not provide structured retry guidance (`retry_hint`) for the model.

## Suggested follow-ups (for implementation phase)

- Normalize all tools to emit errors via `Tool::emit_err` and avoid direct `ToolCallFailed` except where strictly necessary.
- Add `adapt_error` overrides (or augment error generation) for tools lacking retry hints: request_code_context, code_edit, create_file, code_item_lookup, get_code_edges, cargo.
- Standardize retry guidance format per tool, including required fields and examples.
