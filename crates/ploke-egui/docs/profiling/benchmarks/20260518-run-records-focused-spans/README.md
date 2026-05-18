# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `f64cd82110f4`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 8516 ns
- `FsRunStore::load`: 1068304663 ns
- `FsRunStore::load_history_blocks`: 24194806 ns
- `FsRunStore::load_transition_journal`: 4703246 ns
- `FsRunStore::load_record_set`: 1097206833 ns
- `compressed_run_record_profile_probe`: 116729649 ns
- `Graph::from_records`: 6566504 ns
- `graph_load_total`: 1220505862 ns
- note: run_picker_discovery=8516 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=1871654 ns, p95=2665429 ns, p99=25772707 ns, max=60768390 ns, heap_slope=growing
  - component `capture_overhead`: median=3086 ns, median_frame_share=0.16%
  - component `central_graph`: median=1012076 ns, median_frame_share=54.07%
  - component `diagnostics`: median=70452 ns, median_frame_share=3.76%, nested_under=run_navigation
  - component `run_navigation`: median=283484 ns, median_frame_share=15.14%
  - component `selection_inspector`: median=375166 ns, median_frame_share=20.04%
  - component `timeline`: median=51827 ns, median_frame_share=2.76%
  - component `top_strip`: median=75853 ns, median_frame_share=4.05%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1183, median_object_bytes=841757, median_wrapped_bytes=853288, median_live_object_bytes=1649763
  - phase 2 `select_node`: frames=31..60, median_allocs=1216, median_object_bytes=905992, median_wrapped_bytes=917648, median_live_object_bytes=1884482
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1216, median_object_bytes=906001, median_wrapped_bytes=917656, median_live_object_bytes=2001807
  - phase 4 `expand_section`: frames=91..120, median_allocs=1446, median_object_bytes=1085194, median_wrapped_bytes=1098704, median_live_object_bytes=2347676
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1446, median_object_bytes=1085188, median_wrapped_bytes=1098696, median_live_object_bytes=2468696
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1216, median_object_bytes=906010, median_wrapped_bytes=917664, median_live_object_bytes=2577539
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1216, median_object_bytes=905990, median_wrapped_bytes=917648, median_live_object_bytes=2686833
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815292, median_wrapped_bytes=826320, median_live_object_bytes=2797942
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815299, median_wrapped_bytes=826328, median_live_object_bytes=2905491
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=1869981 ns, p95=2567234 ns, p99=2743556 ns, max=4470758 ns, heap_slope=growing
  - component `capture_overhead`: median=3036 ns, median_frame_share=0.16%
  - component `central_graph`: median=1010413 ns, median_frame_share=54.03%
  - component `diagnostics`: median=70193 ns, median_frame_share=3.75%, nested_under=run_navigation
  - component `run_navigation`: median=282672 ns, median_frame_share=15.11%
  - component `selection_inspector`: median=373734 ns, median_frame_share=19.98%
  - component `timeline`: median=51577 ns, median_frame_share=2.75%
  - component `top_strip`: median=75002 ns, median_frame_share=4.01%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815299, median_wrapped_bytes=826336, median_live_object_bytes=72731
  - phase 2 `select_node`: frames=31..60, median_allocs=1213, median_object_bytes=905230, median_wrapped_bytes=916872, median_live_object_bytes=226826
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1213, median_object_bytes=905217, median_wrapped_bytes=916864, median_live_object_bytes=342260
  - phase 4 `expand_section`: frames=91..120, median_allocs=1446, median_object_bytes=1085188, median_wrapped_bytes=1098696, median_live_object_bytes=617003
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1446, median_object_bytes=1085180, median_wrapped_bytes=1098688, median_live_object_bytes=737431
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1213, median_object_bytes=905226, median_wrapped_bytes=916872, median_live_object_bytes=846093
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1213, median_object_bytes=905212, median_wrapped_bytes=916856, median_live_object_bytes=956896
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815288, median_wrapped_bytes=826320, median_live_object_bytes=1066459
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815293, median_wrapped_bytes=826328, median_live_object_bytes=1173519

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-run-records-focused-spans/standard.puffin`: 1755762 bytes, sha256 `9a5c2e6d29c9d287018f3eae7fd7fbb9281beb1aa3f3752ed6085afb239b7087`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-records-focused-spans/inspector_run_records_phase_sequence_30.heap.json`: 5793 bytes, sha256 `7cc84cb6358e82f6bf833025b13ec1f3fb7f9b0d10dd7333036137d2c7fcbb0c`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-records-focused-spans/inspector_run_records_phase_sequence_alternate_30.heap.json`: 4931 bytes, sha256 `dc353a65fa3b7aa07e969e8883ba329e73bc420bb0b7cb744ad16a6fe4424c89`

See `report.json` for typed timings and compact allocation summaries.
