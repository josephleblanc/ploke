# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `485d15440be0`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/ui/app/shell.rs`

unrelated dirty paths:
- `docs/active/agents/run-reviews/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 6873 ns
- `FsRunStore::load`: 934275657 ns
- `FsRunStore::load_history_blocks`: 24088779 ns
- `FsRunStore::load_transition_journal`: 5004799 ns
- `FsRunStore::load_record_set`: 963373782 ns
- `compressed_run_record_profile_probe`: 118539996 ns
- `Graph::from_records`: 6645466 ns
- `graph_load_total`: 1088562170 ns
- note: run_picker_discovery=6873 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=2240436 ns, p95=3068610 ns, p99=24895715 ns, max=63277234 ns, heap_slope=growing
  - component `capture_overhead`: median=7905 ns, median_frame_share=0.35%
  - component `central_graph`: median=1322364 ns, median_frame_share=59.02%
  - component `diagnostics`: median=70903 ns, median_frame_share=3.16%, nested_under=run_navigation
  - component `run_navigation`: median=285783 ns, median_frame_share=12.75%
  - component `selection_inspector`: median=405546 ns, median_frame_share=18.10%
  - component `timeline`: median=54692 ns, median_frame_share=2.44%
  - component `top_strip`: median=75982 ns, median_frame_share=3.39%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1172, median_object_bytes=862286, median_wrapped_bytes=873648, median_live_object_bytes=1679901
  - phase 2 `select_node`: frames=31..60, median_allocs=1248, median_object_bytes=943660, median_wrapped_bytes=955680, median_live_object_bytes=1883091
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1248, median_object_bytes=943655, median_wrapped_bytes=955680, median_live_object_bytes=1990184
  - phase 4 `expand_section`: frames=91..120, median_allocs=1483, median_object_bytes=1123517, median_wrapped_bytes=1137432, median_live_object_bytes=2333193
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1483, median_object_bytes=1123522, median_wrapped_bytes=1137440, median_live_object_bytes=2438465
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1253, median_object_bytes=944342, median_wrapped_bytes=956424, median_live_object_bytes=2544651
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1253, median_object_bytes=944314, median_wrapped_bytes=956392, median_live_object_bytes=2651013
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1179, median_object_bytes=853620, median_wrapped_bytes=865056, median_live_object_bytes=2758969
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1179, median_object_bytes=853611, median_wrapped_bytes=865048, median_live_object_bytes=2863505
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=2229216 ns, p95=3053843 ns, p99=3632031 ns, max=4712094 ns, heap_slope=plateau
  - component `capture_overhead`: median=7904 ns, median_frame_share=0.35%
  - component `central_graph`: median=1320631 ns, median_frame_share=59.24%
  - component `diagnostics`: median=70882 ns, median_frame_share=3.17%, nested_under=run_navigation
  - component `run_navigation`: median=285673 ns, median_frame_share=12.81%
  - component `selection_inspector`: median=402299 ns, median_frame_share=18.04%
  - component `timeline`: median=54481 ns, median_frame_share=2.44%
  - component `top_strip`: median=76032 ns, median_frame_share=3.41%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1167, median_object_bytes=852109, median_wrapped_bytes=863416, median_live_object_bytes=90486
  - phase 2 `select_node`: frames=31..60, median_allocs=1241, median_object_bytes=942385, median_wrapped_bytes=954344, median_live_object_bytes=237291
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1241, median_object_bytes=942388, median_wrapped_bytes=954352, median_live_object_bytes=342787
  - phase 4 `expand_section`: frames=91..120, median_allocs=1477, median_object_bytes=1122770, median_wrapped_bytes=1136616, median_live_object_bytes=614945
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1477, median_object_bytes=1122765, median_wrapped_bytes=1136608, median_live_object_bytes=719177
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1244, median_object_bytes=942789, median_wrapped_bytes=954784, median_live_object_bytes=824575
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1244, median_object_bytes=942790, median_wrapped_bytes=954784, median_live_object_bytes=932520
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1173, median_object_bytes=852867, median_wrapped_bytes=864240, median_live_object_bytes=1039260
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1173, median_object_bytes=852865, median_wrapped_bytes=864232, median_live_object_bytes=1143572
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=2230228 ns, p95=3228579 ns, p99=5333052 ns, max=6998017 ns, heap_slope=growing
  - component `capture_overhead`: median=7865 ns, median_frame_share=0.35%
  - component `central_graph`: median=1320508 ns, median_frame_share=59.20%
  - component `diagnostics`: median=71002 ns, median_frame_share=3.18%, nested_under=run_navigation
  - component `run_navigation`: median=285783 ns, median_frame_share=12.81%
  - component `selection_inspector`: median=405285 ns, median_frame_share=18.17%
  - component `timeline`: median=54412 ns, median_frame_share=2.43%
  - component `top_strip`: median=76032 ns, median_frame_share=3.40%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1167, median_object_bytes=852105, median_wrapped_bytes=863416, median_live_object_bytes=90428
  - phase 2 `select_node`: frames=31..60, median_allocs=1243, median_object_bytes=943044, median_wrapped_bytes=955016, median_live_object_bytes=211535
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1243, median_object_bytes=943046, median_wrapped_bytes=955016, median_live_object_bytes=317856
  - phase 4 `expand_section`: frames=91..120, median_allocs=2140, median_object_bytes=1170527, median_wrapped_bytes=1190520, median_live_object_bytes=708289
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2140, median_object_bytes=1170536, median_wrapped_bytes=1190528, median_live_object_bytes=812084
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1246, median_object_bytes=943423, median_wrapped_bytes=955424, median_live_object_bytes=909415
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1246, median_object_bytes=943406, median_wrapped_bytes=955408, median_live_object_bytes=1015402
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1172, median_object_bytes=852706, median_wrapped_bytes=864064, median_live_object_bytes=1123596
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1172, median_object_bytes=852701, median_wrapped_bytes=864064, median_live_object_bytes=1227551
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=2225260 ns, p95=3618114 ns, p99=4112078 ns, max=4641127 ns, heap_slope=plateau
  - component `capture_overhead`: median=7865 ns, median_frame_share=0.35%
  - component `central_graph`: median=1319777 ns, median_frame_share=59.30%
  - component `diagnostics`: median=70923 ns, median_frame_share=3.18%, nested_under=run_navigation
  - component `run_navigation`: median=285284 ns, median_frame_share=12.82%
  - component `selection_inspector`: median=402542 ns, median_frame_share=18.08%
  - component `timeline`: median=54291 ns, median_frame_share=2.43%
  - component `top_strip`: median=76002 ns, median_frame_share=3.41%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1167, median_object_bytes=852102, median_wrapped_bytes=863416, median_live_object_bytes=90280
  - phase 2 `select_node`: frames=31..60, median_allocs=1240, median_object_bytes=942262, median_wrapped_bytes=954216, median_live_object_bytes=209076
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1240, median_object_bytes=942263, median_wrapped_bytes=954216, median_live_object_bytes=314191
  - phase 4 `expand_section`: frames=91..120, median_allocs=2573, median_object_bytes=1228766, median_wrapped_bytes=1252696, median_live_object_bytes=509077
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2573, median_object_bytes=1228767, median_wrapped_bytes=1252696, median_live_object_bytes=612543
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1243, median_object_bytes=942647, median_wrapped_bytes=954624, median_live_object_bytes=706261
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1243, median_object_bytes=942625, median_wrapped_bytes=954608, median_live_object_bytes=827767
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1172, median_object_bytes=852707, median_wrapped_bytes=864064, median_live_object_bytes=937171
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1172, median_object_bytes=852709, median_wrapped_bytes=864072, median_live_object_bytes=1041217

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260519-485d15440be0-standard/standard.puffin`: 3323326 bytes, sha256 `d4c9911483d595270845d4284b11b81f61725c6da497e57bd5efac1ee7f45f78`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-485d15440be0-standard/inspector_run_records_phase_sequence_30.heap.json`: 14890 bytes, sha256 `c484d0076b3de4cb9db30e51655effaf84adf58d245722e6229c78afe22feb98`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-485d15440be0-standard/inspector_run_records_phase_sequence_alternate_30.heap.json`: 12376 bytes, sha256 `23bf6c760e24536eb439004364a07a52d7e44c0a6a4619590fc6ad5036089845`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-485d15440be0-standard/inspector_llm_calls_phase_sequence_30.heap.json`: 11970 bytes, sha256 `c6a4578049d17fd1e687a9542bb2dd1da0509a8ae98ddd2ac93cf2d099fc2e1d`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-485d15440be0-standard/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 11960 bytes, sha256 `82ef10497355286efc426790da424669842b72179fde36b1444629c57251ea1b`

See `report.json` for typed timings and compact allocation summaries.
