# Worker Packet: edit-surface-integration-worker

- role: Worker

## Active Task

### esm-join-grant-evidence-backend-history: Join checked grant authority into History evidence

- lane: edit-surface-integration
- priority: 1
- state: AssignedActive { worker: "edit-surface-integration-worker" }
- allowed_edit:
  - crates/ploke-eval/src/cli/prototype1_state/backend.rs
  - crates/ploke-eval/src/cli/prototype1_state/history.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs
- forbidden_edit:
  - crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs
  - docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - .orchestrator/reports/esm-admission-coordinate-policy.md
  - .orchestrator/reports/esm-history-surfacegrant-evidence.md
- acceptance:
  - SurfaceEvidence::checked or equivalent accepts typed grant/check evidence derived from the actual checked SurfaceGrant, not policy strings plus grant: None.
  - CheckedSurfaceEdit::surface_evidence passes original coordinate/policy/grant authority into History using named Rust types.
  - TUI apply authority and backend checked authority remain aligned; no prompt/child-plan/request-file authority.
  - No edits outside backend.rs, history.rs, and edit_surface/tui.rs.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
