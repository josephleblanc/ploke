# Persisted Files

This page summarizes major persisted file roots. It is intentionally organized
by operator use, not by Rust module.

## Default home

```text
~/.ploke-eval/
  datasets/
  repos/
  models/
  instances/
  batches/
  campaigns/
```

## Runs

```text
~/.ploke-eval/instances/<instance-id>/
  run.json
  config/
  runs/run-<timestamp>-<arm>-<suffix>/
    repo-state.json
    execution-log.json
    indexing-status.json
    snapshot-status.json
    multi-swe-bench-submission.jsonl
    record.json.gz
```

Run telemetry is local evidence. Official benchmark pass/fail comes from the
external evaluator over exported submission JSONL.

## Batches

```text
~/.ploke-eval/batches/<batch-id>/
  batch.json
  batch-run-summary.json
  multi-swe-bench-submission.jsonl
```

Batch aggregate JSONL is convenient but weaker than campaign export when closure
state is available.

## Campaigns

```text
~/.ploke-eval/campaigns/<campaign>/
  campaign.json
  closure-state.json
  multi-swe-bench-submission.jsonl
```

Campaign export reads completed closure rows and per-run submission artifacts.

## Prototype 1

```text
~/.ploke-eval/campaigns/<campaign>/prototype1/
  run-profile.toml
  run-profile.commitment.json
  scheduler.json
  branches.json
  transition-journal.jsonl
  history/
  messages/
  evaluations/
  nodes/
```

See the [Operator Map](../prototype1/operator-map.md) for Prototype 1
operator context, and [Persistence Inventory and Cozo Map](../prototype1/persistence-inventory-and-cozo-map.md)
for the detailed location inventory and database migration map.
