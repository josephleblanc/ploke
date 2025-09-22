# Implementation log 001 — Plan updates and readiness (2025-08-19)

Context
- Incorporated Cozo v0.7 Validity/Json semantics into our plans and ploke-db contract.
- Captured USER decisions and added “New questions” that are lightweight to resolve.
- Added per-milestone context file lists to keep future conversations lean.

Changes in this commit (docs-only)
- Added updates-2025-08-19.md summarizing normative changes and decisions.
- Added milestones/m0_context_files.md and milestones/m1_context_files.md.
- Added ploke_db_requests.md for API/schema coordination.
- Augmented decisions_required.md with resolved decisions and open questions.
- Noted time-travel modeling and Json storage guidance in ploke_db_contract.md.

Readiness
- Ready to proceed with Milestone 0 implementation.
- Blockers: None beyond confirming the “New questions” in decisions_required.md (Q1–Q4), which do not block initial coding but should be confirmed early.

Next steps (M0)
- Make run_event_bus the single source of truth for IndexingStatus → AppEvent.
- Remove direct IndexingCompleted/Failed emissions from handlers/indexing.rs.
- Wire telemetry spans/fields through tool-call dispatch path (request_id/call_id).
- Prefer DB persistence for chat history; keep FileManager path as optional export.
