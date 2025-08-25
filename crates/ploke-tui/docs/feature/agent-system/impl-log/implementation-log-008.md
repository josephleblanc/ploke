# Implementation log 008 — M0 hardening: DB observability writer (2025-08-19)

Summary
- Added a background Observability task that listens to AppEvents and persists conversation turns and tool-call lifecycles to ploke-db per the contract.
- Supports both typed LLM tool events (ToolEvent::{Requested,Completed,Failed}) and the legacy SystemEvent bridge for M0 compatibility.
- Avoided Cargo changes by using a lightweight FNV-1a 64-bit hex as a temporary stand-in for args_sha256; will switch to real SHA-256 in a follow-up when Cargo edits are included.

Changes
- crates/ploke-tui/src/observability.rs (new)
  - Subscribes to both realtime and background EventBus channels.
  - On MessageUpdated: upserts conversation_turn with kind/content and parent linkage.
  - On tool-call Requested/Completed/Failed: records tool_call requested and done rows with Validity timestamps; wraps non-JSON content as JSON strings for parse_json on DB side.
- crates/ploke-tui/src/lib.rs
  - Exposes new observability mod and spawns the Observability task alongside run_event_bus and llm_manager.

Rationale
- Centralize persistence as an event-driven sidecar to avoid invasive changes across handlers and to honor the SSoT flow.
- Preserve system compatibility during M0 by observing both typed and legacy tool events.

Notes and next steps
- Replace FNV placeholder with SHA-256 once Cargo.toml edits are staged (sha2 or ring).
- Expand persistence to include latency measurement (compute from Requested→Done).
- Consider batching and backpressure controls in M1; for M0 we persist on each event.

Acceptance alignment
- Tool-call lifecycle: one row on request and one on completion/failure; idempotency enforced by ploke-db.
- Chat history persistence: each MessageUpdated leads to an upserted conversation_turn.
