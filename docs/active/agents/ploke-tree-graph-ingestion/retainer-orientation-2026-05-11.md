# Retainer Orientation - 2026-05-11

Task: `retainer-graph-ingestion-map`

Result: read-only orientation report.

## Current Source Of Truth

Use the newer inventory and current code over the older survey when they
disagree.

- `docs/active/agents/ploke-tree-graph-ingestion/README.md:24-41`
  defines the model: `ploke-records` owns passive shapes, `ploke-tree` owns
  typed loading and `Graph`, projections are not authority.
- `docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md:33-63`
  lists current graph files and inputs consumed by `Graph::from_records`.
- `docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md:77-151`
  is the compact surface tracker and current gaps.
- `docs/active/agents/ploke-tree-graph-ingestion/2026-05-11_3a2ea733-persisted-surface-survey.md:250-335`
  remains useful as a historical loader/surface survey, but is stale for
  `messages/child-plan/*` and `run-profile.*`.

## Graph Ingestion Ranges

- `crates/ploke-tree/src/graph/build.rs:24-33`
- `crates/ploke-tree/src/graph/build/history.rs:71-162`
- `crates/ploke-tree/src/graph/build/selection.rs:21-245`
- `crates/ploke-tree/src/graph/build/selection/membership.rs:1-106`
- `crates/ploke-tree/src/graph/build/passive.rs:9-157`
- `crates/ploke-tree/src/graph/build/scheduler.rs:10-61`
- `crates/ploke-tree/src/graph/build/journal.rs:12-120`

## Store Loader Ranges

- `crates/ploke-tree/src/store/record_set.rs:9-37`
- `crates/ploke-tree/src/store/evidence.rs:10-161`
- `crates/ploke-tree/src/store/fs.rs:33-113`
- `crates/ploke-tree/src/store/fs.rs:154-174`
- `crates/ploke-tree/src/store/fs.rs:256-440`

## Passive Record Ranges

- `crates/ploke-records/src/record.rs:8-35`
- `crates/ploke-records/src/child_plan.rs:1-46`
- `crates/ploke-records/src/run_profile.rs:1-39`
- `crates/ploke-records/src/run_profile.rs:186-200`
- `crates/ploke-records/src/scheduler.rs:117-238`
- `crates/ploke-records/src/invocation.rs:1-93`

## Worker Rules And Prior Reports

- `docs/active/agents/ploke-tree-graph-ingestion/orchestration.md:94-139`
- `docs/active/agents/ploke-tree-graph-ingestion/orchestration.md:166-190`
- `docs/active/agents/ploke-tree-graph-ingestion/implementation-lanes.md:76-130`
- `docs/active/agents/2026-05-11_ploke-egui-graph-import-boundary-handoff.md:53-90`
- `docs/active/agents/2026-05-11_ploke-egui-graph-import-boundary-handoff.md:180-207`

## Stale Assumptions Flagged

- Older survey rows saying `messages/child-plan/node-*.json` lacks passive
  owner/loader are stale. `ChildPlanRecord` exists and `FsRunStore` loads child
  plan evidence.
- Older survey rows saying `run-profile.toml` and
  `run-profile.commitment.json` lack passive owner/loader are stale.
  `RunProfileRecord`, `RunProfileCommitmentRecord`, loader, and graph metadata
  evidence exist.
- Older survey rows marking scheduler/node/parent/successor/journal summaries
  as loaded but not ingested are stale. Current graph attaches these as
  evidence.

Verification reported by retainer: read-only metadata and targeted reads only.
