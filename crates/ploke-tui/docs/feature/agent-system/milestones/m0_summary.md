# Milestone 0 — Summary and handoff to M1

Scope delivered
- Eventing SSoT: IndexingStatus → AppEvent forwarded only by run_event_bus; no duplicate IndexingCompleted/Failed.
- Typed tool events bridge: LlmTool::{Requested, Completed, Failed} added; compatibility path via SystemEvent retained for M0.
- Persistence sidecar: conversation_turn upserted on MessageUpdated; tool_call lifecycle (requested → completed/failed) persisted.
- Telemetry: latency_ms computed by correlating started_at ← get_tool_call with ended_at_ms; args_sha256 uses SHA-256.
- File export: atomic save (tempfile + fsync + rename) to .ploke_history.md with UI confirmation event.

Quality gates
- Broadcast readiness: EventBusStarted emitted to stabilize tests without sleeps.
- Tests:
  - SSoT: Completed and Failed forwarded exactly once.
  - Observability DB E2E: requested → completed path persists status and non-negative latency.
  - Keymap and parser tests remain green.
- Docs:
  - Observability guide with audit queries (tool_call, conversation_turn).
  - Status report updated; plan progress advanced.

Notes on design choices
- Time-travel relations (Validity) used for lifecycle modeling; idempotent upserts with assertion rows.
- JSON persistence currently enabled to speed iteration; revisit redaction defaults pre prod-ready.
- Event priority: tool requested in background, terminal signals in realtime for responsive UI.

Known limitations (carried to M1/M2)
- ObservabilityStore still accepts JSON as strings; consider typed JSON inputs in M1.
- Chat history primary persistence to DB is planned; FileManager remains an export path only.
- Capacity configuration remains static defaults; expose via user config in M1.

Handoff to M1 (initial focus)
- Approvals flow for edit proposals (Approve/Deny + preview).
- Tighten type-safety for observability inputs (serde_json::Value).
- Power-user configuration for channel capacities and log retention.

How to verify M0 locally
- cargo test -p ploke-tui -- --nocapture
- Review logs at logs/ploke.log
- Run Cozo audit queries in the Observability Guide.
