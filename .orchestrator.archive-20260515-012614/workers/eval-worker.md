# Worker Packet: eval-worker

- role: Worker

## Active Task

### p1-eval-fitness-gate: Reject operational regressions in branch evaluation

- lane: eval-fitness
- priority: 3
- state: AssignedActive { worker: "eval-worker" }
- allowed_edit:
  - crates/ploke-eval/src/branch_evaluation.rs
- forbidden_edit:
  - crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
- docs:
  - docs/active/CURRENT_FOCUS.md
- acceptance:
  - Branch evaluation rejects tool_calls_failed increases and aborted=false to true.
  - Branch evaluation treats nonempty_valid_patch=false to true as improvement and true to false as regression.
  - Focused branch_evaluation tests cover the new fields.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
