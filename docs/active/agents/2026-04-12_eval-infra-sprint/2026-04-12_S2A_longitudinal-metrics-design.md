# S2A - Longitudinal Metrics Design

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: H0
- Related hypothesis: We need a coherent change-over-time view of validity and outcome metrics so eval work can be steered by evidence rather than isolated run inspection
- Design intent: Define the minimal central metrics layer for tracking changes over time across eval runs, starting from existing artifacts and workflow docs
- Scope: Inventory existing metric sources, define the minimal metric set, and propose the central document or artifact shape for longitudinal reporting
- Non-goals: Do not implement full aggregation code in this packet, do not retroactively normalize every historical run, do not block primary-lane work
- Owned files: `docs/active/workflow/evidence-ledger.md`, `docs/active/workflow/priority-queue.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/workflow/run-manifest.v0.draft.json`, `docs/workflow/experiment-config.v0.draft.json`, existing eval artifact docs as needed
- Dependencies: none
- Acceptance criteria:
  1. The packet identifies the minimal metric set to track over time, including both outcome and validity/health metrics.
  2. The packet proposes one clear central location or artifact pattern for reporting those metrics.
  3. The packet distinguishes metrics already derivable from current artifacts from metrics that need new capture or aggregation work.
- Required evidence:
  - source inventory of current metric-bearing artifacts
  - proposed minimal metric set with rationale
  - explicit note on missing data or aggregation blockers
  - recommended next packet(s) for implementation or backfill
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required for doc-only analysis.
