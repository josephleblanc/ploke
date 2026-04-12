# S2A Longitudinal Metrics Report

## implemented

- `docs/active/plans/evals/eval-design.md` already defines the outcome vs validity split.
- `docs/workflow/run-manifest.v0.draft.json` already has a `metrics` block, but only for a narrow per-run slice.
- `docs/workflow/experiment-config.v0.draft.json` already names analysis metrics and validity guards.
- `docs/active/workflow/hypothesis-registry.md` maps hypotheses to metric families.
- `docs/active/workflow/evidence-ledger.md` and `docs/active/workflow/recent-activity.md` already carry operational beliefs and current-state claims.

## claims

- Minimal longitudinal metric set:
  - outcome: `solve_rate`, `token_cost`, `wall_time`
  - validity/health: `provider_failure_rate`, `setup_failure_rate`, `tool_misuse_rate`, `recovery_rate`, `attribution_coverage`
- Central reporting pattern:
  - add one living roll-up doc under `docs/active/workflow/`, ideally `longitudinal-metrics.md`
  - one row per formal run or experiment, keyed by `experiment_id` + `manifest_id`
  - each row links the immutable run manifest and the source artifact set

## evidence

- The design doc explicitly treats `solve_rate`, `token_cost`, and `wall_time` as the primary outcome metrics.
- The design doc also lists validity metrics that matter for interpretation, including provider failures, tool failures, recovery, index coverage, replay completeness, and attributable-failure rates.
- The draft experiment config already uses `solve_rate` as primary and `token_cost`, `wall_time`, `tool_misuse_rate`, and `recovery_rate` as analysis metrics, with validity guards on provider/setup failures.
- The draft run manifest already exposes per-run timing, outcome, token, turn, and failure-class fields.

## unsupported_claims

- There is no single canonical longitudinal metrics ledger in the current docs.
- The current draft manifest does not yet prove that live runs populate every field needed for roll-up.
- `tool_misuse_rate`, `recovery_rate`, and `attribution_coverage` are named in docs, but their exact formulas are not yet standardized here.
- No automatic aggregation/reporting job was verified in this packet.

## not_checked

- Concrete run artifacts such as `record.json.gz`, `summary.json`, or any live experiment directory.
- Any code that might already compute or export longitudinal aggregates.
- Whether a central metrics doc already exists outside the paths read here.

## risks

- Metric drift between design docs, draft schemas, and live run artifacts can produce contradictory reporting.
- Outcome metrics alone can hide setup or provider instability.
- Without one central roll-up, different agents may report different denominators for the same metric.
- Some validity metrics need explicit capture/aggregation work before they are trustworthy over time.

## next_step

- Create the central longitudinal metrics ledger and define formulas/denominators for the minimal set above, then backfill a small sample of formal runs to confirm the metrics are derivable end-to-end.
