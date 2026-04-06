# ploke-eval

Minimal benchmark/eval runner scaffolding for `ploke`.

Current scope:
- fetch a benchmark repo into a stable local cache
- prepare one run manifest from a Multi-SWE-bench instance
- execute one prepared run
- reset the repo to the benchmark base commit
- index the repo with `ploke`
- save a DB snapshot after indexing

## Default layout

By default, `ploke-eval` uses:

```text
~/.ploke-eval/
  datasets/   downloaded Multi-SWE-bench JSONL files
  repos/      benchmark repo checkouts
  runs/       per-instance run manifests and artifacts
```

Override the root with:

```bash
PLOKE_EVAL_HOME=/some/path
```

## Quick start

Current single-run example for `ripgrep`:

```bash
cargo run -p ploke-eval -- fetch-msb-repo --dataset-key ripgrep
cargo run -p ploke-eval -- prepare-msb-single --dataset-key ripgrep --instance BurntSushi__ripgrep-2209
cargo run -p ploke-eval -- run-msb-single --instance BurntSushi__ripgrep-2209
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
```

The run directory contains:

```text
run.json               prepared run manifest
repo-state.json        repo state after checkout to base_sha
execution-log.json     high-level run steps
indexing-status.json   indexing result summary
snapshot-status.json   saved DB snapshot summary
config/                per-run XDG config sandbox used by SaveDb
```

## Current embedding preset

`run-msb-single` currently uses a hardcoded OpenRouter embedding preset:

```text
model: mistralai/codestral-embed-2505
dims:  1536
```

This expects `OPENROUTER_API_KEY` to be present in the environment.
