# Implementation log 016 — Lifecycle guards, JSON validation, typed tool outcomes (2025-08-20)

Summary
- Continued M1 implementation per granular plan with an accelerated cadence.
- Added lifecycle/idempotency guards and JSON validation to apply_code_edit staging.
- Emitted typed tool events alongside the compatibility SystemEvent bridge for terminal statuses.
- Documented spec alignment notes from ploke-db update.

Changes
- Staging guard:
  - Ignore duplicate apply_code_edit requests for the same request_id; emit SysInfo + ToolCallFailed (bridge) and LlmTool::Failed (typed).
- JSON validation:
  - Validate edits array is present and non-empty.
  - Validate each edit’s byte range (end_byte >= start_byte).
  - Detect and reject overlapping edit ranges per file before preview/apply.
- Typed events:
  - On approval success: emit both SystemEvent::ToolCallCompleted and LlmTool::Completed with results_json in content.
  - On approval failure or denial: emit both SystemEvent::ToolCallFailed and LlmTool::Failed.
  - Also emit typed failures for unsupported tool name, invalid token_budget, empty query, RAG errors, and RAG unavailability.
- Preview path:
  - No change in output format; validation errors are summarized in SysInfo and terminate the staging flow early.

ploke-db update alignment (2025-08-20)
- Status: Essential schema and API endpoints are in place and functional; M1 is close.
- To reach production-ready: add lifecycle guards, idempotency, JSON validation, and targeted acceptance tests.
  - This PR addresses guards/idempotency/validation in TUI; DB-side tests/guards remain a follow-up.
- Spec alignment: either update the spec to include results_json or move apply results to another relation.
  - Current behavior: completed events include a results_json payload in the content field; we will adapt API calls once ploke-db exposes code_edit_proposal/outcome endpoints.

Next steps
- Wire proposal persistence to ploke-db’s new APIs when available (code_edit_proposal/outcome), keeping in-memory as source of truth until then.
- Add acceptance tests for:
  - Duplicate staging ignored.
  - Invalid ranges and overlap detection produce failures and no staging.
  - Typed Completed/Failed emitted exactly once in tandem with SystemEvent bridge.
- Optional: extend user-config to toggle validation strictness.

Risks/notes
- Range validation uses byte offsets; callers must ensure UTF-8 boundaries if they intend textual edits.
- Overlap detection is strict; overlapping edits must be merged by the tool author before submission.

Verification
- cargo test -p ploke-tui (existing tests should remain green).
- Exercise apply_code_edit tool with overlapping and invalid ranges to see immediate SysInfo and failure events.
