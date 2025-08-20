# Technical debt (ploke-db)

Context
- Added initial ObservabilityStore implementation to unblock ploke-tui M0 observability features.
- Cozo v0.7 Validity/Json semantics adopted in minimal fashion.

Known debt and follow-ups
- Schema management
  - Observability schema is created via best-effort idempotent :create; replace with explicit migration framework and idempotent checks via ::relations.
  - Add indices: tool_call by (parent_id, at DESC), unique (request_id, call_id) modeled by key but add secondary indexes for audits.
- JSON handling
  - API accepts Option<String> for JSON; currently stored via parse_json. Consider typed serde_json::Value and builder helpers, plus redaction toggles prior to prod.
- Idempotency and lifecycle
  - record_tool_call_done implements a basic idempotency check with a single snapshot '@ NOW'. Consider multi-transaction patterns with write locks if needed.
  - Reject invalid transitions (requested → completed → failed) with richer state checks and tests.
- Time semantics
  - For conversation turns, we rely on at='ASSERT' as created_at; no update paths yet. Add retractions if needed and queries over history.
- Callbacks/observability of DB actions
  - Integrate Db::register_callback for tool_call and conversation_turn to stream updates to UI; provide builder APIs to subscribe/unsubscribe.
- Tests
  - Add unit tests for all ObservabilityStore methods (idempotency, listing, JSON round-trip).
  - Add integration tests from ploke-tui to exercise full flow.
- Config
  - Redaction toggles and path normalization policies to be wired once decisions finalize.
- Perf
  - Add pagination with cursors and time windows for list_ APIs; add indexes accordingly.

Tracking
- Reference decisions in crates/ploke-tui/docs/feature/agent-system/*.md.
- Revisit before pre-prod hardening.

Last updated: 2025-08-19
