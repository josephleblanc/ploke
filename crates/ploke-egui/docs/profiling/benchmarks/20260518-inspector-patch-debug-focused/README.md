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

- `run_picker_discovery`: 7023 ns
- `FsRunStore::load`: 915016589 ns
- `FsRunStore::load_history_blocks`: 23755139 ns
- `FsRunStore::load_transition_journal`: 4557160 ns
- `FsRunStore::load_record_set`: 943333537 ns
- `compressed_run_record_profile_probe`: 116497610 ns
- `Graph::from_records`: 6850714 ns
- `graph_load_total`: 1066684286 ns
- note: run_picker_discovery=7023 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `patch_debug_cold_300`: frames=300, median=1965079 ns, p95=2262264 ns, p99=2629813 ns, max=182599761 ns, heap_slope=plateau
  - component `capture_overhead`: median=3106 ns, median_frame_share=0.15%
  - component `central_graph`: median=995694 ns, median_frame_share=50.66%
  - component `diagnostics`: median=69971 ns, median_frame_share=3.56%, nested_under=run_navigation
  - component `run_navigation`: median=281537 ns, median_frame_share=14.32%
  - component `selection_inspector`: median=478346 ns, median_frame_share=24.34%
  - component `timeline`: median=51797 ns, median_frame_share=2.63%
  - component `top_strip`: median=75962 ns, median_frame_share=3.86%
- `patch_debug_warm_300`: frames=300, median=1981349 ns, p95=2324662 ns, p99=2549382 ns, max=3142212 ns, heap_slope=plateau
  - component `capture_overhead`: median=3176 ns, median_frame_share=0.16%
  - component `central_graph`: median=998349 ns, median_frame_share=50.38%
  - component `diagnostics`: median=69941 ns, median_frame_share=3.52%, nested_under=run_navigation
  - component `run_navigation`: median=283581 ns, median_frame_share=14.31%
  - component `selection_inspector`: median=478866 ns, median_frame_share=24.16%
  - component `timeline`: median=51707 ns, median_frame_share=2.60%
  - component `top_strip`: median=76173 ns, median_frame_share=3.84%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-inspector-patch-debug-focused/standard.puffin`: 1939327 bytes, sha256 `6557046ba3aaf6c04429bcd686fe16aa9af013e5504d306423f8479db0cf329e`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-patch-debug-focused/patch_debug_cold_300.heap.json`: 7502 bytes, sha256 `83fdf7ef0d6d4cc3f6d363ead24c0924abac46b6ef631ed5caa834e78b4c47da`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-patch-debug-focused/patch_debug_warm_300.heap.json`: 5346 bytes, sha256 `6fde0e24839a46107b73a483bd5facd1d549726c23fc6b840b81772686120473`

See `report.json` for typed timings and compact allocation summaries.
