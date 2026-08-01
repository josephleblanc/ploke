# Worker Packet: worker-docs

- role: Worker

## Active Task

### docs-update-implementation-lanes-agent-turn: Update implementation lanes agent-turn status

- lane: docs
- priority: 3
- state: AssignedActive { worker: "worker-docs" }
- allowed_edit:
  - docs/active/agents/ploke-tree-graph-ingestion/implementation-lanes.md
  - docs/active/agents/ploke-tree-graph-ingestion/README.md
- forbidden_edit:
  - crates/ploke-eval
  - crates/ploke-tree
  - crates/ploke-records
  - crates/ploke-egui
- docs:
  - .orchestrator/graph-ingestion-wave2-workers/review-implementation-lanes-drift.report.md
  - .orchestrator/graph-ingestion-wave2-workers/repair-graph-agent-turn-weak-join.report.md
  - .orchestrator/graph-ingestion-wave2-workers/review-loader-agent-turn-scope-repair.report.md
  - .orchestrator/graph-ingestion-wave2-workers/retainer-records-feature-boundary-map.report.md
- acceptance:
  - implementation-lanes.md reflects current agent-turn owner/loader/graph progress and remaining feature-boundary gap
  - Provider/full-response remains separate unresolved family

## Queued Tasks

### docs-agent-turn-inventory: Update graph-ingestion inventory for agent-turn load and graph coverage

- lane: docs
- priority: 3
- state: AssignedQueued { worker: "worker-docs" }
- allowed_edit:
  - docs/active/agents/ploke-tree-graph-ingestion
  - docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md
- forbidden_edit:
  - crates
- docs:
  - docs/active/agents/ploke-tree-graph-ingestion/README.md
- acceptance:
  - Inventory reflects agent-turn typed owner, RunRecordSet loading, and Graph ingestion status
  - Handoff records verification and remaining gaps
  - No source code edits


## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
