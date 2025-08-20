# Implementation Log 000 — Establish Production Plan and Procedures

Date: 2025-08-18

Summary
- Added production plan at docs/production_plan.md defining phased roadmap from current state to production readiness.
- Established implementation-log rotation procedure (2-log window) and change hygiene practices.
- Linked plan from README to make it discoverable.

Current Readiness Snapshot
- Read path: functional with bounded concurrency, but needs per-request hash verification and simpler ordering.
- Scan path: works but should adopt bounded stream backpressure semantics.
- Error mapping: minor inconsistencies around shutdown/channel; improve From<RecvError> and mapping policy.
- Docs: strong, but README hashing example should align with TrackingHash usage.

Changes Made in This Step
- New file: docs/production_plan.md
- New file: docs/implementation-log-000.md
- README updated to reference the plan and logging process.

Impact and Risks
- No code behavior changes; documentation-only.
- Provides shared understanding and concrete next steps across contributors.

Next Steps (Plan References)
- Phase 1 (Read Path Hardening and Hygiene):
  - Implement per-request file hash verification and pre-allocated result ordering.
  - Extract read/parse/extract helpers to reduce complexity.
  - Add PLOKE_IO_FD_LIMIT env override with clamping; document.
  - Adopt bounded concurrency in scan path (buffer_unordered).
  - Reinstate From<RecvError> for IoError and align error mapping.
  - Update README example to use TrackingHash consistently; remove seahash example.
  - Add tests enumerated in Phase 1.

Verification
- None required beyond docs lint/readability; run cargo test -p ploke-io to confirm no build breakages post-doc changes.

Links
- Plan: crates/ploke-io/docs/production_plan.md
