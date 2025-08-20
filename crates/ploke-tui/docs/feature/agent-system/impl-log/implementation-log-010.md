# Implementation log 010 — M0 hardening: latency tracking and plan sync (2025-08-20)

Summary
- Added latency_ms computation for tool-call completions/failures by correlating with the previously recorded Requested row.
- Uses ploke-db ObservabilityStore::get_tool_call to retrieve started_at (Validity timestamp), then computes ended_at_ms − started_at_ms.
- No API or eventing changes; SSoT and typed tool events remain unchanged.

Changes
- crates/ploke-tui/src/observability.rs
  - persist_tool_done now:
    - fetches the request row via get_tool_call(request_id, call_id),
    - computes ended_at_ms (now) and latency_ms (non-negative),
    - populates ToolCallDone with these values.

Rationale
- Populate telemetry without spreading timing logic across handlers.
- Keeps conversions at the boundary; continues the typed params pattern from previous logs.

Notes and next steps
- Add E2E tests asserting that Requested → Completed populates latency_ms > 0.
- Consider a small readiness signal in run_event_bus to eliminate sleeps in tests.
- Replace FNV-1a placeholder with SHA-256 for args_sha256 once Cargo dep changes are staged.

Acceptance alignment
- Telemetry: latency_ms is now recorded for terminal tool-call events.
- Eventing: no duplicate IndexingCompleted/Failed; SSoT remains single forwarder.
