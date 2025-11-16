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

- **REM-EMB-001**

USER: We will attempt to provide comprehensive test coverage between the end of
phase 4 and determining whether or not to remove the kill switch. Then,
dependent upon a review and analysis of test coverage, we will follow either 1
or 3. Ideally, we have a high confidence in test coverage and can go with
option 3 so the code remains clear and uncluttered moving forward, as this is
the unambiguous desired design direction of the project.

- **REM-EMB-002**

USER: We will go with Option 3. Additionally, we should add a point to
`ploke/docs/active/TECH_DEBT.md` that we should expose options once we develop
a more detailed config that allows the user to select from among these options,
and will implement automatic pruning at that time.


## Notes
- The global queue at `crates/ploke-tui/docs/archive/feature/agent-system/decisions_required.md` now links to this file for remote-embedding specific topics.
- When decisions are resolved here, mirror the outcome back to the global file for historical continuity.
