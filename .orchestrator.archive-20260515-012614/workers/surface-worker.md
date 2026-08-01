# Worker Packet: surface-worker

- role: Worker

## Active Task

### p1-scaffold-label: Make deterministic TUI candidates visibly scaffold-only

- lane: surface-scaffold
- priority: 3
- state: AssignedActive { worker: "surface-worker" }
- allowed_edit:
  - crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
- forbidden_edit:
  - crates/ploke-eval/src/branch_evaluation.rs
- docs:
  - docs/active/CURRENT_FOCUS.md
  - docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md
- acceptance:
  - Generated deterministic EOF comments include scaffold/no-op wording.
  - Existing deterministic child-plan validation still passes.
  - Focused cli_facing test proves scaffold wording is emitted.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
