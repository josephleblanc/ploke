# `ploke-tree`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-tree` builds read-only projections over passive Prototype 1 records. It turns loaded records into `RunForest`, history-spined `Graph`, playback, and artifact-tree views for UI/browser/analysis consumers.

## Responsibilities

- Load scheduler, node, successor, history, journal, agent-turn, protocol, and passive evidence records from a run root.
- Assemble a `RunForest` from scheduler-like records plus evidence/diagnostics.
- Build a semantic `Graph` where sealed History is the primary lineage authority and other records attach as evidence.
- Provide playback projections for coarse/fine history and turn event drilldown.
- Preserve read-only boundaries: deserialization/projection does not advance runtime state or validate authority transitions.

## Key Files

- [`crates/ploke-tree/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tree/src/lib.rs) — crate docs, `RunForest`, projection entry points, public re-exports.
- [`crates/ploke-tree/src/store/fs.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tree/src/store/fs.rs) — `FsRunStore`, filesystem loader for run roots and passive evidence.
- [`crates/ploke-tree/src/store/record_set.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tree/src/store/record_set.rs) — `RunForestInput`, `RunRecordSet`, `RunRootSummary`.
- [`crates/ploke-tree/src/graph/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tree/src/graph/mod.rs) — graph module boundary and semantic read-boundary docs.
- [`crates/ploke-tree/src/graph/build.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tree/src/graph/build.rs) — `Graph::from_records` and builder ingestion passes.
- [`crates/ploke-tree/src/graph/types.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tree/src/graph/types.rs) — `Graph`, indexes, graph-level evidence views, and accessors.

## Public API

- `RunForest::from_records(input)` and `assemble_run_forest(input)` — build UI-facing scheduler/tree projection.
- `FsRunStore::new(run_root)` plus `load`, `load_record_set`, `load_run_root_summary`, `load_forest`.
- `Graph::from_records(&RunRecordSet)` — build immutable history/evidence graph.
- Graph accessors such as `agent_turn_records`, `run_attempts`, `protocol_artifacts`, `run_records`, `closure`, `eval_protocol_evidence`, and `invocations`.
- Playback exports such as `build_coarse_history_spine`, `project_coarse_history_spine`, and fine/coarse run playback helpers.

## Internal Structure

`FsRunStore` reads machine-readable files such as `scheduler.json`, `nodes/*/node.json`, `successor-ready`, `successor-completion`, history segment JSONL files, transition journals, agent-turn records, and passive evidence. `RunForest` merges scheduler and node records, attaches parent/successor evidence, and reports diagnostics for missing or mismatched records. `Graph::from_records` uses a builder to ingest history, scheduler records, transition journal, and passive evidence, then attaches `RunForest` and agent-turn records.

## Dependencies

- **Uses:** `ploke-records`, `serde`, filesystem JSON/JSONL loading, ordered maps/sets.
- **Used by:** `ploke-egui`, `ploke-tree-browser`, `ploke-tree-egui`, eval/protocol dashboards, and future analysis projections.

## Notable Patterns / Gotchas

- `ploke-tree` intentionally does not mutate loop state, parse human output into authority, or infer History authority from passive evidence.
- `Graph` is not a visual DTO; UI crates should borrow/derive from it rather than reconstructing loop semantics.
- Scheduler records, branch logs, tool traces, provider artifacts, and protocol artifacts attach as evidence; sealed History remains the primary ordering/authority spine.
