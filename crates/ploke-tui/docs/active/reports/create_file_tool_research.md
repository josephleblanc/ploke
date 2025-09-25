# Create File Tool — Integration Research

Purpose
- Outline what’s required to add a strongly‑typed, safe “create file” tool that the LLM can call from the TUI, consistent with our tool system, IO discipline, and testing gates.

References
- Tool system (dispatch, schemas): `crates/ploke-tui/src/tools/`
- LLM wiring (tools exposure + tool loop):
  - `crates/ploke-tui/src/llm/manager/mod.rs` (tool list + ToolCallRequested dispatch)
  - `crates/ploke-tui/src/llm/manager/session.rs` (parsing tool_calls and coordinating tool results)
- Current tools: `request_code_context.rs`, `code_edit.rs`, `get_file_metadata.rs`
- Edit staging + approvals: `crates/ploke-tui/src/rag/tools.rs`, `crates/ploke-tui/src/rag/editing.rs`, `crates/ploke-tui/src/app_state/core.rs`
- IO manager (atomic writes, path policy): `crates/ploke-io/src/{actor.rs,handle.rs,write.rs}`
- Shared IO types: `crates/ploke-core/src/io_types.rs`

Engineering Guardrails (applied to this addition)
- Strong typing: Tool params/result must be strongly typed with `Serialize`/`Deserialize`; avoid ad‑hoc JSON.
- Safety-first editing: No direct writes from the tool; stage a proposal and apply via `IoManager`. For creation, enforce path policy and atomic write.
- Evidence-based: Add unit tests for schema/serde and execution stubs; add e2e (offline) where appropriate; gate live tests behind feature flags.

What Exists Today (tool path overview)
- Tools are defined via a static-dispatch trait `Tool` with GAT params: `crates/ploke-tui/src/tools/mod.rs`.
  - `ToolName` and `ToolDescr` enumerate and describe tools; `ToolDefinition` shapes the OpenRouter tool JSON.
  - `process_tool` matches `ToolName` and calls the concrete tool’s `deserialize_params` + `execute`, emitting `SystemEvent::ToolCallCompleted` or `ToolCallFailed`.
- LLM manager exposes tools to the API request: `llm/manager/mod.rs` (currently `RequestCodeContextGat`, `GatCodeEdit`, `GetFileMetadata`).
- Code edit tool stages edits into an `EditProposal` with a preview and requires approval to write via `IoManager`: see `rag/tools.rs` + `rag/editing.rs`.
- `IoManager` supports atomic snippet writes to existing files (verified against `expected_file_hash`), but has no “create file” request path yet.

Goal
- New tool: `create_file` (name TBD) that stages a file creation proposal (path + content) and, upon approval, atomically creates the file through `IoManager` with path-policy enforcement. Returns a typed result indicating staged/applied counts and target path.

Proposed API (Rust types + JSON schema)
- Tool name/descr (add to enums):
  - `ToolName::CreateFile` -> `"create_file"`
  - `ToolDescr::CreateFile` -> "Create a new file atomically within the workspace, staging for approval."
- Parameters (borrowed + owned variants per Tool trait pattern):
  - `file_path: String | Cow<'a, str>` — absolute or workspace‑relative path (we’ll resolve against `crate_focus` when relative).
  - `content: String | Cow<'a, str>` — full file content to write on creation.
  - `on_exists: enum { error, overwrite, append }` (default `error`). Strongly typed as `OnExists`.
  - `create_parents: bool` (default `false`) — allow creating missing parent directories.
- JSON schema (tool.parameters):
  - type: object; properties `{ file_path: string, content: string, on_exists: enum, create_parents: boolean }`; required `["file_path", "content"]`; defaults for others.
- Result (typed response to LLM), new struct in `ploke_core::rag_types`:
  - `CreateFileResult { ok: bool, file_path: String, exists: bool, applied: bool, preview_mode: String, auto_confirmed: bool }`

Integration Plan (by crate)

1) ploke-core
- Add types in `crates/ploke-core/src/io_types.rs`:
  - `CreateFileData { id: Uuid, name: String, file_path: PathBuf, content: String, namespace: Uuid, on_exists: OnExists, create_parents: bool }`
  - `CreateFileResult { new_file_hash: TrackingHash }` (returned by IO path).
  - `OnExists` enum with `Serialize`/`Deserialize` and snake_case names.
- Add LLM result type in `crates/ploke-core/src/rag_types.rs`:
  - `CreateFileResult` (LLM-facing): see Proposed API above; distinct from IO-level result to keep clean boundaries.

2) ploke-io
- Extend `IoRequest` in `crates/ploke-io/src/actor.rs` with `CreateFile { request: CreateFileData, responder: oneshot::Sender<Result<CreateFileResult, PlokeError>> }`.
- In `IoManager::handle_request`, add branch to handle `CreateFile`:
  - Normalize path against configured roots and symlink policy (use `normalize_against_roots[_with_policy]`).
  - If `create_parents`, create parent dirs with safe perms.
  - Existence policy: `error` → fail if file exists; `overwrite` → temp write + atomic rename; `append` → temp write of `existing + content` and atomic replace.
  - Compute `TrackingHash` from the final content (parse tokens for Rust files; for non-Rust, use a stable hash over bytes as fallback).
  - Return `CreateFileResult { new_file_hash }`.
  - If watcher feature enabled, emit `Created` or `Modified` accordingly.
- Add public `IoManagerHandle::create_file(request: CreateFileData)` API that sends the message and awaits the response (mirrors existing batch APIs).
- Tests:
  - Unit tests for creation: new file, `on_exists=error`, `on_exists=overwrite`, `append`, parent creation flag.
  - Path-policy tests to ensure cross-root writes are denied.

3) ploke-tui (tool definition + staging flow)
- Add `CreateFile` tool module: `crates/ploke-tui/src/tools/create_file.rs` implementing `Tool`.
  - Borrowed param struct, owned param struct, `Tool::schema()` static JSON schema, `Tool::execute()` logic.
- Update `crates/ploke-tui/src/tools/mod.rs`:
  - Add variant to `ToolName`/`ToolDescr`, `as_str()` mapping, and `process_tool` match arm invoking `create_file::CreateFile::execute`.
- Expose to LLM: add `CreateFile::tool_def()` into the tools list in `crates/ploke-tui/src/llm/manager/mod.rs`.
- Staging behavior:
  - Mirror code-edit flow by introducing a “creation proposal”. Options:
    1) Minimal change: add a new proposal type alongside `EditProposal` (e.g., `FileCreateProposal`) with `files`, preview (after-only), and status. Add approval path analogous to `approve_edits` that calls `IoManagerHandle::create_file` for each item.
    2) Generalized: refactor `EditProposal` to an enum `ChangeProposal { Edit(EditProposal), Create(FileCreateProposal) }` and update storage/handlers. Larger change, but unifies UX.
  - Recommended: start with Option (1) to minimize surface area; follow-on refactor can unify proposals once both paths are stable.
- Preview for creations:
  - Show “Before: <does not exist>” and “After: <content>” in code-block mode; in diff mode, fabricate a unified diff from empty → content.
- Apply path:
  - Add `approve_creations(state, event_bus, request_id)` in `crates/ploke-tui/src/rag/editing.rs` that sends `IoManagerHandle::create_file` requests, emits `ToolCallCompleted` with per-file results, persists proposals, and triggers a rescan like edits do.
- Tests:
  - Tool schema serialization round-trip (like existing tests for other tools).
  - Param parse + `into_owned()`.
  - Execution unit test using temp dir and a real `IoManagerHandle` to validate success/failure paths (no live network).

4) LLM wire/response handling
- No changes to the response loop are needed: the tool already emits `SystemEvent::ToolCallCompleted` with serialized typed content that is fed back as a tool message.

Safety & Policy Considerations
- Root containment: enforce via `IoManager` roots/symlink policy. Relative paths resolve against `crate_focus` (or current workspace dir if unset), then normalized in IO.
- Atomicity: use temp file + fsync + rename for both create/overwrite; best-effort fsync of parent dir.
- No direct disk writes from tool code: all writes go through `IoManager` only.
- Hash discipline: compute `TrackingHash` for the new file content to align with change detection and future edits.

Open Questions (request human input)
- OnExists policy defaults and allowed modes: is `append` needed in M1, or keep only `{error, overwrite}`?
  - USER: Only `{error, overwrite}`. We do not need `append` in M1. 
- Parent directory creation: allow with an explicit flag or always deny outside pre-created tree?
  - USER: Allow, but ensure created directory is within allowed scope of file access.
- Non-Rust files: should `TrackingHash` fallback be permitted, or should creation be limited (initially) to Rust files only?
  - USER: Rust files only. We are not going to handle non-Rust files yet.
- Should creations be auto-approved when `auto_confirm_edits=true`, matching edit behavior?
  - USER: Yes

Testing Plan (evidence discipline)
- Unit tests across crates as noted above.
- E2E (offline): simulate tool call → stage proposal → approve → verify file exists, `new_file_hash` present, and DB rescan executed.
- Live tests (`#[cfg(feature = "live_api_tests")]`): add matrix case invoking `create_file` on a temp workspace; assert tool_calls observed, proposal staged, approval applied, and file delta recorded under `target/test-output/...`.

Migration/UX Notes
- UI: approvals overlay should display creation previews with clear badges (Created) vs Edited.
- Docs: add tool description, schema, and examples to the TUI user docs; update the Agent Operating Guide references where tools are listed.

Minimal Task List
- ploke-core: add IO + LLM result types and `OnExists` enum.
- ploke-io: add `IoRequest::CreateFile`, handler, and `IoManagerHandle::create_file` API; tests.
- ploke-tui/tools: implement `CreateFile` tool module; register in `ToolName`/`process_tool`; schema + tests.
- ploke-tui/rag: add staging + approval path for creations; preview generation; tests.
- llm/manager: include tool in exposed list; builder tests updated accordingly.

Rollout Strategy
- Land IO + core types first with unit tests.
- Add tool module and stage-only flow (behind feature flag if desired), then wire approvals.
- Add e2e offline tests; finally, add live tests behind gate.

Appendix: Example Tool Schema (JSON)
```json
{
  "type": "object",
  "properties": {
    "file_path": { "type": "string", "description": "Absolute or workspace-relative path." },
    "content": { "type": "string", "description": "Full file content." },
    "on_exists": { "type": "string", "enum": ["error", "overwrite", "append"], "default": "error" },
    "create_parents": { "type": "boolean", "default": false }
  },
  "required": ["file_path", "content"],
  "additionalProperties": false
}
```
