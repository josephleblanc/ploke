# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `4b8b45188526`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/inspector.rs`

unrelated dirty paths:
- `docs/active/archaeology/INDEX.md`
- `docs/active/archaeology/ploke-tree-graph/README.md`
- `docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260517-4b8b45188526-standard/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 28080008742 ns
- `FsRunStore::load`: 910690235 ns
- `FsRunStore::load_history_blocks`: 22852591 ns
- `FsRunStore::load_transition_journal`: 4442688 ns
- `FsRunStore::load_record_set`: 937988861 ns
- `compressed_run_record_profile_probe`: 119442996 ns
- `Graph::from_records`: 4919613 ns
- `graph_load_total`: 1062353724 ns
- note: run_picker_discovery=28080008742 ns (kept in startup spans)
- note: warning: run_picker_discovery exceeded 250000000 ns
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_expanded_300`: frames=300, median=2116979 ns, p95=2612307 ns, p99=2873627 ns, max=115258963 ns, heap_slope=plateau
  - component `capture_overhead`: median=3056 ns, median_frame_share=0.14%
  - component `central_graph`: median=1001258 ns, median_frame_share=47.29%
  - component `diagnostics`: median=69020 ns, median_frame_share=3.26%, nested_under=run_navigation
  - component `run_navigation`: median=278503 ns, median_frame_share=13.15%
  - component `selection_inspector`: median=633698 ns, median_frame_share=29.93%
  - component `timeline`: median=51406 ns, median_frame_share=2.42%
  - component `top_strip`: median=75542 ns, median_frame_share=3.56%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260517-4b8b45188526-standard/standard.puffin`: 930714 bytes, sha256 `c0f192733e684eb6895ba8804cec5b99c66fb7c2d8241a7bfc90db2f1af06f8e`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-4b8b45188526-standard/inspector_run_records_expanded_300.heap.json`: 4558 bytes, sha256 `0bffa572dc815e476e4816f836b8fba47fc01e6f130e1c9576bb91cdec76799c`

See `report.json` for typed timings and compact allocation summaries.
