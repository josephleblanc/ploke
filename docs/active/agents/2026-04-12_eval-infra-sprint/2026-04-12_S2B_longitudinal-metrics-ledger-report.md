# S2B Longitudinal Metrics Ledger Report

## implemented

- Created [docs/active/workflow/longitudinal-metrics.md](../../workflow/longitudinal-metrics.md) as the central longitudinal metrics ledger.
- Added explicit formulas, denominators, and source expectations for the minimal metric set.
- Split the metric set into `derivable now` versus `blocked` so the ledger does not blur missing capture with missing aggregation.
- Added a README pointer and a recent-activity note so the ledger is discoverable from the workflow landing page and the live board.

## claims

- The workflow now has one central artifact for longitudinal metrics instead of relying on scattered references.
- The ledger defines a practical roll-up contract keyed by `experiment_id` + `manifest_id`.
- The ledger keeps numeric sources narrow: immutable run artifacts first, narrative workflow docs only for explanation.
- `solve_rate`, `token_cost`, `wall_time`, `provider_failure_rate`, `setup_failure_rate`, and `attribution_coverage` are derivable from current artifacts or current run-record fields.
- `tool_misuse_rate` and `recovery_rate` remain blocked until turn-level misuse and recovery capture is standardized.

## evidence

- `docs/active/agents/2026-04-12_eval-infra-sprint/2026-04-12_S2A_longitudinal-metrics-report.md` identified the minimal metric set and the need for a single central roll-up doc.
- `docs/active/plans/evals/eval-design.md` distinguishes outcome metrics from validity/health metrics and names the relevant metric families.
- `docs/workflow/run-manifest.v0.draft.json` provides the target manifest fields used for per-run formulas.
- `docs/active/workflow/README.md` now points to the ledger as a live workflow artifact.
- `docs/active/workflow/handoffs/recent-activity.md` now records the ledger creation in the active board.

## unsupported_claims

- I did not verify any live run rows against the ledger.
- I did not implement aggregation code or a backfill job.
- I did not standardize turn-level misuse or recovery capture in production code.

## not_checked

- Whether current historical runs already contain enough turn-level detail to populate `tool_misuse_rate` and `recovery_rate` manually.
- Whether any other workflow doc outside the owned set already duplicates this ledger pattern.
- Whether a future automation layer should live in docs only or be paired with code support.

## risks

- If the capture layer never standardizes misuse and recovery markers, the two blocked metrics will stay manually interpreted.
- If future run artifacts drift from the draft manifest, the ledger formulas can become stale unless the doc is updated with schema changes.
- Manual roll-ups can still diverge unless formal runs are keyed consistently by `experiment_id` and `manifest_id`.

## next_step

- Backfill a small sample of formal runs against the new ledger and then open the next packet for capture/aggregation work on `tool_misuse_rate` and `recovery_rate`.
