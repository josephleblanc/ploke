# ploke-eval

Minimal benchmark/eval runner scaffolding for `ploke`.

Current scope:
- fetch a benchmark repo into a stable local cache
- prepare one run manifest from a Multi-SWE-bench instance
- prepare one batch manifest plus many per-instance manifests from Multi-SWE-bench
- execute one prepared run
- execute one prepared batch
- inspect available OpenRouter providers for a model
- reset the repo to the benchmark base commit
- index the repo with `ploke`
- save a DB snapshot after indexing

## Default layout

By default, `ploke-eval` uses:

```text
~/.ploke-eval/
  datasets/   downloaded Multi-SWE-bench JSONL files
  repos/      benchmark repo checkouts
  models/     model registry, active model, provider preferences
  runs/       per-instance run manifests and artifacts
  batches/    batch manifests, summaries, aggregate JSONL exports
```

Provider preferences are stored in `~/.ploke-eval/models/provider-preferences.json`.

Override the root with:

```bash
PLOKE_EVAL_HOME=/some/path
```

## Assistant transcript

Print only assistant messages from the most recent completed run:

```bash
cargo run -p ploke-eval -- transcript
```

The last completed run is recorded in `~/.ploke-eval/last-run.json`.

List providers available for a model:

```bash
cargo run -p ploke-eval -- model providers
```

Pin a provider for a run:

```bash
cargo run -p ploke-eval -- run-msb-agent-single --instance BurntSushi__ripgrep-2209 --provider chutes
```

## Quick start

Current single-run example for `ripgrep`:

```bash
cargo run -p ploke-eval -- fetch-msb-repo --dataset-key ripgrep
cargo run -p ploke-eval -- prepare-msb-single --dataset-key ripgrep --instance BurntSushi__ripgrep-2209
cargo run -p ploke-eval -- run-msb-single --instance BurntSushi__ripgrep-2209
```

Batch example for `ripgrep`:

```bash
cargo run -p ploke-eval -- prepare-msb-batch --dataset-key ripgrep --specific 2209
cargo run -p ploke-eval -- run-msb-agent-batch --batch-id ripgrep-2209
```

## What gets read and written

For the example above, the defaults are:

```text
dataset jsonl:
  ~/.ploke-eval/datasets/BurntSushi__ripgrep_dataset.jsonl

repo checkout:
  ~/.ploke-eval/repos/BurntSushi/ripgrep

run directory:
  ~/.ploke-eval/runs/BurntSushi__ripgrep-2209

batch directory:
  ~/.ploke-eval/batches/ripgrep-2209
```

The run directory contains:

```text
run.json               prepared run manifest
repo-state.json        repo state after checkout to base_sha
execution-log.json     high-level run steps
indexing-status.json   indexing result summary
snapshot-status.json   saved DB snapshot summary
multi-swe-bench-submission.jsonl
                      benchmark-ready JSONL patch record with org/repo/number/fix_patch
config/                per-run XDG config sandbox used by SaveDb
```

The batch directory contains:

```text
batch.json                      prepared batch manifest
batch-run-summary.json         per-instance success/failure summary
multi-swe-bench-submission.jsonl
                               aggregate JSONL patch records for the batch
```

`multi-swe-bench-submission.jsonl` is a candidate patch artifact for the
official Multi-SWE-bench evaluator. Local `ploke-eval` artifacts such as
`agent-turn-summary.json`, `execution-log.json`, and `patch_artifact` are run
telemetry, not the benchmark source of truth. Official pass/fail comes from
running the external Multi-SWE-bench evaluator on the exported submission.

## Current embedding preset

`run-msb-single` currently uses a hardcoded OpenRouter embedding preset:

```text
model: mistralai/codestral-embed-2505
dims:  1536
```

This expects `OPENROUTER_API_KEY` to be present in the environment.
