# Batch Lifecycle

A batch is a manifest plus many per-instance runs.

## Prepare

```bash
cargo run -p ploke-eval -- run prepare batch \
  --dataset-key ripgrep \
  --specific 2209
```

Writes:

```text
~/.ploke-eval/batches/<batch-id>/batch.json
```

and per-instance run manifests under `instances/`.

## Execute

Setup-only:

```bash
cargo run -p ploke-eval -- run batch setup --batch-id ripgrep-2209
```

Agent turns (live provider/model API for each attempted instance):

```bash
cargo run -p ploke-eval -- run batch agent --batch-id ripgrep-2209
```

## Outputs

```text
~/.ploke-eval/batches/<batch-id>/
  batch.json
  batch-run-summary.json
  multi-swe-bench-submission.jsonl
```

The batch aggregate JSONL is convenient, but campaign export is preferred when a
campaign closure records completed runs.
