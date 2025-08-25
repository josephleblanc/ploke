# Implementation log 004 — M0 hardening follow-up (2025-08-19)

Summary
- Continued M0 plan execution: telemetry improvements for typed tool events, fixed test compilation, and removed minor warnings.

Changes
- EventBus SSoT test
  - Updated crates/ploke-tui/src/event_bus/mod.rs test to construct IndexingStatus via Into from IndexStatus, avoiding reliance on a non-existent Default.
- Telemetry
  - Added structured tracing fields to ToolEvent::Requested emission and handling:
    - In llm/tool_call.rs: emit request_id, parent_id, tool, call_id.
    - In llm/mod.rs (llm_manager): log request_id, parent_id, vendor, tool, call_id on receipt.
- Warnings cleanup
  - Removed an unused import from app/events.rs.

Notes
- The SSoT loop continues to translate IndexStatus → AppEvent exclusively in run_event_bus.
- Tool event bridge remains: LlmTool(ToolEvent) is primary, SystemEvent::ToolCallRequested stays for M0 compatibility.

Next
- Add E2E tool-call tests with correlation assertions.
- Wire more span fields across rag_handlers and persistence hook (per ploke_db_contract.md).
