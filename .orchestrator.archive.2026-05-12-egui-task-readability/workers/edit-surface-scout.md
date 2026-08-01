# Worker Packet: edit-surface-scout

- role: Retainer
- refresh_after_questions: 5

## Active Task

### esm-map-eval-code: Map eval-owned edit-surface code to model.md

- lane: edit-surface-scout
- priority: 1
- state: AssignedActive { worker: "edit-surface-scout" }
- forbidden_edit:
  - docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface
  - crates/ploke-eval/src/cli/prototype1_state/history.rs
  - crates/ploke-eval/src/cli/prototype1_state/backend.rs
- acceptance:
  - Return exact file:line ranges for SurfaceGrant/Grant, Proposal/EditProposal, Check/CheckedProposal, ArtifactDelta, SurfaceEvidence, surface_attempt, and OperationCoordinate equivalents.
  - Identify drift where code treats request/prompt/child-plan files as authority instead of SurfaceGrant+check+History evidence.
  - No edits; include smallest verification commands.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
