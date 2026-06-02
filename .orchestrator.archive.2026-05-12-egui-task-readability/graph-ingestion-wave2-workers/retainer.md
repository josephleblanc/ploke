# Worker Packet: retainer

- role: Retainer
- refresh_after_questions: 5

## Active Task

### retainer-records-feature-boundary-map: Map records feature boundary repair

- lane: retainer
- priority: 3
- state: AssignedActive { worker: "retainer" }
- forbidden_edit:
  - crates/ploke-eval
- docs:
  - .orchestrator/graph-ingestion-wave2-workers/review-records-agent-turn-passive-shapes.report.md
- acceptance:
  - Read-only: map current ploke-records feature dependency graph and smallest feature split for passive agent-turn records
  - Report exact Cargo/module changes to inspect; do not edit files

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
