# Milestone 0 — Status report and ETA (2025-08-20)

Scope recap
- Eventing SSoT for indexing, typed tool events bridge, persistence sidecar, telemetry fields, UI confirmations.

Checklist (status)
- [x] Eventing SSoT: run_event_bus is sole forwarder of IndexingStatus → AppEvent; rate-limited lag warnings added; tests for Completed/Failed present.
- [x] Typed LLM tool events: ToolEvent::{Requested, Completed, Failed} bridged from/to SystemEvent for M0.
- [x] Persistence sidecar (observability): conversation_turn upsert on MessageUpdated; tool_call lifecycle logged.
- [x] Latency telemetry: latency_ms computed from requested→done (DB lookup).
- [x] File export path: atomic save with fsync+rename; UI confirmation event HistorySaved.
- [ ] Observability E2E tests (Requested→Done rows + fields) — ETA 0.5d
- [ ] Observability guide (audit queries + grep tips) — ETA 0.5d
- [ ] Replace FNV placeholder with SHA-256 for args_sha256 — ETA 0.25d
- [ ] Optional: run_event_bus readiness signal for tests — ETA 0.25d (not required for acceptance)

Risks and notes
- Test flakiness due to broadcast subscription timing: acceptable for M0; readiness signal reduces sleeps later.
- Privacy posture: currently storing full JSON for fast iteration per decision; revisit defaults pre prod-ready.

ETA to complete M0
- Remaining effort: ~1.25–1.5 days of focused work.

Next actions
- Add observability E2E tests and docs.
- Switch to SHA-256 hashing once dependency changes are permitted.
- Keep accelerated slicing: land tests/docs in a single PR if practical.
