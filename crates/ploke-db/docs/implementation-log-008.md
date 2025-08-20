# Implementation Log 008 — ObservabilityStore (M0 unblock)

Date: 2025-08-19

Summary
- Introduced ObservabilityStore contract in ploke-db to unblock ploke-tui development for M0 observability.
- Implemented time-travel relations using Cozo Validity and Json types for:
  - conversation_turn(id: Uuid, at: Validity => parent_id: Uuid?, message_id: Uuid, kind: String, content: String, thread_id: Uuid?)
  - tool_call(request_id: Uuid, call_id: String, at: Validity => parent_id: Uuid, vendor: String, tool_name: String, args_sha256: String, arguments_json: Json?, status: String, ended_at_ms: Int?, latency_ms: Int?, outcome_json: Json?, error_kind: String?, error_msg: String?)
- Added minimal, idempotent APIs:
  - upsert_conversation_turn, list_conversation_since
  - record_tool_call_requested, record_tool_call_done
  - get_tool_call, list_tool_calls_by_parent

Files changed
- src/lib.rs: export new observability module and types.
- src/observability.rs: new module with types, trait, and Database implementation.
- docs/technical_debt.md: new doc to track follow-ups and debts.
- docs/implementation-log-008.md: this log.

Design notes
- Used 'ASSERT' for write timestamps to leverage Cozo's transaction-stable time for Validity.
- JSON fields are accepted as Option<String> and stored via parse_json(); when None, the literal "null" is parsed to Json null for consistency.
- Idempotency for record_tool_call_done performs a snapshot check '@ NOW' to avoid duplicate lifecycle rows when the payload is unchanged.

Next steps
- Add indexes to match audit/query patterns (by parent_id, by status, at DESC).
- Wire callbacks using Db::register_callback for live UI updates.
- Add tests around lifecycle, idempotency, and JSON round-trips.

References
- ploke-tui/docs/feature/agent-system/ploke_db_contract.md
- Cozo v0.7 time-travel and Json semantics (docs/dependency_details/cozo/types)
# Implementation Log 008 — Observability Store Refinements

Date: 2025-08-19

Summary
- Fixed a test failure caused by schema re-creation in `ensure_observability_schema`.
- Improved idempotency in schema initialization by tolerating more Cozo error messages.
- Preserved lifecycle and idempotency rules added for `tool_call` relation.

Details
- Problem: Running tests with `Database::init_with_schema()` pre-creates relations. Our `ensure_observability_schema()` attempted `:create` again and only ignored errors containing "exists"/"duplicate"/"already". Cozo returned "Stored relation conversation_turn conflicts with an existing one", which we did not ignore, causing failures.
- Change: Broadened the ignore set to include "conflicts with an existing one" and a generic lowercase "conflict" check. This makes `ensure_observability_schema()` idempotent across environments where the relation may already exist.

Type-safety notes and next steps
- Current API uses:
  - `ToolStatus` as a Rust enum with `as_str`/`from_str` to bridge to Cozo string values.
  - `arguments_json`/`outcome_json` as `Option<String>` wrapped JSON; coercion via `parse_json()`/`dump_json()` at the Cozo boundary.
- Future improvements (planned):
  - Introduce typed JSON payloads using `serde_json::Value` for arguments/outcome fields and convert at boundaries.
  - Introduce a `ConversationKind` enum for `ConversationTurn::kind` instead of plain `String`.
  - Add serde derives for types once the crate’s dependency set confirms availability across workspace.

Testing
- The updated logic ensures `ensure_observability_schema()` is safe to call even when schemas are already present.
- Lifecycle and idempotency rules remain tested in `tests/observability_tests.rs`:
  - requested → requested (same payload) is no-op.
  - requested → completed (same payload twice) is idempotent.
  - completed → failed (or vice versa) is rejected.

Rationale
- We keep changes minimal to unblock development while capturing the direction for stronger type-safety in a follow-up without introducing new dependencies or breaking public types in this iteration.
