# Worker Packet: worker-loader

- role: Worker

## Active Task

### repair-agent-turn-loader-scope: Constrain agent-turn loader discovery

- lane: loader
- priority: 3
- state: AssignedActive { worker: "worker-loader" }
- allowed_edit:
  - crates/ploke-tree/src/store
  - crates/ploke-tree/Cargo.toml
- forbidden_edit:
  - crates/ploke-eval
  - crates/ploke-records
  - crates/ploke-tree/src/graph
  - crates/ploke-egui
- docs:
  - .orchestrator/graph-ingestion-wave2-workers/review-loader-agent-turn-runrecordset.report.md
  - docs/active/agents/ploke-tree-graph-ingestion/implementation-lanes.md
- acceptance:
  - Agent-turn loader only discovers expected run-local files, not entire run root recursion
  - Store loader deserializes through named ploke-records types
  - No ploke-eval edits

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
