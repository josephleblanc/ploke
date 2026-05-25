# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `19561dec186b`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/diff/mod.rs`

unrelated dirty paths:
- `crates/test-utils/src/fixture_dbs.rs`
- `crates/test-utils/src/lib.rs`
- `docs/how-to/recreate-backup-db-fixtures.md`
- `docs/testing/BACKUP_DB_FIXTURES.md`
- `xtask/src/main.rs`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-19561dec186b-standard/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-inspector-section-sequence-allocation.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 7244 ns
- `FsRunStore::load`: 929360266 ns
- `FsRunStore::load_history_blocks`: 24051195 ns
- `FsRunStore::load_transition_journal`: 4503970 ns
- `FsRunStore::load_record_set`: 957919759 ns
- `compressed_run_record_profile_probe`: 116897232 ns
- `Graph::from_records`: 6780270 ns
- `graph_load_total`: 1081599837 ns
- note: run_picker_discovery=7244 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_sections_sequence_30`: frames=310, median=1970678 ns, p95=2495892 ns, p99=23807329 ns, max=88130162 ns, heap_slope=growing
  - component `capture_overhead`: median=3076 ns, median_frame_share=0.15%
  - component `central_graph`: median=993861 ns, median_frame_share=50.43%
  - component `diagnostics`: median=70422 ns, median_frame_share=3.57%, nested_under=run_navigation
  - component `run_navigation`: median=284432 ns, median_frame_share=14.43%
  - component `selection_inspector`: median=471683 ns, median_frame_share=23.93%
  - component `timeline`: median=53039 ns, median_frame_share=2.69%
  - component `top_strip`: median=78517 ns, median_frame_share=3.98%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-19561dec186b-standard/standard.puffin`: 980774 bytes, sha256 `f1c05c121a36f76654c372052e233702e8142a9ae2277700ad4f1d9e0fe956e2`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-19561dec186b-standard/inspector_sections_sequence_30.heap.json`: 9545 bytes, sha256 `d2edac4026de1b973d7ab9d41af8f3fec22f07ecad243f8941e4526ebb222001`

See `report.json` for typed timings and compact allocation summaries.
