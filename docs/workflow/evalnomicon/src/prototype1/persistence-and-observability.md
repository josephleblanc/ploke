# Persistence and Observability

Prototype 1 currently writes several evidence families rather than one uniform record. This page names the intended documentation boundary; detailed inventories belong in reference appendices.

## Evidence families

The current loop writes or references:

- loop control state under `~/.ploke-eval/campaigns/<campaign>/prototype1/`
- campaign and run evidence under `~/.ploke-eval/campaigns/<campaign>/` and `~/.ploke-eval/instances/...`
- protocol artifacts under run directories
- local sealed History under `prototype1/history/`
- diagnostic logs under campaign node streams and `~/.ploke-eval/logs/`
- artifact-carried identity inside realized parent/child worktrees

## Projection warning

A file can be useful without being authoritative. In particular:

- `scheduler.json` is a mutable legacy projection and path/index context.
- `branches.json` preserves branch/candidate/evaluation evidence but is not Crown authority.
- `transition-journal.jsonl` is append-only transition evidence, not sealed History.
- History indexes and heads are rebuildable projections over sealed blocks.
- monitor reports and dashboards are operator views.

## Message boxes

Cross-runtime communication should be documented as typed boxes, not arbitrary files:

```text
Box = (Lock transition, Unlock transition, File schema)
```

Each mutable buffer should name:

- owner role/state
- allowed readers
- concrete schema
- transition that justifies each write
- transition that justifies each read

## Reference docs to promote

- `docs/workflow/evalnomicon/drafts/persistence/map-2026-05-03/synthesis.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/README.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/record-surface-map.md`
