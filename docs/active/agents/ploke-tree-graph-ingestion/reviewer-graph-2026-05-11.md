# reviewer-graph report 2026-05-11

Review scope: graph type split and current call sites around `ploke-tree::graph::Graph`, `OperationIndex`, History entries, allocation behavior, and graph centrality. Review only; no source edits.

## Findings

1. `OperationIndex` is public on `Graph` but has no ingestion path, so it is always empty. `Graph::from_records` ingests History, scheduler records, transition journal, and passive evidence at `crates/ploke-tree/src/graph/build.rs:26`, but no builder writes `graph.operations`. Scheduler nodes and runner requests already carry `operation_target` at `crates/ploke-records/src/scheduler.rs:126` and `crates/ploke-records/src/scheduler.rs:157`; `ingest_scheduler_node` only attaches node/artifact evidence at `crates/ploke-tree/src/graph/build/scheduler.rs:44`, and passive ingestion currently never handles `PassiveEvidence::run_attempts` at `crates/ploke-tree/src/graph/build/passive.rs:10`. Downstream consumers cannot rely on `Graph.operations` as a central operation index yet.

2. `OperationKey` can split one semantic operation into unrelated identities. `OperationKey::RuntimeTarget` keys by runtime plus target, while `OperationKey::HistoryEntry` keys by `EntryId` at `crates/ploke-tree/src/graph/types/operation.rs:14`; `OperationNode` then makes the coordinate optional at `crates/ploke-tree/src/graph/types/operation.rs:73`. The handoff defines `OperationCoordinate` as generator runtime plus target, with History admission as authority/evidence. Prefer keying operations by coordinate and attaching History entries as admission evidence, or adding an explicit join object; otherwise admitted and observed facts for the same operation can remain unjoined.

3. `ploke-tree::Graph` is not yet the central graph consumed by egui. `ploke-egui::import::graph_from_run_records` still builds `crate::graph::Graph` directly from `RunRecordSet` and `CoarseHistorySpine` at `crates/ploke-egui/src/import/mod.rs:34`. That keeps semantic graph assembly in the renderer-facing crate instead of projecting from `ploke_tree::Graph::from_records`.

4. Allocation/cloning concern: `OperationKey::from_coordinate` and `OperationTargetKey::from` clone ids and whole target vectors at `crates/ploke-tree/src/graph/types/operation.rs:24` and `crates/ploke-tree/src/graph/types/operation.rs:48`, while `OperationNode` can also retain the full `Coordinate` at `crates/ploke-tree/src/graph/types/operation.rs:75`. If this index starts ingesting node/run-attempt operations, each operation can store the same runtime/target data twice. Store only the key plus evidence/admission refs unless consumers need the original owned coordinate.

## Notes

- History entry placement looks correct: `HistoryEntryNode` and `HistoryPayloadKind` now live under `graph/types/history.rs`, not `operation.rs`.
- `Graph` remains the intended immutable read-side object in `ploke-tree`, but this is not fully enforced while public fields allow direct mutation and egui still assembles its own graph.

## Verification

- `cargo check -p ploke-tree`: passed.
- `cargo test -p ploke-tree graph:: 2>&1 | tail -n 80`: passed, 14 tests.
- `git diff --check -- crates/ploke-tree/src/graph/types.rs crates/ploke-tree/src/graph/types/history.rs crates/ploke-tree/src/graph/types/operation.rs`: passed.

Changed files: this report only.
