# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `44618b23a9bd`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/ui/app/shell.rs`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 28908938878 ns
- `FsRunStore::load`: 906844893 ns
- `FsRunStore::load_history_blocks`: 23617511 ns
- `FsRunStore::load_transition_journal`: 4775053 ns
- `FsRunStore::load_record_set`: 935241565 ns
- `compressed_run_record_profile_probe`: 117176317 ns
- `Graph::from_records`: 5280722 ns
- `graph_load_total`: 1057700948 ns
- note: run_picker_discovery=28908938878 ns (kept in startup spans)
- note: warning: run_picker_discovery exceeded 250000000 ns
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `patch_debug_warm_300`: frames=300, median=1947756 ns, p95=2047403 ns, p99=2246828 ns, max=189286422 ns, heap_slope=plateau
  - component `capture_overhead`: median=3156 ns, median_frame_share=0.16%
  - component `central_graph`: median=1015425 ns, median_frame_share=52.13%
  - component `diagnostics`: median=69680 ns, median_frame_share=3.57%, nested_under=run_navigation
  - component `run_navigation`: median=277921 ns, median_frame_share=14.26%
  - component `selection_inspector`: median=454173 ns, median_frame_share=23.31%
  - component `timeline`: median=51667 ns, median_frame_share=2.65%
  - component `top_strip`: median=75541 ns, median_frame_share=3.87%
- `inspector_patch_debug_expanded_300`: frames=300, median=1945862 ns, p95=2093950 ns, p99=2503169 ns, max=2955107 ns, heap_slope=plateau
  - component `capture_overhead`: median=3126 ns, median_frame_share=0.16%
  - component `central_graph`: median=1015967 ns, median_frame_share=52.21%
  - component `diagnostics`: median=69761 ns, median_frame_share=3.58%, nested_under=run_navigation
  - component `run_navigation`: median=278854 ns, median_frame_share=14.33%
  - component `selection_inspector`: median=451648 ns, median_frame_share=23.21%
  - component `timeline`: median=51487 ns, median_frame_share=2.64%
  - component `top_strip`: median=75702 ns, median_frame_share=3.89%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260517-44618b23a9bd-standard/standard.puffin`: 1995018 bytes, sha256 `f850dbe7d30c3ae7cccc53b61fb1aeba82f26e525a157d40f589827c5a6061d5`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-44618b23a9bd-standard/patch_debug_warm_300.heap.json`: 4560 bytes, sha256 `d4de9118e47082e3010681b7010c756cecd74bd21a797f1f3f3eb88ab12d5fd1`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-44618b23a9bd-standard/inspector_patch_debug_expanded_300.heap.json`: 4500 bytes, sha256 `5e28c2a5ab0db647144ecd9e0768b130a13dbb7f244a38f0d5f75e6c85b7a504`

See `report.json` for typed timings and compact allocation summaries.
