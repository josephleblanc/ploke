# Implementation log 007 — M0 hardening: accelerate SSoT, lag rate-limiting, and export confirmations (2025-08-19)

Summary
- Accelerated M0 implementation pace per directive: took multiple steps in one change set.
- Eventing SSoT: Expanded run_event_bus to forward Failed and Cancelled statuses, and emit IndexingStarted exactly once per run.
- Capacity hygiene: Added rate-limited warnings for broadcast lag to avoid UI noise and log spam.
- Persistence UX: Added SystemEvent::HistorySaved and UI handler to confirm chat export success.
- Tests: Added a new SSoT test to assert IndexingStatus::Failed is forwarded exactly once.

Changes
- crates/ploke-tui/src/event_bus/mod.rs
  - Added IndexStatus::Failed → AppEvent::Error + AppEvent::IndexingFailed mapping.
  - Emitted AppEvent::IndexingStarted on first Running status per indexing run.
  - Rate-limited RecvError::Lagged warnings to at most once per second.
  - New test ssot_forwards_indexing_failed_once mirroring the Completed test.
- crates/ploke-tui/src/file_man.rs
  - On successful SaveRequested, emit SystemEvent::HistorySaved on realtime channel.
- crates/ploke-tui/src/lib.rs
  - Added SystemEvent::HistorySaved { file_path } and mapped it to realtime priority.
- crates/ploke-tui/src/app/events.rs
  - Handle HistorySaved by emitting a SysInfo message to the timeline.

Rationale
- Single source of truth: ensure all indexing outcomes (Completed/Cancelled/Failed) are routed exclusively via run_event_bus, never from handlers.
- Operator visibility: confirm when exports succeed; reduce repeated lag warnings to actionable cadence.

Notes and next steps
- Continue larger, end-to-end steps for M0:
  - Wire ObservabilityStore integration once ploke-db exposes the contract.
  - Persist conversation turns and tool-call lifecycles with Validity and Json (per contract).
  - Add correlation fields to tool events in logs (already present for request_id/call_id in several places).
- Consider a readiness handshake for EventBus in tests to remove sleeps entirely (non-blocking for M0).

Blockers referenced
- See updates added to decisions_required.md in this commit regarding ploke-db ObservabilityStore and chat history DB persistence triggers.

Requirement reminder
- From this point forward, implement larger slices per step to accelerate M0, balancing thorough tests and minimal risk.
