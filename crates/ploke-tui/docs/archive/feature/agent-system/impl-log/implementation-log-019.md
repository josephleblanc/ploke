# Implementation log 019 — M1 core wiring verified; next: decisions_required updates (2025-08-20)

Summary
- Verified Milestone 1 core flow is implemented in code:
  - apply_code_edit is staged (not immediately applied) with preview and idempotency in handlers/rag.rs.
  - Approve/Deny flows are implemented via io_handle.write_snippets_batch and SysInfo reporting.
  - Runtime editing controls are wired end-to-end:
    - Commands: parser + exec support for edit preview mode/lines/auto.
    - Dispatcher updates config and emits confirmations.
  - LLM tools: request_code_context and apply_code_edit registered; typed LlmTool bridge present; SystemEvent path retained for M0 back-compat.

What changed in this step
- No code changes required; this log documents the verification and sets up the next doc updates.

Requested file to proceed (please add to chat so we can edit it next):
- crates/ploke-tui/docs/feature/agent-system/decisions_required.md

Planned edits after the file is available
- Append new decision items:
  - 20) Default max preview lines per file (recommend default 300; adjustable at runtime).
  - 21) Path normalization scope for previews/SysInfo (recommend workspace-root-relative).
- These align with m1_granular_plan and current implementation behavior.

Notes
- Unified diff preview supported via the "similar" crate; code-block remains default per config.
- Tool event telemetry is logged; DB persistence integration will follow ploke-db ObservabilityStore readiness (tracked separately).

Next steps
- Update decisions_required.md with items 20 and 21 once provided.
- Consider small polish: summarize staged file list more compactly for very large edit sets; add optional per-file diff toggles in future UI work.
