# Worker Packet: live-worker

- role: Worker

## Active Task

### p1-live-network-splice: Prepare live broad headless-TUI verification

- lane: live-verification
- priority: 3
- state: AssignedActive { worker: "live-worker" }
- docs:
  - docs/active/CURRENT_FOCUS.md
  - docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md
- acceptance:
  - Reports exact live test command, credential preflight, and newest published request path if present.
  - No source files edited by this task.
  - If live test runs, reports admitted/rejected outcome without dumping model payloads.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
