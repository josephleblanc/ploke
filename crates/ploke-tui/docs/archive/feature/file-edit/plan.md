Goal
- Allow the LLM to propose concrete code edits (splices) that are applied by ploke-io with atomic writes, then report success/failure back via tool-call completion events.

Approach
- Add a new tool: apply_code_edit
- When the model returns this tool call, parse its arguments into ploke_core::WriteSnippetData values and call IoManagerHandle::write_snippets_batch.
- Map results to ToolCallCompleted/ToolCallFailed events.
- Keep everything within existing subsystems; no new actors required.

Approval / Denial step (optional)
- Add a gated approval phase before applying edits.
  - Compute a per-edit preview (e.g., unified diff) from current on-disk content vs. proposed splice.
  - Decide auto-apply vs. require approval based on:
    1) Global config flag: editing.auto_confirm_edits (bool, default false).
    2) Autonomous agent mode: editing.agent.enabled (bool) with editing.agent.min_confidence (0.0–1.0).
       - If agent mode is enabled and the tool call provides an optional "confidence" field, auto-apply only when confidence >= min_confidence.
       - If confidence is absent, treat as 0.0 (no auto-apply) unless overridden by policy.
  - If auto-apply conditions are NOT met, emit a "pending approval" event with a request_id and the diff preview(s); wait for explicit Approve/Deny commands from the user.
  - On Approve: apply edits atomically via ploke-io; on Deny or timeout: drop the proposal and report back (ToolCallFailed or a structured rejection event).

Files to touch (minimal)
- crates/ploke-tui/src/llm/mod.rs
  - Define a tool schema function apply_code_edit_tool_def() returning ToolDefinition for the new tool.
  - Include this tool alongside request_code_context_tool_def() in the tools vector used by RequestSession.
  - Optionally accept a top-level numeric "confidence" argument (0.0–1.0) from the tool call payload for the approval gate.

- crates/ploke-tui/src/app_state/handlers/rag.rs
  - Extend handle_tool_call_requested to support name == "apply_code_edit".
  - Parse arguments JSON payload into one or more WriteSnippetData (ploke_core::io_types::WriteSnippetData).
  - Before applying, build per-edit previews (diffs) for user review.
  - Approval gate:
    - Read config.editing.auto_confirm_edits and config.editing.agent settings from AppState.
    - If auto-confirm is true OR (agent.enabled && confidence >= agent.min_confidence): apply immediately.
    - Else, emit a "pending approval" event with request_id, optional confidence, and diff previews; store the pending set in AppState for later decision.
  - On approval: invoke state.io_handle.write_snippets_batch(edits).await and aggregate results; emit ToolCallCompleted with results JSON.
  - On denial (or timeout): emit ToolCallFailed with a descriptive message and clear pending state.

- crates/ploke-tui/src/user_config.rs
  - Add EditingConfig:
    - editing.auto_confirm_edits: bool (default: false)
    - editing.agent.enabled: bool (default: false)
    - editing.agent.min_confidence: f32 (default: 0.8)

- crates/ploke-tui/src/app_state/core.rs
  - Thread EditingConfig into AppState for read access in handlers.
  - Add pending edit proposals storage: e.g., RwLock<HashMap<Uuid, PendingEdits>>.

- crates/ploke-tui/src/app_state/commands.rs
  - Add:
    - ApproveEdits { request_id: Uuid }
    - DenyEdits { request_id: Uuid }

- crates/ploke-tui/src/app_state/dispatcher.rs
  - Route ApproveEdits/DenyEdits to rag.rs to finalize or discard pending edits.

- crates/ploke-tui/src/app/commands/parser.rs and crates/ploke-tui/src/app/commands/exec.rs
  - Add user commands:
    - "edit approve <request_id>"
    - "edit deny <request_id>"
  - Parse and dispatch to StateCommand::{ApproveEdits, DenyEdits}.

- crates/ploke-tui/src/app/events.rs (minimal UI surface)
  - When a proposal is pending, add a SysInfo message summarizing files and short diff excerpts.

- crates/ploke-tui/src/lib.rs (events)
  - Optional: add structured SystemEvent variants for the flow:
    - FileEditProposed { request_id, confidence: Option<f32>, diffs: Vec<DiffPreview> }
    - FileEditApplied { request_id, results_json }
    - FileEditRejected { request_id, reason }
  - Minimal path can reuse existing ToolCallCompleted/Failed + SysInfo messages.

Notes
- Agent mode is not implemented yet; only config toggles and min_confidence gate are read.
- The "confidence" value is optional and originates from the model/tool response; when omitted, treat as 0.0 (require approval) unless auto_confirm_edits=true.

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
  - confidence: number (0.0–1.0) Optional confidence for approval gate; if omitted, treat as 0.0.
- required: ["edits"]
- Example:
  {
    "confidence": 0.92,
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
