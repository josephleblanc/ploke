# Worker Packet: reviewer

- role: Reviewer

## Active Task

### review-graph-agent-turn-weak-join-repair: Review graph agent-turn weak join repair

- lane: review
- priority: 3
- state: AssignedActive { worker: "reviewer" }
- forbidden_edit:
  - crates/ploke-eval
- docs:
  - .orchestrator/graph-ingestion-wave2-workers/repair-graph-agent-turn-weak-join.report.md
  - .orchestrator/graph-ingestion-wave2-workers/review-graph-agent-turn-evidence.report.md
- acceptance:
  - Read-only: verify ambiguous agent-turn evidence is no longer attached to runtime/operation nodes
  - Check History authority boundary and report exact lines

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
