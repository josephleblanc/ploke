# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `c4a6acb3ab66`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/inspector.rs`
- `crates/ploke-egui/src/ui/view/projection.rs`
- `crates/ploke-tree/src/graph/build/selection.rs`
- `crates/ploke-tree/src/graph/build/selection/tests.rs`
- `crates/ploke-tree/src/graph/types/selection.rs`
- `crates/ploke-tree/src/graph/types/warning.rs`

unrelated dirty paths:
- `docs/active/agents/collaboration-incidents/authority-boundary-violations/README.md`
- `docs/active/archaeology/INDEX.md`
- `docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-selection-metric-witness-benchmark-note.md`
- `docs/active/agents/collaboration-incidents/authority-boundary-violations/2026-05-17-headless-harness-probe-used-primary-gitdir.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 28583398987 ns
- `FsRunStore::load`: 927948639 ns
- `FsRunStore::load_history_blocks`: 22735623 ns
- `FsRunStore::load_transition_journal`: 4603536 ns
- `FsRunStore::load_record_set`: 955293037 ns
- `compressed_run_record_profile_probe`: 116048568 ns
- `Graph::from_records`: 6044569 ns
- `graph_load_total`: 1077389091 ns
- note: run_picker_discovery=28583398987 ns (kept in startup spans)
- note: warning: run_picker_discovery exceeded 250000000 ns
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `select_artifact_inspector_300`: frames=300, median=2060035 ns, p95=2611098 ns, p99=5994996 ns, max=102111512 ns, heap_slope=plateau
  - component `capture_overhead`: median=3046 ns, median_frame_share=0.14%
  - component `central_graph`: median=1007049 ns, median_frame_share=48.88%
  - component `diagnostics`: median=69811 ns, median_frame_share=3.38%, nested_under=run_navigation
  - component `run_navigation`: median=278082 ns, median_frame_share=13.49%
  - component `selection_inspector`: median=571542 ns, median_frame_share=27.74%
  - component `timeline`: median=54052 ns, median_frame_share=2.62%
  - component `top_strip`: median=75762 ns, median_frame_share=3.67%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-c4a6acb3ab66-standard/standard.puffin`: 988041 bytes, sha256 `c5ac90260c2d524900c09be34c4e71c0c7d1b99be6b9f817dd9ba3b8461b8ae9`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-c4a6acb3ab66-standard/select_artifact_inspector_300.heap.json`: 4134 bytes, sha256 `7a4fb6d17755a034e7d95370a5f6d53cef15fb5950c16170a6212026a73f9ab7`

See `report.json` for typed timings and compact allocation summaries.
