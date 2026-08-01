# Worker Packet: retainer

- role: Retainer
- refresh_after_questions: 5

## Active Task

### design-canonical-tool-transport-home: Map canonical passive tool transport ownership options

- lane: retainer
- priority: 3
- state: AssignedActive { worker: "retainer" }
- forbidden_edit:
  - crates/ploke-eval
- docs:
  - docs/active/agents/ploke-tree-graph-ingestion/reviewer-graph-tool-ui-contract-2026-05-11.md
  - docs/active/agents/ploke-tree-graph-ingestion/2026-05-11_3a2ea733-persisted-surface-survey.md
- acceptance:
  - Read-only report identifies current owners, dependency edges, and lowest-drift shared-home option without copying DTOs

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
