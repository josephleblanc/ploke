# Worker Packet: worker-graph

- role: Worker

## Active Task

### repair-graph-agent-turn-weak-join: Repair graph agent-turn weak joins

- lane: graph
- priority: 3
- state: AssignedActive { worker: "worker-graph" }
- allowed_edit:
  - crates/ploke-tree/src/graph
- forbidden_edit:
  - crates/ploke-eval
  - crates/ploke-records
  - crates/ploke-tree/src/store
  - crates/ploke-egui
- docs:
  - .orchestrator/graph-ingestion-wave2-workers/review-graph-agent-turn-evidence.report.md
  - .orchestrator/graph-ingestion-wave2-workers/graph-agent-turn-evidence.report.md
- acceptance:
  - Ambiguous node-scoped agent-turn evidence is not attached as runtime/operation-specific evidence
  - History authority remains unchanged
  - Avoid cloning full runtime/operation coordinate vectors per artifact where practical

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
