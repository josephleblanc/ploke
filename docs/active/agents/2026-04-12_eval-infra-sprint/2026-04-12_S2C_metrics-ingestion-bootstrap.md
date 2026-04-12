# S2C - Metrics Ingestion And Auto-Rollup Bootstrap

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: H0
- Related hypothesis: Longitudinal metrics only become operationally useful once new formal runs can be discovered and rolled into a durable machine-readable layer with a regenerated summary surface
- Design intent: Define the lightest-weight path from formal run artifacts to persistent longitudinal metric updates without requiring full CI/CD adoption first
- Scope: Design a bootstrap ingestion loop for newly available formal runs, including candidate storage shape, run-discovery mechanism, regeneration/update flow, and where compact delta summaries should surface
- Non-goals: Do not implement the ingestion loop in this packet, do not require CI/CD adoption, do not redesign the run manifest or record schema
- Owned files: `docs/active/workflow/**`, `docs/workflow/**`, related sprint docs as needed
- Dependencies: `S2A` report, `S2B` ledger artifact
- Acceptance criteria:
  1. The packet output defines a concrete lightweight ingestion/update path from new run artifacts to the longitudinal metrics surface.
  2. The output proposes at least one durable machine-readable layer for roll-up data and explains why it fits the current workflow.
  3. The output identifies what can be automated now versus what remains blocked on missing capture, conventions, or infrastructure.
  4. The output connects the bootstrap path back to the measurement model in `eval-design.md`.
- Required evidence:
  - sampled source-artifact list
  - explicit proposed storage/update flow
  - note on automation assumptions and operational prerequisites
  - recommended follow-up packet for implementation or backfill
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required for doc-only design work.
