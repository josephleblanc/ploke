# Implementation log 005 — M0 hardening: fix IndexingStatus test, context updates (2025-08-19)

Summary
- Continued the Milestone 0 plan by fixing the EventBus SSoT test to construct IndexingStatus explicitly (no Default/Into).
- Added a data type watchlist to the M0 context doc to avoid future pitfalls with IndexingStatus.
- No further code changes required for typed tool events in this step; previous telemetry and bridge remain intact.

Changes
- crates/ploke-tui/src/event_bus/mod.rs
  - Test ssot_forwards_indexing_completed_once now sends an explicit indexer::IndexingStatus struct instead of using Into.
- crates/ploke-tui/docs/feature/agent-system/milestones/m0_context_files.md
  - Appended “Data type watchlist” covering IndexingStatus construction, Cancelled handling, and calc_progress semantics.

Rationale
- ploke_embed::indexer::IndexingStatus does not implement Default or From<IndexStatus>. Tests and components must construct the full struct to send status updates, avoiding trait bound errors.

Next
- Keep expanding E2E coverage for tool-call correlation and persistence once ploke-db exposes the contract functions.
- Consider adding throttling for repeated RecvError::Lagged summaries (nice-to-have for M0).
