# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `571576ed4472`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/benchmark/allocation_breakdown.rs`
- `crates/ploke-egui/src/cli/mod.rs`
- `crates/ploke-egui/src/native.rs`
- `crates/ploke-egui/src/ui/view/projection.rs`

unrelated dirty paths:
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`
- `docs/active/agents/2026-05-08_bounded-edit-surface-handoff.md`
- `docs/workflow/evalnomicon/drafts/edit-surface/proof-index.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-571576ed4472-standard/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-allocation-callsite-post-standard/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-allocation-callsite-sampled/`

## Callsite Sampling

- sample every: `128` matching allocations
- accounting: `scaled sampled estimates; callsite totals are not exact allocator totals`
- scopes:
  - `eframe_run_native`
  - `selection_inspector`
  - `central_graph_widget_add`
  - `inspector_run_record_tool_step`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 5501 ns
- `FsRunStore::load`: 938618093 ns
- `FsRunStore::load_history_blocks`: 23872487 ns
- `FsRunStore::load_transition_journal`: 4563583 ns
- `FsRunStore::load_record_set`: 967058641 ns
- `compressed_run_record_profile_probe`: 117168269 ns
- `Graph::from_records`: 6759222 ns
- `graph_load_total`: 1090988857 ns
- note: run_picker_discovery=5501 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=3221068 ns, p95=4434930 ns, p99=26434085 ns, max=1597313250 ns, heap_slope=growing
  - component `capture_overhead`: median=8466 ns, median_frame_share=0.26%
  - component `central_graph`: median=2199828 ns, median_frame_share=68.29%
  - component `diagnostics`: median=81003 ns, median_frame_share=2.51%, nested_under=run_navigation
  - component `run_navigation`: median=339548 ns, median_frame_share=10.54%
  - component `selection_inspector`: median=429187 ns, median_frame_share=13.32%
  - component `timeline`: median=58190 ns, median_frame_share=1.80%
  - component `top_strip`: median=115928 ns, median_frame_share=3.59%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1244, median_object_bytes=839482, median_wrapped_bytes=851696, median_live_object_bytes=1638255
  - phase 2 `select_node`: frames=31..60, median_allocs=1353, median_object_bytes=926278, median_wrapped_bytes=939512, median_live_object_bytes=1849845
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1407, median_object_bytes=935754, median_wrapped_bytes=949616, median_live_object_bytes=1969797
  - phase 4 `expand_section`: frames=91..120, median_allocs=1662, median_object_bytes=1118852, median_wrapped_bytes=1134880, median_live_object_bytes=2319080
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1667, median_object_bytes=1119717, median_wrapped_bytes=1135792, median_live_object_bytes=2428829
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1449, median_object_bytes=942314, median_wrapped_bytes=956664, median_live_object_bytes=2539150
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1464, median_object_bytes=945070, median_wrapped_bytes=959600, median_live_object_bytes=2652017
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1393, median_object_bytes=854722, median_wrapped_bytes=868648, median_live_object_bytes=2763618
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1404, median_object_bytes=856247, median_wrapped_bytes=870288, median_live_object_bytes=2872730
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (140502016 wrapped bytes)
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=3218643 ns, p95=4342858 ns, p99=4521033 ns, max=5613788 ns, heap_slope=growing
  - component `capture_overhead`: median=8396 ns, median_frame_share=0.26%
  - component `central_graph`: median=2178938 ns, median_frame_share=67.69%
  - component `diagnostics`: median=79961 ns, median_frame_share=2.48%, nested_under=run_navigation
  - component `run_navigation`: median=335541 ns, median_frame_share=10.42%
  - component `selection_inspector`: median=496884 ns, median_frame_share=15.43%
  - component `timeline`: median=57789 ns, median_frame_share=1.79%
  - component `top_strip`: median=115256 ns, median_frame_share=3.58%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1225, median_object_bytes=827383, median_wrapped_bytes=839384, median_live_object_bytes=83170
  - phase 2 `select_node`: frames=31..60, median_allocs=1363, median_object_bytes=927889, median_wrapped_bytes=941256, median_live_object_bytes=243314
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1393, median_object_bytes=932578, median_wrapped_bytes=946304, median_live_object_bytes=356530
  - phase 4 `expand_section`: frames=91..120, median_allocs=1649, median_object_bytes=1116470, median_wrapped_bytes=1132296, median_live_object_bytes=635618
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1659, median_object_bytes=1118012, median_wrapped_bytes=1133952, median_live_object_bytes=744216
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1440, median_object_bytes=940121, median_wrapped_bytes=954352, median_live_object_bytes=855086
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1444, median_object_bytes=941065, median_wrapped_bytes=955344, median_live_object_bytes=967130
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1378, median_object_bytes=852063, median_wrapped_bytes=865792, median_live_object_bytes=1077589
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1387, median_object_bytes=853378, median_wrapped_bytes=867192, median_live_object_bytes=1186687
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (91246592 wrapped bytes)
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=3226618 ns, p95=5969316 ns, p99=6253040 ns, max=20527897 ns, heap_slope=growing
  - component `capture_overhead`: median=8295 ns, median_frame_share=0.25%
  - component `central_graph`: median=2160113 ns, median_frame_share=66.94%
  - component `diagnostics`: median=79670 ns, median_frame_share=2.46%, nested_under=run_navigation
  - component `run_navigation`: median=311085 ns, median_frame_share=9.64%
  - component `selection_inspector`: median=429378 ns, median_frame_share=13.30%
  - component `timeline`: median=58891 ns, median_frame_share=1.82%
  - component `top_strip`: median=87314 ns, median_frame_share=2.70%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1229, median_object_bytes=827534, median_wrapped_bytes=839560, median_live_object_bytes=83628
  - phase 2 `select_node`: frames=31..60, median_allocs=1355, median_object_bytes=927817, median_wrapped_bytes=941056, median_live_object_bytes=215975
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1395, median_object_bytes=934029, median_wrapped_bytes=947704, median_live_object_bytes=332528
  - phase 4 `expand_section`: frames=91..120, median_allocs=2352, median_object_bytes=1170926, median_wrapped_bytes=1193336, median_live_object_bytes=736088
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2368, median_object_bytes=1173658, median_wrapped_bytes=1196248, median_live_object_bytes=845257
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1480, median_object_bytes=947547, median_wrapped_bytes=962184, median_live_object_bytes=946310
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1486, median_object_bytes=948489, median_wrapped_bytes=963192, median_live_object_bytes=1056776
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1412, median_object_bytes=857980, median_wrapped_bytes=872064, median_live_object_bytes=1166592
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1420, median_object_bytes=858939, median_wrapped_bytes=873120, median_live_object_bytes=1276385
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (99783680 wrapped bytes)
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=3220176 ns, p95=7084794 ns, p99=7573984 ns, max=7945082 ns, heap_slope=plateau
  - component `capture_overhead`: median=8276 ns, median_frame_share=0.25%
  - component `central_graph`: median=2160874 ns, median_frame_share=67.10%
  - component `diagnostics`: median=78407 ns, median_frame_share=2.43%, nested_under=run_navigation
  - component `run_navigation`: median=310064 ns, median_frame_share=9.62%
  - component `selection_inspector`: median=421583 ns, median_frame_share=13.09%
  - component `timeline`: median=58329 ns, median_frame_share=1.81%
  - component `top_strip`: median=86232 ns, median_frame_share=2.67%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1225, median_object_bytes=827319, median_wrapped_bytes=839312, median_live_object_bytes=83283
  - phase 2 `select_node`: frames=31..60, median_allocs=1338, median_object_bytes=924124, median_wrapped_bytes=937224, median_live_object_bytes=211323
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1380, median_object_bytes=931163, median_wrapped_bytes=944736, median_live_object_bytes=327430
  - phase 4 `expand_section`: frames=91..120, median_allocs=2753, median_object_bytes=1224229, median_wrapped_bytes=1250256, median_live_object_bytes=531874
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2777, median_object_bytes=1227901, median_wrapped_bytes=1254184, median_live_object_bytes=642308
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1463, median_object_bytes=944290, median_wrapped_bytes=958792, median_live_object_bytes=741406
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1471, median_object_bytes=945992, median_wrapped_bytes=960584, median_live_object_bytes=867885
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1404, median_object_bytes=856645, median_wrapped_bytes=870672, median_live_object_bytes=980671
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1405, median_object_bytes=856795, median_wrapped_bytes=870840, median_live_object_bytes=1088246
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (91670528 wrapped bytes)

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-allocation-callsite-sampled/standard.puffin`: 3439690 bytes, sha256 `211cb3eae97401c50a50a7adff00434a6121eda41390402df3e6c731bd684ec9`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-allocation-callsite-sampled/inspector_run_records_phase_sequence_30.heap.json`: 88026 bytes, sha256 `6d1b7b5a955a07c461b410ba4e09cd415d7d9833c1fc814ce4c57a387e859693`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-allocation-callsite-sampled/inspector_run_records_phase_sequence_alternate_30.heap.json`: 80579 bytes, sha256 `0794f20ce2781971ad8500410b884559fe8daae9123ed7c3b9c05d805c929011`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-allocation-callsite-sampled/inspector_llm_calls_phase_sequence_30.heap.json`: 91894 bytes, sha256 `b53c704266d02728cf4f305d8434c66f4cc7d3e6f996c212142472501412138e`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-allocation-callsite-sampled/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 87711 bytes, sha256 `9b8c9abf69aed98f1f9da2a61cf58c29a522b87d3f91161491d0bb9b16680087`

See `report.json` for typed timings and compact allocation summaries.
