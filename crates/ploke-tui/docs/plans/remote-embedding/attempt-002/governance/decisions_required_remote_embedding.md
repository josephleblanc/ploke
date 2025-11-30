# Remote Embedding Attempt 002 – Decisions Requiring Review

This document mirrors the global `decisions_required.md` queue but focuses on the remote-embedding effort. All new questions, approvals, and resolutions for this feature should be captured here (with references back to the global governance file when necessary).

## Open decisions

- **REM-EMB-001 — Kill-switch rollback policy**
  - Context: Slice 4 flips multi-embedding flags ON and removes legacy columns. We plan to keep a runtime kill switch only during the soak period, but we need guidance on how long that switch must remain available and what operational steps (DB backups, CLI toggles) are required before full removal.
  - Options:
    1. Keep the kill switch for a fixed number of releases (e.g., two tagged builds) with a documented rollback checklist (DB snapshot + config toggle).
    2. Keep the kill switch indefinitely but gate it behind a config flag so it cannot be triggered accidentally.
    3. Remove the kill switch immediately after soak once tests pass, relying solely on git reverts if issues arise.
  - Recommended: Option 1 to balance safety with avoiding permanent legacy code.
  - Blocking: Yes — we cannot schedule Slice 4 cleanup until confirmed.

- **REM-EMB-002 — Storage sizing for multiple embedding sets**
  - Context: Remote embeddings retain multiple provider/model/dimension sets per node. We need a policy for how many sets to keep and at what thresholds pruning kicks in (affects `/embedding prune` defaults and telemetry expectations).
  - Options:
    1. Hard cap per node type (e.g., keep at most two active sets; require manual drop for more).
    2. Global byte/row budget enforced via `/embedding prune --max-bytes`.
    3. No automatic cap; rely on operator commands only.
  - Recommended: Option 1 with telemetry instrumentation so we can revisit later.
  - Blocking: Medium — CLI defaults and DB migrations depend on this call before Slice 3.

## Resolved decisions
*(Add entries here once the USER signs off and update the execution plan / slice reports accordingly.)*

## Notes
- The global queue at `crates/ploke-tui/docs/archive/feature/agent-system/decisions_required.md` now links to this file for remote-embedding specific topics.
- When decisions are resolved here, mirror the outcome back to the global file for historical continuity.
