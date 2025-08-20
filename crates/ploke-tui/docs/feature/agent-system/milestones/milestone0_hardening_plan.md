# Milestone 0 — Baseline hardening and observability (granular plan)

Objective
- Ship reliable, observable, and reversible foundations before enabling more autonomy.
- Make every tool action auditable, chat histories reliably persisted, and event routing unambiguous.

Scope overview
- Eventing: single source of truth for indexing; typed LLM tool events design and bridge.
- Persistence: conversations, tool calls, and chat history with atomic writes.
- Telemetry: correlated tracing with stable IDs across subsystems.
- Safety: clear IO path policy violations with user guidance.
- Capacity hygiene: backpressure awareness and lag observation.
- Documentation: observability guide; decision queue; implementation logs.

Workstreams

A) Eventing: SSoT for indexing, typed tool events
1. Indexing SSoT
   - Change: handlers/indexing.rs should not emit AppEvent::IndexingCompleted/Failed directly.
   - Only run_event_bus() turns IndexingStatus::Completed/Cancelled into AppEvents.
   - Add test: spawn run_event_bus, inject IndexingStatus::Completed over index_tx, assert a single IndexingCompleted is received on realtime channel.
2. Typed LLM tool events (design + compatibility)
   - Define:
     - enum ToolEvent { Requested {…}, Completed {…}, Failed {…} }
     - AppEvent::LlmTool(ToolEvent)
   - Maintain a temporary bridge from SystemEvent::ToolCallRequested to ToolEvent::Requested.
   - Update llm/session.rs and llm/tool_call.rs to use ToolEvent internally; bridge to SystemEvent only for compatibility.
   - Document removal plan of the SystemEvent variant in M1.

B) Persistence: DB and chat history
1. ploke-db contract (see ploke_db_contract.md)
   - Functions:
     - record_tool_call_requested, record_tool_call_completed, record_tool_call_failed (idempotent on (request_id, call_id)).
     - upsert_conversation_turn (user/assistant/sysinfo).
   - Schemas keyed by (request_id, call_id) with created_at/ended_at and args_sha.
2. Chat history persistence fix
   - FileManager::save_content:
     - Write to temp file in the destination directory, fsync, then atomic rename to final path ".ploke_history.md" (or decision from USER).
     - Log the final file path (not the directory) on success.
   - Emit SysInfo on success/failure.

C) Telemetry: correlated tracing
1. Tool-call telemetry fields
   - request_id (Uuid), call_id (String), vendor, tool_name
   - args_sha256 (of canonicalized arguments JSON)
   - started_at, ended_at, latency_ms
   - outcome: ok/err, error_kind, error_msg
2. Span propagation
   - Attach %request_id and %call_id on all tool boundaries (llm → tool_call::dispatch → handlers::rag).
   - Confirm Io layer error surfacing includes path, policy, and file info.

D) Safety envelope: I/O path policy
- Enforce absolute-path requirement and symlink policy in ploke-io (default).
- On violations: return Fatal/Warning mapping with user-friendly SysInfo guidance (“Use absolute paths; ensure file is under allowed roots; see config docs”).
- Document (for M1) a config path to set allowed roots and symlink policy.

E) Backpressure and capacity hygiene
- Review EventBusCaps defaults; ensure realtime queue is bounded conservatively.
- Observe RecvError::Lagged occurrences and emit a Rate-limited SysInfo summary when detected (dev-level logging ok for M0).

F) Documentation
- Observability guide: “Follow a tool call end-to-end” with grep examples for request_id; DB query snippets; where logs are written.
- Implementation logs: create implementation-log-001.md for M0 decisions.
- Decision queue: decisions_required.md with blockers/directional choices.

Acceptance criteria
- IndexingCompleted/Failed emitted exactly once per indexing run.
- Every tool call generates:
  - one DB row on request and one on completion/failure (idempotent across retries),
  - structured logs with correlation fields,
  - AppEvent completion/failure with matching request_id/call_id.
- Chat history save writes to the documented final file via atomic rename.
- All new .expect/.unwrap on event paths removed or justified with comments.

Out of scope for M0
- Git integration and validation gates (fmt/clippy/test) are M1/M3, respectively.
- Code edit approval pipeline UX; multi-branch agents.

Risks and mitigations
- Breaking event routing: Keep compatibility bridge until M1.
- Logging sensitive data: Hash arguments and avoid dumping full payloads; content/body redactions configurable later.
- Cozo schema churn: Version the new relations; keep migrations idempotent; include smoke tests.

Dependencies
- ploke-db adds the logging APIs and relations (contract doc).
- ploke-io already enforces path policy; surface errors cleanly.

PR slicing (suggested)
1) Eventing SSoT + tests
2) FileManager atomic save + SysInfo + doc
3) Telemetry fields + span propagation
4) DB logging functions (+ behind a feature flag if needed) + E2E tests
5) Documentation (observability guide) + initial implementation log

References
- See crates/ploke-tui/docs/ploke_db_contract.md for DB API and invariants.
- See crates/ploke-tui/docs/agentic_system_plan.md for roadmap context.

Progress update — 2025-08-20
- Implemented: Observability latency_ms computed from requested→done (DB lookup via get_tool_call).
- Implemented: SSoT tests for Completed and Failed passing (single forwarder).
- Implemented: EventBus readiness signal to eliminate sleeps in tests.
- Implemented: SHA-256 for args_sha256 (replacing FNV-1a placeholder).
- In progress: Observability E2E tests and observability guide doc.
