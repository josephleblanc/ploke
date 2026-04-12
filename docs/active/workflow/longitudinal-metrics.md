# Longitudinal Metrics Ledger

- last_updated: 2026-04-12
- owning_branch: `refactor/tool-calls`
- review_cadence: update when a formal run lands or a metric definition changes
- update_trigger: update after formal run ingestion, schema changes, or capture/backfill changes
- source: [eval-design.md](../plans/evals/eval-design.md), [run-manifest.v0.draft.json](../../workflow/run-manifest.v0.draft.json), [S2A report](../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2A_longitudinal-metrics-report.md)

This is the central roll-up surface for longitudinal eval metrics. Numeric truth comes from immutable run artifacts; narrative workflow docs may explain gaps, but they are not numeric sources.

## Canonical inputs

- Primary numeric source: immutable run manifest.
- Secondary source when a field is only emitted there: run record / turn trace.
- Reconciliation source: execution log for provider selection or missing provenance.
- If a metric cannot be computed from immutable records, mark it blocked rather than estimating it.

## Metric Definitions

| metric | canonical formula | denominator | source expectation | status |
|---|---|---|---|---|
| `solve_rate` | `sum(solved_i) / N_formal` where `solved_i = 1` if `benchmark_verdict == passed`, else `0` | `N_formal` = formal runs in the reporting window | `outcome.benchmark_verdict`, `outcome.agent_outcome`, or `metrics.solve_rate_contribution` | derivable now |
| `token_cost` | `mean(prompt_tokens_i + completion_tokens_i)` | formal runs with both token fields present | `metrics.token_cost_input`, `metrics.token_cost_output`, or equivalent run-record fields | derivable now |
| `wall_time` | `mean(wall_clock_secs_i)` | formal runs with timing present | `timing.wall_clock_secs` | derivable now |
| `provider_failure_rate` | `sum(provider_failed_i) / N_formal` | `N_formal` | `outcome.run_status`, `failure_classification.primary`, provider provenance | derivable now |
| `setup_failure_rate` | `sum(setup_failed_i) / N_formal` | `N_formal` | `outcome.run_status`, `failure_classification.primary`, setup-phase capture | derivable now, with capture gaps in older runs |
| `tool_misuse_rate` | `sum(misuse_i) / N_tool_runs` where `misuse_i = 1` if a recorded tool call is classified as wrong-tool or invalid-arguments | runs with at least one tool call | turn-level tool-call classification with an explicit misuse flag | blocked |
| `recovery_rate` | `sum(recovered_i) / N_recoverable` where `recovered_i = 1` if the run later succeeds after a recoverable error | runs with at least one recoverable failure | turn-level failure/recovery markers and retry metadata | blocked |
| `attribution_coverage` | `sum(attributed_i) / N_failed_or_aborted` where `attributed_i = 1` if `failure_classification.primary` is present | failed or aborted formal runs | `failure_classification.primary` and `failure_classification.confidence` | derivable now |

## Reporting Rules

- Key every row by `experiment_id` + `manifest_id`.
- Keep one row per formal run; aggregate by experiment or arm only after the run rows are stable.
- Track missingness separately from the metric value.
- Do not backfill speculative values into blocked metrics.

## Current Gap Summary

- Derivable now: `solve_rate`, `token_cost`, `wall_time`, `provider_failure_rate`, `setup_failure_rate`, `attribution_coverage`.
- Blocked: `tool_misuse_rate`, `recovery_rate`.
- Next capture need: explicit turn-level misuse and recovery markers, or a small aggregation layer that derives them from recorded turn events.

## Ingestion And Refresh Path

Lightest-weight bootstrap path:

1. Discover newly completed formal runs from the experiment workspace.
2. Read the immutable run manifest as the canonical row source, falling back to the experiment summary artifact only for discovery or file location hints.
3. Materialize one append-only machine-readable row per formal run in [longitudinal-metrics.rows.jsonl](longitudinal-metrics.rows.jsonl), keyed by `experiment_id` + `manifest_id`.
4. Regenerate this markdown ledger from the JSONL companion so the human-readable surface stays synchronized with the machine-readable source.
5. Preserve blocked or missing fields explicitly instead of inferring them.

## Operational Assumptions

- Formal runs are immutable once recorded.
- Each formal run has stable `experiment_id` and `manifest_id` values.
- Discovery can start from the existing experiment directory layout without requiring CI/CD.
- The machine-readable companion is the durable source for roll-ups; the markdown ledger is the rendered view.
- If a metric or provenance field is missing, the row stays incomplete and the gap is recorded rather than guessed.

## Automation Boundaries

- Automatable now: detect new formal runs, append new JSONL rows, regenerate the markdown ledger, and emit a compact delta summary.
- Later: scheduled or event-driven refresh, automatic discovery across new workspaces, and direct publication of aggregate charts or dashboards.
- Still blocked: metrics that depend on turn-level misuse or recovery capture until that telemetry exists.

## Prototype Validation

- Sample run: [S2D sample JSONL row](../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2D_sample.rows.jsonl) and [S2D sample render](../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2D_sample.rendered.md)
- Proven now: a small real run sample can seed a companion row and regenerate a markdown view from the current run-directory artifacts.
- Still hypothetical: target-converged `experiment_id` / `manifest_id`, timing, token-cost, and turn-level misuse/recovery capture.
