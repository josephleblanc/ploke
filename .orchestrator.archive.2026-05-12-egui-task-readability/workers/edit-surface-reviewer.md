# Worker Packet: edit-surface-reviewer

- role: Reviewer

## Active Task

### esm-review-surfacegrant-coordinate-policy: Review SurfaceGrant coordinate/policy patch

- lane: edit-surface-review
- priority: 1
- state: AssignedActive { worker: "edit-surface-reviewer" }
- forbidden_edit:
  - docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - .orchestrator/reports/esm-retain-model.md
  - .orchestrator/reports/esm-map-eval-code.md
  - .orchestrator/reports/esm-surfacegrant-coordinate-policy.md
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs
- acceptance:
  - Review the patch for correctness, semantic alignment with model.md, structural naming, authority/provenance risks, and test adequacy.
  - No edits; return findings ordered by severity with exact file:line references and recommended accept/fix decision.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
