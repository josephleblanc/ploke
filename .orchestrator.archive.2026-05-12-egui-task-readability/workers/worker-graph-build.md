# Worker Packet: worker-graph-build

- role: Worker

## Active Task

### graph-build-membership-selected-scope: Remove label-only selected membership fallback in graph selection

- lane: graph
- priority: 1
- state: AssignedActive { worker: "worker-graph-build" }
- allowed_edit:
  - crates/ploke-tree/src/graph/build/selection
  - crates/ploke-tree/src/graph/build/selection.rs
- forbidden_edit:
  - crates/ploke-eval
  - crates/ploke-records
  - crates/ploke-egui
  - crates/ploke-tree/src/store
- docs:
  - docs/active/bugs/2026-05-09-prototype1-history-traversal-membership-mismatch.md
- acceptance:
  - Graph selection membership resolution does not assign selected membership by label-only fallback
  - Candidate-set root or recorded candidate identity governs selected membership attachment
  - No store, records, or ploke-eval edits

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
