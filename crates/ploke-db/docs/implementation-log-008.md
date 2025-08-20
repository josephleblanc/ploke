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
