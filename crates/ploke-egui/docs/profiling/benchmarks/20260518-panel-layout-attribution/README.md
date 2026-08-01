# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `673d9d61a304`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/view/edge.rs`
- `crates/ploke-egui/src/ui/view/mod.rs`
- `crates/ploke-egui/src/ui/view/node.rs`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 6632 ns
- `FsRunStore::load`: 963794139 ns
- `FsRunStore::load_history_blocks`: 23546667 ns
- `FsRunStore::load_transition_journal`: 4481777 ns
- `FsRunStore::load_record_set`: 991828284 ns
- `compressed_run_record_profile_probe`: 118276357 ns
- `Graph::from_records`: 8190724 ns
- `graph_load_total`: 1118300364 ns
- note: run_picker_discovery=6632 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=2033794 ns, p95=2982645 ns, p99=4412607 ns, max=67549016 ns, heap_slope=plateau
  - component `capture_overhead`: median=3086 ns, median_frame_share=0.15%
  - component `central_graph`: median=1298546 ns, median_frame_share=63.84%
  - component `diagnostics`: median=69461 ns, median_frame_share=3.41%, nested_under=run_navigation
  - component `run_navigation`: median=282250 ns, median_frame_share=13.87%
  - component `selection_inspector`: median=251222 ns, median_frame_share=12.35%
  - component `timeline`: median=51987 ns, median_frame_share=2.55%
  - component `top_strip`: median=78417 ns, median_frame_share=3.85%
- `warm_idle_300`: frames=300, median=2020651 ns, p95=2292941 ns, p99=2712678 ns, max=3364351 ns, heap_slope=plateau
  - component `capture_overhead`: median=3036 ns, median_frame_share=0.15%
  - component `central_graph`: median=1294007 ns, median_frame_share=64.03%
  - component `diagnostics`: median=68118 ns, median_frame_share=3.37%, nested_under=run_navigation
  - component `run_navigation`: median=277291 ns, median_frame_share=13.72%
  - component `selection_inspector`: median=246803 ns, median_frame_share=12.21%
  - component `timeline`: median=51467 ns, median_frame_share=2.54%
  - component `top_strip`: median=77556 ns, median_frame_share=3.83%
- `inspector_run_records_phase_sequence_30`: frames=270, median=2219012 ns, p95=3106126 ns, p99=10013463 ns, max=35072778 ns, heap_slope=growing
  - component `capture_overhead`: median=3156 ns, median_frame_share=0.14%
  - component `central_graph`: median=1303485 ns, median_frame_share=58.74%
  - component `diagnostics`: median=68419 ns, median_frame_share=3.08%, nested_under=run_navigation
  - component `run_navigation`: median=279594 ns, median_frame_share=12.59%
  - component `selection_inspector`: median=397847 ns, median_frame_share=17.92%
  - component `timeline`: median=53710 ns, median_frame_share=2.42%
  - component `top_strip`: median=78056 ns, median_frame_share=3.51%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1165, median_object_bytes=827446, median_wrapped_bytes=838736, median_live_object_bytes=73429
  - phase 2 `select_node`: frames=31..60, median_allocs=1239, median_object_bytes=908590, median_wrapped_bytes=920512, median_live_object_bytes=370653
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1239, median_object_bytes=908583, median_wrapped_bytes=920504, median_live_object_bytes=479694
  - phase 4 `expand_section`: frames=91..120, median_allocs=1473, median_object_bytes=1088317, median_wrapped_bytes=1102128, median_live_object_bytes=831254
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1473, median_object_bytes=1088316, median_wrapped_bytes=1102128, median_live_object_bytes=938502
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1243, median_object_bytes=909135, median_wrapped_bytes=921096, median_live_object_bytes=1047589
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1243, median_object_bytes=909122, median_wrapped_bytes=921080, median_live_object_bytes=1157398
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1169, median_object_bytes=818431, median_wrapped_bytes=829768, median_live_object_bytes=1268273
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1169, median_object_bytes=818422, median_wrapped_bytes=829760, median_live_object_bytes=1375556
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=2178406 ns, p95=3244155 ns, p99=3838060 ns, max=4598095 ns, heap_slope=plateau
  - component `capture_overhead`: median=3075 ns, median_frame_share=0.14%
  - component `central_graph`: median=1299277 ns, median_frame_share=59.64%
  - component `diagnostics`: median=68188 ns, median_frame_share=3.13%, nested_under=run_navigation
  - component `run_navigation`: median=277530 ns, median_frame_share=12.74%
  - component `selection_inspector`: median=393909 ns, median_frame_share=18.08%
  - component `timeline`: median=53370 ns, median_frame_share=2.44%
  - component `top_strip`: median=77385 ns, median_frame_share=3.55%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1164, median_object_bytes=817770, median_wrapped_bytes=829056, median_live_object_bytes=73468
  - phase 2 `select_node`: frames=31..60, median_allocs=1236, median_object_bytes=907812, median_wrapped_bytes=919720, median_live_object_bytes=222886
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1236, median_object_bytes=907808, median_wrapped_bytes=919712, median_live_object_bytes=330888
  - phase 4 `expand_section`: frames=91..120, median_allocs=1472, median_object_bytes=1088193, median_wrapped_bytes=1101992, median_live_object_bytes=606079
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1472, median_object_bytes=1088190, median_wrapped_bytes=1101992, median_live_object_bytes=729372
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1239, median_object_bytes=908225, median_wrapped_bytes=920160, median_live_object_bytes=837973
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1239, median_object_bytes=908214, median_wrapped_bytes=920152, median_live_object_bytes=945978
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1168, median_object_bytes=818296, median_wrapped_bytes=829616, median_live_object_bytes=1058549
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1168, median_object_bytes=818294, median_wrapped_bytes=829616, median_live_object_bytes=1165784
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=2173457 ns, p95=3104663 ns, p99=3406780 ns, max=7152707 ns, heap_slope=growing
  - component `capture_overhead`: median=3096 ns, median_frame_share=0.14%
  - component `central_graph`: median=1297223 ns, median_frame_share=59.68%
  - component `diagnostics`: median=68148 ns, median_frame_share=3.13%, nested_under=run_navigation
  - component `run_navigation`: median=277561 ns, median_frame_share=12.77%
  - component `selection_inspector`: median=395161 ns, median_frame_share=18.18%
  - component `timeline`: median=53019 ns, median_frame_share=2.43%
  - component `top_strip`: median=77485 ns, median_frame_share=3.56%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1164, median_object_bytes=817767, median_wrapped_bytes=829048, median_live_object_bytes=73556
  - phase 2 `select_node`: frames=31..60, median_allocs=1238, median_object_bytes=908466, median_wrapped_bytes=920376, median_live_object_bytes=201068
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1238, median_object_bytes=908464, median_wrapped_bytes=920368, median_live_object_bytes=310133
  - phase 4 `expand_section`: frames=91..120, median_allocs=2135, median_object_bytes=1135953, median_wrapped_bytes=1155904, median_live_object_bytes=703765
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2135, median_object_bytes=1135951, median_wrapped_bytes=1155896, median_live_object_bytes=810282
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1241, median_object_bytes=908846, median_wrapped_bytes=920784, median_live_object_bytes=910092
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1241, median_object_bytes=908831, median_wrapped_bytes=920768, median_live_object_bytes=1019301
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1167, median_object_bytes=818139, median_wrapped_bytes=829448, median_live_object_bytes=1130472
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1167, median_object_bytes=818132, median_wrapped_bytes=829448, median_live_object_bytes=1240212
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=2218091 ns, p95=3803094 ns, p99=3862846 ns, max=4410964 ns, heap_slope=plateau
  - component `capture_overhead`: median=3096 ns, median_frame_share=0.13%
  - component `central_graph`: median=1298806 ns, median_frame_share=58.55%
  - component `diagnostics`: median=68959 ns, median_frame_share=3.10%, nested_under=run_navigation
  - component `run_navigation`: median=281508 ns, median_frame_share=12.69%
  - component `selection_inspector`: median=398458 ns, median_frame_share=17.96%
  - component `timeline`: median=54132 ns, median_frame_share=2.44%
  - component `top_strip`: median=77956 ns, median_frame_share=3.51%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1164, median_object_bytes=817772, median_wrapped_bytes=829056, median_live_object_bytes=73679
  - phase 2 `select_node`: frames=31..60, median_allocs=1235, median_object_bytes=907699, median_wrapped_bytes=919592, median_live_object_bytes=195129
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1235, median_object_bytes=907697, median_wrapped_bytes=919592, median_live_object_bytes=303500
  - phase 4 `expand_section`: frames=91..120, median_allocs=2568, median_object_bytes=1194194, median_wrapped_bytes=1218080, median_live_object_bytes=501283
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2568, median_object_bytes=1194199, median_wrapped_bytes=1218080, median_live_object_bytes=607812
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1238, median_object_bytes=908076, median_wrapped_bytes=920000, median_live_object_bytes=704431
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1238, median_object_bytes=908067, median_wrapped_bytes=919992, median_live_object_bytes=812776
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1167, median_object_bytes=818139, median_wrapped_bytes=829448, median_live_object_bytes=925329
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1167, median_object_bytes=818137, median_wrapped_bytes=829448, median_live_object_bytes=1032764

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-panel-layout-attribution/standard.puffin`: 5310415 bytes, sha256 `e38baa4325fd9049ba65a67686fb6de6bdd7e66216725109eee692b39a1a67e3`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-panel-layout-attribution/startup_frames_300.heap.json`: 10402 bytes, sha256 `29f489142ad5c477fb254c605923cb7e1a540b520e8c8c914d8f55af08dea68b`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-panel-layout-attribution/warm_idle_300.heap.json`: 9093 bytes, sha256 `bb237b0ff9f02a8ff761945bed9ddb8fd419934a89dd7bb922cf25aa0f1f41f8`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-panel-layout-attribution/inspector_run_records_phase_sequence_30.heap.json`: 11205 bytes, sha256 `b728806e7db0867b3a317058aabbb8f3185afce6bb511e2989b96c8e6cab7b94`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-panel-layout-attribution/inspector_run_records_phase_sequence_alternate_30.heap.json`: 10755 bytes, sha256 `c12aa5f03caf45cb38050986324fef12b36fe097bb41225b3d0bc0a87db8a7d9`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-panel-layout-attribution/inspector_llm_calls_phase_sequence_30.heap.json`: 10347 bytes, sha256 `b0fa057222337498f85eb18c01f4a06376a7b44b9aeb185b6d7654ff9ae84cca`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-panel-layout-attribution/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 10337 bytes, sha256 `9b26c14d12c0d86500074c4375c78e6e99b4f5ac5629cf33a42ab39454b4e909`

See `report.json` for typed timings and compact allocation summaries.
