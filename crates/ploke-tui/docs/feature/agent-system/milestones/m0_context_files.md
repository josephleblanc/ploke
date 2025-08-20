# Milestone 0 — Context files checklist

Keep these files in context while implementing M0 (hardening and observability):

Core eventing and telemetry
- crates/ploke-tui/src/event_bus/mod.rs
  - Ensure run_event_bus is the sole forwarder of IndexingStatus → AppEvent.
- crates/ploke-tui/src/app_state/handlers/indexing.rs
  - Remove direct emissions of IndexingCompleted/Failed; emit IndexingStatus only.
- crates/ploke-tui/src/app/events.rs
  - Handles AppEvent::IndexingCompleted/Failed; ensure no duplicate reactions.
- crates/ploke-tui/src/error.rs
  - ResultExt/ErrorExt emission used across subsystems.
- crates/ploke-tui/src/tracing_setup.rs
  - Default EnvFilter and file logging.

Copy/paste-friendly list:
crates/ploke-tui/src/event_bus/mod.rs
crates/ploke-tui/src/app_state/handlers/indexing.rs
crates/ploke-tui/src/app/events.rs
crates/ploke-tui/src/error.rs
crates/ploke-tui/src/tracing_setup.rs

Tool-call bridge (compat path for M0)
- crates/ploke-tui/src/llm/tool_call.rs
- crates/ploke-tui/src/llm/session.rs
- crates/ploke-tui/src/llm/mod.rs
  - Maintain SystemEvent::ToolCallRequested bridge; plan typed events for M1.

Persistence (chat history and queries)
- Primary: Persist chat history in DB (see ploke_db_contract.md). FileManager remains for export paths.
- crates/ploke-tui/src/file_man.rs
  - SaveRequested remains an export path; not the primary persistence in M0.

Docs and contracts
- crates/ploke-tui/docs/feature/agent-system/ploke_db_contract.md
- crates/ploke-tui/docs/dependency_details/cozo/types/time-travel.md (reference)
- crates/ploke-tui/docs/dependency_details/cozo/types/json.md (reference)
- crates/ploke-tui/docs/feature/agent-system/updates-2025-08-19.md (this update)
- crates/ploke-tui/docs/feature/agent-system/milestones/m0_context_files.md (this file)

Tests to add/update
- EventBus SSoT test: inject IndexingStatus::Completed on index_tx, assert exactly one AppEvent::IndexingCompleted.
- (Optional) Tool-call await correlation test exists; extend with telemetry span fields.

Notes
- Do not change API surface to consumers in M0; focus on SSoT, telemetry, and DB persistence plan.
