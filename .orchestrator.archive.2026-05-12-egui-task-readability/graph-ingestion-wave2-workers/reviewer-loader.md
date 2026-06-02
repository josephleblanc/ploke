# Worker Packet: reviewer-loader

- role: Reviewer

## Active Task

### review-records-feature-boundary-repair: Review records feature boundary repair

- lane: review
- priority: 3
- state: AssignedActive { worker: "reviewer-loader" }
- forbidden_edit:
  - crates/ploke-eval
- docs:
  - .orchestrator/graph-ingestion-wave2-workers/repair-agent-turn-record-feature-boundary.report.md
  - .orchestrator/graph-ingestion-wave2-workers/retainer-records-feature-boundary-map.report.md
- acceptance:
  - Read-only: verify passive agent-turn records no longer require tool-contracts and ploke-tree does not pull ploke-tui
  - Check RawValue use does not reintroduce production Value walking
  - Report exact findings and do not edit files

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
