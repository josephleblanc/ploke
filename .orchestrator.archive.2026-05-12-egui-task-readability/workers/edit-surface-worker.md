# Worker Packet: edit-surface-worker

- role: Worker

## Active Task

### esm-core-mandatory-surfacegrant: Make SurfaceGrant authority mandatory and check-preserving

- lane: edit-surface-authority
- priority: 1
- state: AssignedActive { worker: "edit-surface-worker" }
- allowed_edit:
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs
- forbidden_edit:
  - crates/ploke-eval/src/cli/prototype1_state/backend.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs
  - crates/ploke-eval/src/cli/prototype1_state/history.rs
  - docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - .orchestrator/reports/esm-review-surfacegrant-coordinate-policy.md
- acceptance:
  - Remove Option<GrantAuthority>; normal Grant/SurfaceGrant must require coordinate and policy authority.
  - Material-only bounds, if needed for helper logic, must be a distinct non-authority adapter and must not be constructible as Grant.
  - Check must preserve grant authority/provenance so downstream apply/History can cite it.
  - Focused tests prove broad/request-style construction cannot mint non-authority Grant, narrow preserves authority, and checked path exposes coordinate/policy.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
