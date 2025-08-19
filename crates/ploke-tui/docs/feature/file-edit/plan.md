Goal
- Allow the LLM to propose concrete code edits (splices) that are applied by ploke-io with atomic writes, then report success/failure back via tool-call completion events.

Approach
- Add a new tool: apply_code_edit
- When the model returns this tool call, parse its arguments into ploke_core::WriteSnippetData values and call IoManagerHandle::write_snippets_batch.
- Map results to ToolCallCompleted/ToolCallFailed events.
- Keep everything within existing subsystems; no new actors required.

Files to touch (minimal)
- crates/ploke-tui/src/llm/mod.rs
  - Define a tool schema function apply_code_edit_tool_def() returning ToolDefinition for the new tool.
  - Include this tool alongside request_code_context_tool_def() in the tools vector used by RequestSession.

- crates/ploke-tui/src/app_state/handlers/rag.rs
  - Extend handle_tool_call_requested to support name == "apply_code_edit".
  - Parse arguments JSON payload into one or more WriteSnippetData (ploke_core::io_types::WriteSnippetData).
  - Invoke state.io_handle.write_snippets_batch(edits).await and aggregate results.
  - On success: send AppEvent::System(SystemEvent::ToolCallCompleted { content: json }) with details for each edit.
  - On failure: send ToolCallFailed with a descriptive error message.

No changes required (for a minimal path)
- EventBus, SystemEvent variants (you can keep using ToolCallRequested/Completed/Failed already present).
- FileManager (not needed for minimum viable implementation).
- ploke-core/ploke-io (already expose WriteSnippetData and write_snippets_batch).

Recommended argument schema (JSON) for apply_code_edit
- type: object
- properties:
  - edits: array of:
    - file_path: string (absolute path required by IoManager policy unless configured)
    - expected_file_hash: string (UUID or hex, aligning with TrackingHash string format)
    - start_byte: integer (inclusive)
    - end_byte: integer (exclusive)
    - replacement: string
  - namespace: string (UUID; can default to PROJECT_NAMESPACE_UUID on the IO side if missing)
- required: ["edits"]
- Example:
  {
    "edits": [
      {
        "file_path": "/abs/path/src/lib.rs",
        "expected_file_hash": "b1a9d1c8-8c2f-5b4e-b2e7-2a8a1d2c9f3e",
        "start_byte": 120,
        "end_byte": 156,
        "replacement": "pub fn new_name() {}"
      }
    ]
  }

Result payload suggestion
- On success (ToolCallCompleted content):
  {
    "ok": true,
    "applied": 1,
    "results": [
      {
        "file_path": "...",
        "new_file_hash": "..."
      }
    ]
  }
- On errors (ToolCallFailed content or per-edit error in results):
  {
    "ok": false,
    "error": "…"
  }

Implementation sketch (handlers/rag.rs)
- In handle_tool_call_requested:
  - if name == "apply_code_edit":
    - let edits_val = arguments.get("edits").and_then(|v| v.as_array()).ok_or(...)
    - Map into WriteSnippetData:
      - Parse file_path as PathBuf
      - Parse expected_file_hash into TrackingHash (if represented as a string UUID, convert accordingly)
      - Fill id/name/namespace with sensible defaults (e.g., new Uuid, “edit”, PROJECT_NAMESPACE_UUID)
    - Run state.io_handle.write_snippets_batch(vec).await
    - Build success JSON; send ToolCallCompleted
    - On error, send ToolCallFailed

Caveats
- Ensure file paths respect IoManager path policy (absolute paths; roots configured if used).
- Handle timeouts or long-running edits on a background task if applying many edits.
- Ensure ToolCallCompleted content is machine-readable JSON (not human prose).
- Consider emitting a SysInfo message for the user summarizing applied edits.

Minimal tests to add (optional but small)
- Unit test for the JSON-to-WriteSnippetData mapping (pure function).
- Integration-style test that:
  - Creates a temp file with content.
  - Emits a ToolCallRequested(apply_code_edit) with a small replacement.
  - Asserts a ToolCallCompleted event with ok:true and validates file content.

This plan reuses your existing tool-call flow and IoManager API, keeping the change surface tiny while enabling fully automated, provider-driven code edits.
