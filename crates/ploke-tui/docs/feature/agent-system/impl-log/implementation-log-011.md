# Implementation log 011 — M0 hardening: EventBus readiness + SHA-256 + doc pruning (2025-08-20)

Summary
- Added EventBus readiness signal: run_event_bus now emits AppEvent::EventBusStarted on startup so tests can await readiness deterministically (no sleeps).
- Replaced FNV-1a placeholder with SHA-256 for args_sha256 in observability; added sha2 dependency.
- Trimmed documentation for focus by replacing the verbose "Copy/paste-friendly list" with a minimal context list and updating milestone progress.

Changes
- crates/ploke-tui/src/lib.rs
  - New AppEvent::EventBusStarted variant; priority marked Realtime.
- crates/ploke-tui/src/event_bus/mod.rs
  - run_event_bus sends EventBusStarted once after subscriptions.
  - Tests now await EventBusStarted instead of sleeping; reordered subscription before spawn to avoid race.
- crates/ploke-tui/src/app/events.rs
  - Handle EventBusStarted as a no-op to keep UI lean.
- crates/ploke-tui/src/observability.rs
  - args_sha256 now uses sha256_hex; removed fnv1a64 helper; imported sha2.
- crates/ploke-tui/Cargo.toml
  - Added sha2 = "0.10".
- Docs
  - Updated m0_status_report to mark SHA-256 and readiness as done.
  - Updated milestone0_hardening_plan progress.
  - Pruned m0_context_files minimal list.

Rationale
- Deterministic tests improve stability under broadcast semantics.
- Stronger hashing aligns with privacy and audit requirements.
- Focused docs reduce cognitive load for remaining M0 work.

Next steps (to complete M0)
- Add observability end-to-end tests for requested → completed/failed path using ploke-db.
- Draft a short observability guide with example Cozo queries.
- Minor UI polish around IndexingCompleted/Failed messages if needed.

Acceptance alignment
- SSoT for indexing preserved and tested (single forwarder).
- Telemetry: latency_ms and args_sha256 (SHA-256) recorded.
- Documentation now reflects current state and next concrete actions.
