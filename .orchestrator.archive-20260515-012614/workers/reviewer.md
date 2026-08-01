# Worker Packet: reviewer

- role: Reviewer

## Active Task

### p1-scaffold-label-review: Review deterministic scaffold labeling

- lane: review
- priority: 3
- state: AssignedActive { worker: "reviewer" }
- docs:
  - docs/active/CURRENT_FOCUS.md
  - docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md
- acceptance:
  - Review confirms deterministic scaffold labeling does not create authority/provenance drift.
  - Review confirms the focused test covers emitted proposal content and child-plan validation remains meaningful.

## Queued Tasks

### p1-eval-fitness-review: Review branch evaluation fitness gates

- lane: review
- priority: 3
- state: AssignedQueued { worker: "reviewer" }
- docs:
  - docs/active/CURRENT_FOCUS.md
- acceptance:
  - Review confirms no structural/naming drift and no missing regression edge for the new branch evaluation fields.
  - Review report lists any residual test gap or explicitly says none.


## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
