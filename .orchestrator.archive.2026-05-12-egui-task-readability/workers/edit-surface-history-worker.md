# Worker Packet: edit-surface-history-worker

- role: Worker

## Active Task

### esm-history-checked-grant-api: Make SurfaceEvidence checked API accept typed grant authority

- lane: edit-surface-history
- priority: 1
- state: AssignedActive { worker: "edit-surface-history-worker" }
- allowed_edit:
  - crates/ploke-eval/src/cli/prototype1_state/history.rs
- forbidden_edit:
  - crates/ploke-eval/src/cli/prototype1_state/backend.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs
  - crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - .orchestrator/reports/esm-history-surfacegrant-evidence.md
- acceptance:
  - SurfaceEvidence::checked or equivalent takes typed grant/check evidence and no longer hardcodes grant: None for checked edit-surface evidence.
  - Keep existing History tests typed; update local History tests to construct the named grant/check evidence carrier.
  - No serde_json::Value, no prompt/child-plan/request-file authority, and no edits outside history.rs.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
