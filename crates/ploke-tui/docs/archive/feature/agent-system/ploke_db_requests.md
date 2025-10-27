# ploke-db requests and API coordination (M0 → M1)

Overview
- This document tracks concrete requests to ploke-db to support M0 (observability) and M1 (editing HIL), consistent with Cozo v0.7 Validity/Json semantics.

M0 — Observability store
- Relations (time-travel enabled; last key is at: Validity):
  - tool_call(request_id: Uuid, call_id: String, at: Validity => parent_id: Uuid, vendor: String, tool_name: String, args_sha256: String, arguments_json: Json?, status: String, ended_at_ms: Int?, latency_ms: Int?, outcome_json: Json?, error_kind: String?, error_msg: String?)
  - conversation_turn(id: Uuid, at: Validity => parent_id: Uuid?, message_id: Uuid, kind: String, content: String, thread_id: Uuid?)
    - Note: we can also model conversation_turn as non-time-travel if we never mutate; but Validity keeps options open.
- API (idempotent on (request_id, call_id)):
  - record_tool_call_requested(req)
    - Asserts a row with status="requested", arguments_json possibly redacted, args_sha256 required.
  - record_tool_call_done(done)
    - Asserts a row with status="completed" or "failed" carrying ended_at_ms, latency_ms, outcome_json OR error_* fields.
  - get_tool_call(request_id, call_id) -> Option<(requested, done?)>
  - list_tool_calls_by_parent(parent_id, limit)
  - upsert_conversation_turn(turn)
  - list_conversation_since(since_ms, limit)
- Cozo specifics:
  - Use 'ASSERT' for inserts to stamp the same microsecond timestamp across the entire transaction (requested + done if in one tx).
  - Query history or snapshots with '@ NOW' and optionally format_timestamp(at) for display.
- Path policy:
  - Persist project-relative paths in any payloads; absolute-path policies remain enforced at ploke-io layer.

M1 — Editing proposals
- Relation (time-travel; last key at: Validity):
  - code_edit_proposal(request_id: Uuid, at: Validity => diffs_json: Json, confidence: Float?, status: String, decided_at_ms: Int?, applied_at_ms: Int?, commit_hash: String?)
- API:
  - record_edit_proposed(req_id, diffs_json, confidence?)
  - record_edit_decision(req_id, status: "approved" | "denied")
  - record_edit_applied(req_id, results_json, applied_at_ms, commit_hash?)
  - get_edit_proposal(req_id)
- Privacy:
  - Diffs may include content; allow redaction and store hashes when disabled.

Minimal ploke-db surfaces currently used by ploke-tui (for reference)
- Types and functions (observed in this repo):
  - Database::init_with_schema(), Database::new(), Database::new_init()
  - Database::raw_query(), raw_query_mut(), relations_vec(), import_from_backup(), count_relations()
  - Database::get_crate_name_id(crate_name), get_crate_files(crate_name), get_path_info(path)
  - create_index_primary(&Database), create_index_warn(&Database, NodeType), replace_index_warn(&Database, NodeType)
  - retract_embedded_files(file_id, NodeType)
  - bm25_index::bm25_service::start(...)
  - CallbackManager::new_bounded(...), plus callback handling
- Types: NodeType, TypedEmbedData, QueryResult, TrackingHash (ploke-core)

Acceptance tests (suggested)
- Idempotency: requested then done twice → second is a no-op; status remains completed/failed.
- Snapshot: '@ NOW' returns the latest state; querying a historical timestamp reflects prior status.
- Redaction toggle: when disabled, arguments_json/outcome_json are stored as Json; when enabled, only args_sha256 is kept.
