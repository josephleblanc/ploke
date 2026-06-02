# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `5078f9b2e652`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/native.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`

unrelated dirty paths:
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `crates/ploke-tui/src/llm/manager/session.rs`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-eframe-root-attribution/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 6672 ns
- `FsRunStore::load`: 959797755 ns
- `FsRunStore::load_history_blocks`: 23706007 ns
- `FsRunStore::load_transition_journal`: 4519568 ns
- `FsRunStore::load_record_set`: 988026918 ns
- `compressed_run_record_profile_probe`: 113643851 ns
- `Graph::from_records`: 6776212 ns
- `graph_load_total`: 1108449635 ns
- note: run_picker_discovery=6672 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=2283234 ns, p95=3676373 ns, p99=26373694 ns, max=62708420 ns, heap_slope=growing
  - component `capture_overhead`: median=8085 ns, median_frame_share=0.35%
  - component `central_graph`: median=1318590 ns, median_frame_share=57.75%
  - component `diagnostics`: median=70703 ns, median_frame_share=3.09%, nested_under=run_navigation
  - component `run_navigation`: median=287451 ns, median_frame_share=12.58%
  - component `selection_inspector`: median=402707 ns, median_frame_share=17.63%
  - component `timeline`: median=54412 ns, median_frame_share=2.38%
  - component `top_strip`: median=82425 ns, median_frame_share=3.61%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1172, median_object_bytes=862289, median_wrapped_bytes=873648, median_live_object_bytes=1679757
  - phase 2 `select_node`: frames=31..60, median_allocs=1245, median_object_bytes=952316, median_wrapped_bytes=964320, median_live_object_bytes=1879068
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1245, median_object_bytes=952314, median_wrapped_bytes=964320, median_live_object_bytes=1985528
  - phase 4 `expand_section`: frames=91..120, median_allocs=2577, median_object_bytes=1229273, median_wrapped_bytes=1253240, median_live_object_bytes=2380232
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2577, median_object_bytes=1229270, median_wrapped_bytes=1253240, median_live_object_bytes=2484841
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1247, median_object_bytes=943141, median_wrapped_bytes=955160, median_live_object_bytes=2578809
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1247, median_object_bytes=943133, median_wrapped_bytes=955152, median_live_object_bytes=2684378
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1176, median_object_bytes=853211, median_wrapped_bytes=864608, median_live_object_bytes=2793606
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1176, median_object_bytes=853208, median_wrapped_bytes=864608, median_live_object_bytes=2898036

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-eframe-root-attribution-llm-alt-rerun/standard.puffin`: 844485 bytes, sha256 `bb0cf07ff25032f3d61fe856c7a9384e12975f795d0dd6f21df992aca39ee45e`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-eframe-root-attribution-llm-alt-rerun/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 13668 bytes, sha256 `64d3e1fa90ef8957e138e5d4c0f7589e2869c66be28ab2aca4d45fe4f527098a`

See `report.json` for typed timings and compact allocation summaries.
