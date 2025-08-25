# Implementation log 021 — Introduce get_file_metadata tool and refine prompt (2025-08-21)

Summary
- Added a lightweight tool get_file_metadata to fetch the current file tracking hash (UUID) and basic metadata.
- Updated PROMPT_CODE to instruct the model to use get_file_metadata when expected_file_hash is missing before calling apply_code_edit.
- Kept request_code_context and apply_code_edit unchanged; expanded examples and guidance to reduce malformed edit attempts.

Changes
- crates/ploke-tui/src/llm/mod.rs
  - Added get_file_metadata_tool_def() and included it in the tool list for requests.
- crates/ploke-tui/src/app_state/handlers/rag.rs
  - Implemented handling for get_file_metadata tool calls: reads file, computes a deterministic v5 UUID over file bytes using PROJECT_NAMESPACE_UUID, returns JSON with file_hash/tracking_hash, size, and mtime.
  - Rewrote PROMPT_CODE sections to document when/how to use get_file_metadata and updated examples accordingly.

Rationale
- Many edit failures stem from missing or stale expected_file_hash values. Providing an explicit metadata tool clarifies the path for the model and improves success rate for apply_code_edit.
- Computing a v5 UUID over file bytes within the project namespace provides a stable identifier during development; we may align this with ploke-io’s hashing exactly in a follow-up if needed.

Next steps
- Validate end-to-end tool-call loop with a tool-capable provider and confirm that apply_code_edit succeeds when preceded by get_file_metadata.
- Consider adding snapshots asserting the presence and shape of get_file_metadata in outgoing tool definitions and the JSON payload schema used by the tool.
- Align tracking hash computation with the canonical implementation in ploke-io (if different), and optionally surface workspace-relative paths in metadata for UX.

Notes
- Observability and session await logic already treat SystemEvent::ToolCallCompleted as sufficient; no additional plumbing required.
