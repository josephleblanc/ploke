# Worker Packet: worker-graph-types

- role: Worker

## Active Task

### playback-fine-module-split: Split oversized playback fine module

- lane: projection
- priority: 1
- state: AssignedActive { worker: "worker-graph-types" }
- allowed_edit:
  - crates/ploke-tree/src/playback/fine.rs
  - crates/ploke-tree/src/playback/fine
  - crates/ploke-tree/src/playback/mod.rs
- forbidden_edit:
  - crates/ploke-eval
  - crates/ploke-records
  - crates/ploke-tree/src/store
  - crates/ploke-tree/src/graph
- docs:
  - docs/active/agents/ploke-tree-graph-ingestion/implementation-lanes.md
- acceptance:
  - fine.rs is split into focused submodules without behavior changes
  - Public playback API remains compatible for current callers
  - Focused projection tests still pass

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
