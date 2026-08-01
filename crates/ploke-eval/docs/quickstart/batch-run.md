# Batch Run

Use this for a selected group of benchmark instances.

## Prepare a small batch

```bash
cargo run -p ploke-eval -- run prepare batch \
  --dataset-key ripgrep \
  --specific 2209
```

Expected state:

```text
~/.ploke-eval/batches/ripgrep-2209/batch.json
```

## Execute setup-only or agent turns

Setup-only:

```bash
cargo run -p ploke-eval -- run batch setup --batch-id ripgrep-2209
```

Agent path (live provider/model API for each attempted instance):

```bash
cargo run -p ploke-eval -- run batch agent --batch-id ripgrep-2209
```

Expected batch artifacts:

```text
~/.ploke-eval/batches/ripgrep-2209/
  batch.json
  batch-run-summary.json
  multi-swe-bench-submission.jsonl
```

## Trust note

A raw batch aggregate JSONL is convenient, but campaign export is the stronger
submission surface when a campaign closure state exists. See
[Campaign Lifecycle](../operations/campaign-lifecycle.md).

## State touched

| Step | Safety | State touched |
| --- | --- | --- |
| prepare batch | `FS` | batch manifest and per-instance manifests |
| batch setup | `FS` + maybe `NET` | per-instance setup artifacts and batch summary |
| batch agent | `FS` + `NET` + `LIVE-API` | per-instance agent artifacts and aggregate JSONL |
