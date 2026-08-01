# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `0be892aba8fc`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/dashboard/tiles.rs`
- `crates/ploke-egui/src/ui/inspector.rs`
- `crates/ploke-tree/src/graph/build/selection.rs`
- `crates/ploke-tree/src/graph/build/selection/tests.rs`
- `crates/ploke-tree/src/graph/types/selection.rs`

unrelated dirty paths:
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/tools/tool_tests/patches.rs`
- `docs/active/agents/expected-failing-regression-tests.md`
- `docs/active/archaeology/INDEX.md`
- `docs/active/archaeology/ploke-tree-graph/README.md`
- `docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md`
- `docs/active/bugs/2026-05-19-rf-05-edit-composition-same-file-repair.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260519-0be892aba8fc-standard/`
- `docs/active/archaeology/ploke-tree-graph/lineage-authority.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 9608 ns
- `FsRunStore::load`: 920568975 ns
- `FsRunStore::load_history_blocks`: 23748888 ns
- `FsRunStore::load_transition_journal`: 4586369 ns
- `FsRunStore::load_record_set`: 948907497 ns
- `compressed_run_record_profile_probe`: 114457235 ns
- `Graph::from_records`: 6660085 ns
- `graph_load_total`: 1070027141 ns
- note: run_picker_discovery=9608 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=3782388 ns, p95=3953439 ns, p99=12511451 ns, max=71168357 ns, heap_slope=growing
  - component `capture_overhead`: median=8216 ns, median_frame_share=0.21%
  - component `central_graph`: median=3185175 ns, median_frame_share=84.21%
  - component `diagnostics`: median=74811 ns, median_frame_share=1.97%, nested_under=run_navigation
  - component `run_navigation`: median=371327 ns, median_frame_share=9.81%
  - component `timeline`: median=120005 ns, median_frame_share=3.17%
  - component `top_strip`: median=79099 ns, median_frame_share=2.09%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1965, median_object_bytes=1071251, median_wrapped_bytes=1089312, median_live_object_bytes=806748
  - phase 2 `select_node`: frames=31..60, median_allocs=2316, median_object_bytes=1185492, median_wrapped_bytes=1206640, median_live_object_bytes=1002933
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=2316, median_object_bytes=1185433, median_wrapped_bytes=1206584, median_live_object_bytes=1152883
  - phase 4 `expand_section`: frames=91..120, median_allocs=2315, median_object_bytes=1184205, median_wrapped_bytes=1205344, median_live_object_bytes=1303246
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2316, median_object_bytes=1185500, median_wrapped_bytes=1206648, median_live_object_bytes=1453575
  - phase 6 `collapse_section`: frames=151..180, median_allocs=2316, median_object_bytes=1185503, median_wrapped_bytes=1206656, median_live_object_bytes=1603154
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=2316, median_object_bytes=1185484, median_wrapped_bytes=1206632, median_live_object_bytes=1751967
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1967, median_object_bytes=1071431, median_wrapped_bytes=1089520, median_live_object_bytes=1888789
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1966, median_object_bytes=1070144, median_wrapped_bytes=1088224, median_live_object_bytes=2037658
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=3761599 ns, p95=3891332 ns, p99=4679915 ns, max=9984332 ns, heap_slope=growing
  - component `capture_overhead`: median=8196 ns, median_frame_share=0.21%
  - component `central_graph`: median=3166260 ns, median_frame_share=84.17%
  - component `diagnostics`: median=74790 ns, median_frame_share=1.98%, nested_under=run_navigation
  - component `run_navigation`: median=371418 ns, median_frame_share=9.87%
  - component `timeline`: median=119835 ns, median_frame_share=3.18%
  - component `top_strip`: median=79079 ns, median_frame_share=2.10%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1961, median_object_bytes=1070682, median_wrapped_bytes=1088704, median_live_object_bytes=121790
  - phase 2 `select_node`: frames=31..60, median_allocs=2288, median_object_bytes=1183431, median_wrapped_bytes=1204288, median_live_object_bytes=314065
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=2288, median_object_bytes=1183428, median_wrapped_bytes=1204280, median_live_object_bytes=463115
  - phase 4 `expand_section`: frames=91..120, median_allocs=2288, median_object_bytes=1183441, median_wrapped_bytes=1204296, median_live_object_bytes=612990
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2287, median_object_bytes=1182159, median_wrapped_bytes=1203008, median_live_object_bytes=762067
  - phase 6 `collapse_section`: frames=151..180, median_allocs=2287, median_object_bytes=1182169, median_wrapped_bytes=1203016, median_live_object_bytes=911776
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=2288, median_object_bytes=1183432, median_wrapped_bytes=1204288, median_live_object_bytes=1063659
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1963, median_object_bytes=1070944, median_wrapped_bytes=1088992, median_live_object_bytes=1197721
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1963, median_object_bytes=1070942, median_wrapped_bytes=1088984, median_live_object_bytes=1346921
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=3784642 ns, p95=5002210 ns, p99=5811852 ns, max=32573962 ns, heap_slope=growing
  - component `capture_overhead`: median=8205 ns, median_frame_share=0.21%
  - component `central_graph`: median=3187510 ns, median_frame_share=84.22%
  - component `diagnostics`: median=74791 ns, median_frame_share=1.97%, nested_under=run_navigation
  - component `run_navigation`: median=371949 ns, median_frame_share=9.82%
  - component `timeline`: median=119975 ns, median_frame_share=3.17%
  - component `top_strip`: median=79128 ns, median_frame_share=2.09%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1960, median_object_bytes=1069470, median_wrapped_bytes=1087488, median_live_object_bytes=122212
  - phase 2 `select_node`: frames=31..60, median_allocs=2310, median_object_bytes=1183587, median_wrapped_bytes=1204680, median_live_object_bytes=303509
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=2311, median_object_bytes=1184876, median_wrapped_bytes=1205976, median_live_object_bytes=451969
  - phase 4 `expand_section`: frames=91..120, median_allocs=3296, median_object_bytes=1494368, median_wrapped_bytes=1524280, median_live_object_bytes=1143960
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=3296, median_object_bytes=1494358, median_wrapped_bytes=1524272, median_live_object_bytes=1299538
  - phase 6 `collapse_section`: frames=151..180, median_allocs=2314, median_object_bytes=1184108, median_wrapped_bytes=1205240, median_live_object_bytes=1438376
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=2314, median_object_bytes=1184093, median_wrapped_bytes=1205224, median_live_object_bytes=1587563
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1966, median_object_bytes=1071312, median_wrapped_bytes=1089392, median_live_object_bytes=1724251
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1966, median_object_bytes=1071303, median_wrapped_bytes=1089384, median_live_object_bytes=1872973
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=3764945 ns, p95=5440283 ns, p99=7508831 ns, max=9614497 ns, heap_slope=growing
  - component `capture_overhead`: median=8206 ns, median_frame_share=0.21%
  - component `central_graph`: median=3168384 ns, median_frame_share=84.15%
  - component `diagnostics`: median=74730 ns, median_frame_share=1.98%, nested_under=run_navigation
  - component `run_navigation`: median=371829 ns, median_frame_share=9.87%
  - component `timeline`: median=119895 ns, median_frame_share=3.18%
  - component `top_strip`: median=79088 ns, median_frame_share=2.10%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1961, median_object_bytes=1070696, median_wrapped_bytes=1088720, median_live_object_bytes=121919
  - phase 2 `select_node`: frames=31..60, median_allocs=2287, median_object_bytes=1182840, median_wrapped_bytes=1203472, median_live_object_bytes=302933
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=2286, median_object_bytes=1182028, median_wrapped_bytes=1202864, median_live_object_bytes=451894
  - phase 4 `expand_section`: frames=91..120, median_allocs=3707, median_object_bytes=1497439, median_wrapped_bytes=1531024, median_live_object_bytes=889534
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=3707, median_object_bytes=1497435, median_wrapped_bytes=1531016, median_live_object_bytes=1044872
  - phase 6 `collapse_section`: frames=151..180, median_allocs=2290, median_object_bytes=1183711, median_wrapped_bytes=1204584, median_live_object_bytes=1180531
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=2289, median_object_bytes=1182397, median_wrapped_bytes=1203264, median_live_object_bytes=1345639
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1964, median_object_bytes=1069915, median_wrapped_bytes=1087968, median_live_object_bytes=1482292
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1965, median_object_bytes=1071181, median_wrapped_bytes=1089248, median_live_object_bytes=1631203

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260519-0be892aba8fc-standard/standard.puffin`: 4952748 bytes, sha256 `07addff3b0f2fd0bfb36ca23c66a46e3e2b0d02b049deb11f43e823c0e912928`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-0be892aba8fc-standard/inspector_run_records_phase_sequence_30.heap.json`: 11611 bytes, sha256 `f20ba81598cd13d828b16c226520ac5565b5b41a73b9f72a1105d8d278a3f6bc`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-0be892aba8fc-standard/inspector_run_records_phase_sequence_alternate_30.heap.json`: 9931 bytes, sha256 `b65c5311c155cd48547bc4e2abd90d85cecef3e9e4a0a79f919e6b76dcae0354`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-0be892aba8fc-standard/inspector_llm_calls_phase_sequence_30.heap.json`: 11181 bytes, sha256 `d8092ce1a7dea7174db09071ae46f727fa6b848f66f06979156de31ec184655e`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260519-0be892aba8fc-standard/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 10766 bytes, sha256 `00242255e7f2af7bd7a6a028723799866f8aceed5c5fb338a5051d5e6f5e8aba`

See `report.json` for typed timings and compact allocation summaries.
