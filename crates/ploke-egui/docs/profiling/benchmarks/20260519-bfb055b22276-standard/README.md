# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `bfb055b22276`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/ui/app/layout.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/dashboard/tiles.rs`
- `crates/ploke-egui/src/ui/view/diagnostics.rs`
- `crates/ploke-egui/src/ui/view/edge.rs`
- `crates/ploke-egui/src/ui/view/mod.rs`
- `crates/ploke-egui/src/ui/view/node.rs`
- `crates/ploke-egui/src/ui/view/projection.rs`
- `crates/ploke-tree/src/graph/artifact_tree.rs`
- `crates/ploke-egui/src/ui/view/effects.rs`

unrelated dirty paths:
- `docs/active/archaeology/INDEX.md`
- `docs/active/archaeology/ploke-tree-graph/README.md`
- `docs/active/archaeology/ploke-tree-graph/artifact-lineage-highlight.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 5570 ns
- `FsRunStore::load`: 946902238 ns
- `FsRunStore::load_history_blocks`: 23164706 ns
- `FsRunStore::load_transition_journal`: 4387482 ns
- `FsRunStore::load_record_set`: 974458995 ns
- `compressed_run_record_profile_probe`: 117519616 ns
- `Graph::from_records`: 6892087 ns
- `graph_load_total`: 1098873453 ns
- note: run_picker_discovery=5570 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `warm_idle_300`: frames=300, median=2645910 ns, p95=2794099 ns, p99=3324075 ns, max=69759196 ns, heap_slope=growing
  - component `capture_overhead`: median=8035 ns, median_frame_share=0.30%
  - component `central_graph`: median=2053237 ns, median_frame_share=77.60%
  - component `diagnostics`: median=74690 ns, median_frame_share=2.82%, nested_under=run_navigation
  - component `run_navigation`: median=370255 ns, median_frame_share=13.99%
  - component `timeline`: median=118392 ns, median_frame_share=4.47%
  - component `top_strip`: median=79018 ns, median_frame_share=2.98%
- `mode_artifact_tree_300`: frames=300, median=2645320 ns, p95=2869340 ns, p99=3200322 ns, max=4128696 ns, heap_slope=growing
  - component `capture_overhead`: median=8035 ns, median_frame_share=0.30%
  - component `central_graph`: median=2052276 ns, median_frame_share=77.58%
  - component `diagnostics`: median=74610 ns, median_frame_share=2.82%, nested_under=run_navigation
  - component `run_navigation`: median=370205 ns, median_frame_share=13.99%
  - component `timeline`: median=117932 ns, median_frame_share=4.45%
  - component `top_strip`: median=78958 ns, median_frame_share=2.98%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260519-bfb055b22276-standard/standard.puffin`: 2638577 bytes, sha256 `95b04a708723fc1672dc0f4483f88cc78e234a1afc283c3beea4419173711b5d`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-bfb055b22276-standard/warm_idle_300.heap.json`: 10770 bytes, sha256 `c8a03cade0e8a0c48f169598e61113c09261dbccd90dcba73b930c587c6fbe70`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-bfb055b22276-standard/mode_artifact_tree_300.heap.json`: 9089 bytes, sha256 `5f10376edfe9079444debb569ec647d3fd54c75877e367bb2736e3b475b6c838`

See `report.json` for typed timings and compact allocation summaries.
