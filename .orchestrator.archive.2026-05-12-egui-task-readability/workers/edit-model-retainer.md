# Worker Packet: edit-model-retainer

- role: Retainer
- refresh_after_questions: 5

## Active Task

### esm-retain-model: Retain edit-surface model invariants

- lane: edit-model-retainer
- priority: 1
- state: AssignedActive { worker: "edit-model-retainer" }
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - crates/ploke-eval/src/cli/prototype1_state/mod.rs
- acceptance:
  - Report the model.md authority chain, lossy reductions to refuse, and 2-6 month structural carriers in <=120 lines.
  - Explicitly ignore docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md as target guidance.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
