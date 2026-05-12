# Worker Packet: reviewer-graph

- role: Reviewer

## Active Task

### review-tool-transport-home-risks: Review risks for canonical passive tool transport home

- lane: review
- priority: 3
- state: AssignedActive { worker: "reviewer-graph" }
- forbidden_edit:
  - crates/ploke-eval
- docs:
  - docs/active/agents/ploke-tree-graph-ingestion/reviewer-graph-tool-ui-contract-2026-05-11.md
- acceptance:
  - Read-only risk report covers API drift, dependency hygiene, serde compatibility, and no-copy invariant

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
