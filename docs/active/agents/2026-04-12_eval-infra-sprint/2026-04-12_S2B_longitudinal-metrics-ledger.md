# S2B - Longitudinal Metrics Ledger And Formula Definition

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: H0
- Related hypothesis: Longitudinal metrics only steer the eval programme if one central artifact defines the tracked metrics and their denominators
- Design intent: Turn the accepted S2A report into a concrete, central reporting artifact with explicit formulas and source-artifact links
- Scope: Create the initial central longitudinal metrics ledger doc under `docs/active/workflow/` and define the minimal metric set, formulas, and source expectations
- Non-goals: Do not implement aggregation code, do not backfill all historical runs, do not redesign manifests
- Owned files: `docs/active/workflow/**`, `docs/workflow/run-manifest.v0.draft.json`, related workflow docs as needed
- Dependencies: `S2A` report
- Acceptance criteria:
  1. A single central workflow artifact exists for longitudinal eval metrics.
  2. The minimal metric set has explicit formulas/denominators and source-artifact expectations.
  3. The artifact distinguishes metrics derivable now from metrics blocked on new capture or aggregation work.
- Required evidence:
  - targeted diff summary
  - the exact central artifact path created or updated
  - explicit note on what remains unautomated
  - recommended next packet for code or backfill work
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required for doc-only work.
