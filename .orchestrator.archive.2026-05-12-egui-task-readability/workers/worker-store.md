# Worker Packet: worker-store

- role: Worker

## Active Task

### store-load-attempt-results: Load nodes results attempt runner result files into RunRecordSet

- lane: loader
- priority: 1
- state: AssignedActive { worker: "worker-store" }
- allowed_edit:
  - crates/ploke-tree/src/store
  - crates/ploke-tree/src/lib.rs
- forbidden_edit:
  - crates/ploke-eval
  - crates/ploke-records
  - crates/ploke-egui
  - crates/ploke-tui
- docs:
  - docs/active/agents/ploke-tree-graph-ingestion/orchestration.md
  - docs/active/agents/ploke-tree-graph-ingestion/2026-05-11_3a2ea733-persisted-surface-survey.md
- acceptance:
  - Store loader scans nodes/*/results/*.json as RunnerResultRecord without adding new passive DTOs
  - Attempt-scoped results remain separate from latest runner-result projections
  - No graph semantics or filesystem discovery outside store loading

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
