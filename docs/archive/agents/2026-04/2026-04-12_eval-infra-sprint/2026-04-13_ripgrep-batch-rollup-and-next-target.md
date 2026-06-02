# Ripgrep Batch Rollup And Next Target

- date: 2026-04-13
- mode: surgical implementation note
- control_plane: [2026-04-12_eval-infra-sprint-control-plane.md](./2026-04-12_eval-infra-sprint-control-plane.md)
- workstream: `A2` / `A3` / `A4` / `H0`
- scope: close the ripgrep batch execution loop, record the artifact state honestly, and leave the next-target expansion path explicit

## Why This Exists

We needed a durable restart note after moving from one-off ripgrep treatment runs to a repo-wide batch execution attempt under time pressure.

The main operational problem was not missing run artifacts. It was batch-level bookkeeping drift caused by reusing a dirty batch id and then hitting an interrupted rerun before the batch summary rewrite completed.

## What Actually Completed

The ripgrep run set is now materially complete at the per-run level.

Completed run directories with full agent artifacts now exist for:

- `BurntSushi__ripgrep-1294`
- `BurntSushi__ripgrep-2626`
- `BurntSushi__ripgrep-2610`
- `BurntSushi__ripgrep-2576`
- `BurntSushi__ripgrep-2488`
- `BurntSushi__ripgrep-2295`
- `BurntSushi__ripgrep-2209`
- `BurntSushi__ripgrep-1980`
- `BurntSushi__ripgrep-1642`
- `BurntSushi__ripgrep-1367`
- `BurntSushi__ripgrep-954`
- `BurntSushi__ripgrep-727`
- `BurntSushi__ripgrep-723`
- `BurntSushi__ripgrep-454`

For these runs, the expected per-run artifacts are present:

- `run.json`
- `execution-log.json`
- `record.json.gz`
- `agent-turn-summary.json`
- `multi-swe-bench-submission.jsonl`
- `final-snapshot.db`
- `llm-full-responses.jsonl`

## What Went Wrong Operationally

Two batch surfaces were used:

1. `ripgrep-all`
2. `ripgrep-remaining-r1`

`ripgrep-all` was first run with only one prepared manifest on disk, so its original `batch-run-summary.json` recorded one success and many missing-manifest failures.

After the missing `run.json` files were prepared, `ripgrep-all` was rerun on the same batch id. That rerun advanced through multiple real instances, but `run_batch()` only writes `batch-run-summary.json` after the full loop completes. When the rerun was interrupted mid-batch, the old stale summary remained in place.

So the trusted truth split is:

- per-run directories under `~/.ploke-eval/runs/<instance>/`: trustworthy
- `~/.ploke-eval/batches/ripgrep-all/batch-run-summary.json`: stale for the rerun

The clean recovery path was `ripgrep-remaining-r1`, which produced a fresh batch summary for the remaining target slice.

## Trusted Batch-Level Artifacts

The clean batch-level artifact to trust is:

- [ripgrep-remaining-r1 batch summary](/home/brasides/.ploke-eval/batches/ripgrep-remaining-r1/batch-run-summary.json)

That summary reports:

- attempted: `9`
- succeeded: `9`
- failed: `0`
- stopped_early: `false`

This summary only covers the remainder batch, not the entire 14-instance ripgrep set.

## Snapshot DB Locations

The snapshot/backup DBs needed for downstream website work are per-run artifacts, not central batch assets.

For each completed run, use:

- `~/.ploke-eval/runs/<instance>/final-snapshot.db`
- `~/.ploke-eval/runs/<instance>/indexing-checkpoint.db`
- `~/.ploke-eval/runs/<instance>/snapshot-status.json`

Example:

- [BurntSushi__ripgrep-1294 final snapshot](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1294/final-snapshot.db)
- [BurntSushi__ripgrep-2626 final snapshot](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/final-snapshot.db)
- [BurntSushi__ripgrep-454 final snapshot](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/final-snapshot.db)

## Residual Caveats

- Some runs observed transient OpenRouter transport/body-decode timeouts during execution.
- Those runs still unwound and wrote final artifacts, so they are usable for artifact-driven downstream work.
- The raw full-response sidecar still undercounts final-stop usage on some runs and should not yet be treated as authoritative cost.

## Next Target Expansion Path

The next step is not more ripgrep repair.

The next safe orchestrator move is:

1. choose the second repo target
2. do one reviewed sentinel/probe run
3. add one row to [target-capability-registry.md](../../workflow/target-capability-registry.md) from that evidence
4. note the classification in [recent-activity.md](../../workflow/handoffs/recent-activity.md)
5. prepare a fresh batch id for that repo
6. run the second repo batch

Minimum rule:

- do not reuse dirty batch ids for new execution attempts when the previous summary state is ambiguous

## Verification

- The clean recovery batch [ripgrep-remaining-r1 batch summary](/home/brasides/.ploke-eval/batches/ripgrep-remaining-r1/batch-run-summary.json) reports `9/9` succeeded
- The final straggler `BurntSushi__ripgrep-454` completed via direct single-run fallback and wrote `execution-log.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl`, `multi-swe-bench-submission.jsonl`, and `final-snapshot.db`
- A filesystem rollup over `~/.ploke-eval/runs/BurntSushi__ripgrep-*` confirms that all 14 ripgrep instances now have full run artifacts
