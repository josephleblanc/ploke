# Worker Packet: worker-records

- role: Worker

## Active Task

### repair-agent-turn-record-feature-boundary: Repair agent-turn record feature boundary

- lane: records
- priority: 3
- state: AssignedActive { worker: "worker-records" }
- allowed_edit:
  - crates/ploke-records
- forbidden_edit:
  - crates/ploke-eval
  - crates/ploke-tree
  - crates/ploke-egui
- docs:
  - .orchestrator/graph-ingestion-wave2-workers/review-records-agent-turn-passive-shapes.report.md
  - docs/active/agents/ploke-tree-graph-ingestion/implementation-lanes.md
- acceptance:
  - ploke-tree can depend on passive agent-turn records without pulling ploke-tui
  - agent-turn record owner remains typed and does not use production serde_json::Value walking
  - No ploke-eval edits

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
