# Worker Packet: edit-surface-cli-worker

- role: Worker

## Active Task

### esm-cli-supply-edit-admission: Supply edit-surface admission at CLI-facing callsites

- lane: edit-surface-cli
- priority: 1
- state: AssignedActive { worker: "edit-surface-cli-worker" }
- allowed_edit:
  - crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
- forbidden_edit:
  - crates/ploke-eval/src/cli/prototype1_state/backend.rs
  - crates/ploke-eval/src/cli/prototype1_state/history.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs
  - docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - .orchestrator/reports/esm-admission-coordinate-policy.md
- acceptance:
  - Update cli_facing.rs callsites to pass real EditSurfaceAdmission coordinate and policy into validate_edit_surface_candidate.
  - Do not synthesize authority from prompt files, child-plan paths, request files, or TUI status; if real runtime/artifact coordinate is unavailable, report blocker with exact file:line.
  - Remove/replace remaining cli_facing.rs surface::Grant::new helper use with authority-bearing construction if it is a test helper; no material-only Grant.
  - No edits outside cli_facing.rs.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
