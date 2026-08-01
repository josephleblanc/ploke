# Worker Packet: edit-surface-admission-worker

- role: Worker

## Active Task

No active task assigned.

## Queued Tasks

### esm-backend-pass-checked-grant-evidence: Pass checked SurfaceGrant evidence from backend

- lane: edit-surface-admission
- priority: 1
- state: AssignedQueued { worker: "edit-surface-admission-worker" }
- allowed_edit:
  - crates/ploke-eval/src/cli/prototype1_state/backend.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs
- forbidden_edit:
  - crates/ploke-eval/src/cli/prototype1_state/history.rs
  - crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - .orchestrator/reports/esm-admission-coordinate-policy.md
  - .orchestrator/reports/esm-history-surfacegrant-evidence.md
- acceptance:
  - CheckedSurfaceEdit::surface_evidence passes the actual checked coordinate/policy/grant evidence through the History API instead of string-only policy recovery.
  - Backend checked authority and TUI apply authority remain aligned.
  - No edits outside backend.rs and edit_surface/tui.rs; if History API is not ready, report exact blocker.


## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
