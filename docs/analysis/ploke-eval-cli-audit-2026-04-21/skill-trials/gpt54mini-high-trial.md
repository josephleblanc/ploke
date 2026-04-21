# ploke-eval CLI audit, 2026-04-21

## Commands I would run

- One-instance eval: `fetch-msb-repo`, `prepare-msb-single`, `run-msb-agent-single`.
- If model/provider selection matters, check `model current` and `model providers <model-id>`, then pin with `--provider`.
- Batch/family progress: `campaign show`, `closure status`, and `closure advance eval` or `closure advance all`.
- Export: `campaign export-submissions --campaign <campaign> [--nonempty-only] [--output <path>]`.

## Where outputs appear to live

- Top-level defaults say `PLOKE_EVAL_HOME=~/.ploke-eval`.
- Datasets live under `~/.ploke-eval/datasets`.
- Repo checkouts live under `~/.ploke-eval/repos`.
- Per-instance runs live under `~/.ploke-eval/runs/<instance>/`.
- Batch artifacts live under `~/.ploke-eval/batches/<batch-id>/`.
- `prepare-msb-single` writes `~/.ploke-eval/runs/<instance>/run.json`.
- `prepare-msb-batch` writes per-instance `run.json` files plus `~/.ploke-eval/batches/<batch-id>/batch.json`.
- `run-msb-agent-single` says it writes a turn trace and summary beside the run artifacts.
- `run-msb-batch` says it writes `batch-run-summary.json` and `multi-swe-bench-submission.jsonl`.
- `campaign init` / `campaign show` place campaign state under `~/.ploke-eval/campaigns/<campaign>/`.
- `campaign export-submissions` accepts `--output <path>`, but the help text does not state a default file path.

## What seems trustworthy

- Most trustworthy: per-run artifacts in the chosen run directory.
- Next: `campaign export-submissions` output from completed runs in campaign closure state.
- Then: closure state for campaign progress.
- Less trustworthy: batch aggregate `multi-swe-bench-submission.jsonl`.
- Least trustworthy: terminal summary strings.
- The CLI is explicit that local `ploke-eval` artifacts are telemetry, not the benchmark verdict; the external evaluator decides official pass/fail.

## What `registry`, `campaign`, and `closure` seem to be

- `registry`: the local typed benchmark target registry; `recompute` refreshes it from dataset sources and `status` prints the persisted registry.
- `campaign`: the operator-facing manifest/config layer used by closure-driven workflows; it can list, show, validate, and export submissions.
- `closure`: the derived progress layer for a campaign; it tracks staged coverage across registry, eval, and protocol artifacts, and can `recompute`, report `status`, or `advance`.

## What remains unclear or awkward

- The help surface does not spell out the exact on-disk files for `registry` and `closure` state.
- `campaign export-submissions` has an `--output` flag but no documented default destination.
- `run-msb-batch` / `run-msb-agent-batch` write a submission JSONL, but the help only implies where that file lands.
- `closure advance` is conceptually clear but still a bit broad: it bundles eval and protocol advancement without defining the exact artifact boundary in the help text.
- I would still ask: where exactly are the persisted registry and closure files stored, and which output file should be treated as canonical for campaign export when `--output` is omitted?
