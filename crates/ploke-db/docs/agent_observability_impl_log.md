# ploke-db Observability + Edit Proposals Implementation Log

Date: 2025-08-20

Scope
- Implements M0/M1 surfaces described in crates/ploke-tui/docs/feature/agent-system/ploke_db_requests.md.
- Adds time-travel relations and API methods in ploke-db to unblock ploke-tui development.

What was added
- Relations (created on-demand, idempotent):
  - conversation_turn (time-travel)
  - tool_call (time-travel)
  - code_edit_proposal (time-travel; includes results_json for apply outcome)
- APIs on ObservabilityStore:
  - Conversation: upsert_conversation_turn, list_conversation_since
  - Tool calls: record_tool_call_requested, record_tool_call_done, get_tool_call, list_tool_calls_by_parent
  - Edit proposals: record_edit_proposed, record_edit_decision, record_edit_applied, get_edit_proposal
- JSON handling:
  - Store JSON as Cozo Json and return as JSON strings via dump_json for a text-oriented API.

Behavioral notes
- Idempotency:
  - record_tool_call_requested: no-op if an identical requested state exists or call is already terminal.
  - record_tool_call_done: no-op if identical payload is already recorded; rejects terminal status changes.
- Timestamps:
  - Conversation turn and decisions use Cozo 'ASSERT' to derive a consistent transaction timestamp (decided_at_ms).
  - record_edit_applied accepts applied_at_ms from caller to reflect external timing.
- Status values:
  - Tool calls: "completed" | "failed"
  - Edit proposals: "proposed" | "approved" | "denied" | "applied"

Technical debt / follow-ups
- Align schema vs. spec: code_edit_proposal includes results_json (Json?) to capture apply results; the reference doc did not list it. Either:
  - Update the spec to include results_json, or
  - Move results into a separate relation if needed.
- Redaction policy:
  - The spec mentions redaction toggles. Current implementation accepts pre-redacted JSON but does not enforce redaction. ploke-io should manage redaction before storing, or we add a crate-level toggle.
- Stronger idempotency for edit proposals:
  - record_edit_proposed currently asserts a new state whenever called; consider checking for identical existing proposals and making it a no-op.
- Schema migrations:
  - ensure_observability_schema() best-effort creates relations each call. Consider a startup-time schema migration step for clearer errors and performance.
- Validation:
  - record_edit_applied unconditionally sets status="applied". Consider enforcing that only "approved" proposals can transition to "applied".

Acceptance tests (suggested)
- Tool calls:
  - requested -> done twice => second is a no-op; status unchanged.
  - Historical queries using @ 'NOW' and a past timestamp reflect prior state.
- Edit proposals:
  - proposed -> approved -> applied; verify decided_at_ms derived from ASSERT, applied_at_ms preserved from caller.
  - approved -> denied should be rejected.
- JSON roundtrip:
  - arguments_json/outcome_json/diffs_json/results_json store/retrieve correctly; dump_json returns valid strings.
