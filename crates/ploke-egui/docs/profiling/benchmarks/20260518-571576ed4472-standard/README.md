# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `571576ed4472`

dirty_state: `clean`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 152346 ns
- `FsRunStore::load`: 940821112 ns
- `FsRunStore::load_history_blocks`: 23375886 ns
- `FsRunStore::load_transition_journal`: 4418174 ns
- `FsRunStore::load_record_set`: 968618999 ns
- `compressed_run_record_profile_probe`: 116728925 ns
- `Graph::from_records`: 8762028 ns
- `graph_load_total`: 1094112477 ns
- note: run_picker_discovery=152346 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=2247574 ns, p95=3083295 ns, p99=24754768 ns, max=68677592 ns, heap_slope=growing
  - component `capture_overhead`: median=8276 ns, median_frame_share=0.36%
  - component `central_graph`: median=1336241 ns, median_frame_share=59.45%
  - component `diagnostics`: median=69881 ns, median_frame_share=3.10%, nested_under=run_navigation
  - component `run_navigation`: median=282602 ns, median_frame_share=12.57%
  - component `selection_inspector`: median=407236 ns, median_frame_share=18.11%
  - component `timeline`: median=54412 ns, median_frame_share=2.42%
  - component `top_strip`: median=76123 ns, median_frame_share=3.38%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1171, median_object_bytes=828191, median_wrapped_bytes=839552, median_live_object_bytes=1626811
  - phase 2 `select_node`: frames=31..60, median_allocs=1247, median_object_bytes=909567, median_wrapped_bytes=921576, median_live_object_bytes=1832980
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1247, median_object_bytes=909562, median_wrapped_bytes=921568, median_live_object_bytes=1943088
  - phase 4 `expand_section`: frames=91..120, median_allocs=1482, median_object_bytes=1089416, median_wrapped_bytes=1103336, median_live_object_bytes=2289774
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1482, median_object_bytes=1089414, median_wrapped_bytes=1103328, median_live_object_bytes=2397674
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1252, median_object_bytes=910230, median_wrapped_bytes=922296, median_live_object_bytes=2506480
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1252, median_object_bytes=910218, median_wrapped_bytes=922280, median_live_object_bytes=2615844
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1178, median_object_bytes=819525, median_wrapped_bytes=830968, median_live_object_bytes=2727182
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1178, median_object_bytes=819512, median_wrapped_bytes=830952, median_live_object_bytes=2834336
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=2243477 ns, p95=3076923 ns, p99=3366347 ns, max=4667642 ns, heap_slope=plateau
  - component `capture_overhead`: median=8256 ns, median_frame_share=0.36%
  - component `central_graph`: median=1337704 ns, median_frame_share=59.62%
  - component `diagnostics`: median=69771 ns, median_frame_share=3.10%, nested_under=run_navigation
  - component `run_navigation`: median=281579 ns, median_frame_share=12.55%
  - component `selection_inspector`: median=402937 ns, median_frame_share=17.96%
  - component `timeline`: median=54222 ns, median_frame_share=2.41%
  - component `top_strip`: median=76063 ns, median_frame_share=3.39%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818012, median_wrapped_bytes=829320, median_live_object_bytes=73751
  - phase 2 `select_node`: frames=31..60, median_allocs=1240, median_object_bytes=908285, median_wrapped_bytes=920240, median_live_object_bytes=223122
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1240, median_object_bytes=908291, median_wrapped_bytes=920248, median_live_object_bytes=331395
  - phase 4 `expand_section`: frames=91..120, median_allocs=1476, median_object_bytes=1088666, median_wrapped_bytes=1102512, median_live_object_bytes=606929
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1476, median_object_bytes=1088669, median_wrapped_bytes=1102512, median_live_object_bytes=714100
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1243, median_object_bytes=908696, median_wrapped_bytes=920680, median_live_object_bytes=822873
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1243, median_object_bytes=908692, median_wrapped_bytes=920672, median_live_object_bytes=933976
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1172, median_object_bytes=818761, median_wrapped_bytes=830128, median_live_object_bytes=1043276
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1172, median_object_bytes=818771, median_wrapped_bytes=830144, median_live_object_bytes=1150809
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=2250350 ns, p95=3210835 ns, p99=3613411 ns, max=7284871 ns, heap_slope=growing
  - component `capture_overhead`: median=8276 ns, median_frame_share=0.36%
  - component `central_graph`: median=1339779 ns, median_frame_share=59.53%
  - component `diagnostics`: median=70102 ns, median_frame_share=3.11%, nested_under=run_navigation
  - component `run_navigation`: median=283162 ns, median_frame_share=12.58%
  - component `selection_inspector`: median=405632 ns, median_frame_share=18.02%
  - component `timeline`: median=54683 ns, median_frame_share=2.42%
  - component `top_strip`: median=76324 ns, median_frame_share=3.39%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818005, median_wrapped_bytes=829320, median_live_object_bytes=73754
  - phase 2 `select_node`: frames=31..60, median_allocs=1242, median_object_bytes=908946, median_wrapped_bytes=920904, median_live_object_bytes=197878
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1242, median_object_bytes=908951, median_wrapped_bytes=920904, median_live_object_bytes=307504
  - phase 4 `expand_section`: frames=91..120, median_allocs=2139, median_object_bytes=1136426, median_wrapped_bytes=1156424, median_live_object_bytes=700662
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2139, median_object_bytes=1136431, median_wrapped_bytes=1156424, median_live_object_bytes=807124
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1245, median_object_bytes=909318, median_wrapped_bytes=921304, median_live_object_bytes=907434
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1245, median_object_bytes=909300, median_wrapped_bytes=921288, median_live_object_bytes=1016861
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1171, median_object_bytes=818606, median_wrapped_bytes=829968, median_live_object_bytes=1127548
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1171, median_object_bytes=818607, median_wrapped_bytes=829968, median_live_object_bytes=1234814
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=2246232 ns, p95=3592853 ns, p99=3743997 ns, max=3922763 ns, heap_slope=plateau
  - component `capture_overhead`: median=8295 ns, median_frame_share=0.36%
  - component `central_graph`: median=1337864 ns, median_frame_share=59.56%
  - component `diagnostics`: median=70132 ns, median_frame_share=3.12%, nested_under=run_navigation
  - component `run_navigation`: median=282902 ns, median_frame_share=12.59%
  - component `selection_inspector`: median=405172 ns, median_frame_share=18.03%
  - component `timeline`: median=54312 ns, median_frame_share=2.41%
  - component `top_strip`: median=76414 ns, median_frame_share=3.40%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818010, median_wrapped_bytes=829320, median_live_object_bytes=73687
  - phase 2 `select_node`: frames=31..60, median_allocs=1239, median_object_bytes=908172, median_wrapped_bytes=920112, median_live_object_bytes=195484
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1239, median_object_bytes=908166, median_wrapped_bytes=920112, median_live_object_bytes=303646
  - phase 4 `expand_section`: frames=91..120, median_allocs=2572, median_object_bytes=1194677, median_wrapped_bytes=1218608, median_live_object_bytes=501475
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2572, median_object_bytes=1194673, median_wrapped_bytes=1218600, median_live_object_bytes=608368
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1242, median_object_bytes=908541, median_wrapped_bytes=920512, median_live_object_bytes=705134
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1242, median_object_bytes=908537, median_wrapped_bytes=920512, median_live_object_bytes=829643
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1171, median_object_bytes=818613, median_wrapped_bytes=829976, median_live_object_bytes=942158
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1171, median_object_bytes=818607, median_wrapped_bytes=829968, median_live_object_bytes=1049518

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-571576ed4472-standard/standard.puffin`: 3435700 bytes, sha256 `b832a509ad7c67ac72d4dcf34ea075d1dfc0d1acd40d4e1bbc13ebe1c3526c84`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-571576ed4472-standard/inspector_run_records_phase_sequence_30.heap.json`: 14890 bytes, sha256 `8c2b19c0f4d458fb2b9a5e190ce2dbd985353ffee1dc6e19160cd8a65d9488a9`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-571576ed4472-standard/inspector_run_records_phase_sequence_alternate_30.heap.json`: 12376 bytes, sha256 `dcdb30531f52a4cbdd06da56e511cd20e7af9bc0abc8e2d783c05e3dc4d0c62c`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-571576ed4472-standard/inspector_llm_calls_phase_sequence_30.heap.json`: 11970 bytes, sha256 `9d7f1f1677ea789a8b5075dd307a630988e56fcca290811b9cc684489066b534`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-571576ed4472-standard/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 11960 bytes, sha256 `44284b15e25eefbf53b3cd23ac1b6c8e45bc6a72aaff0b812d0135daec2fac75`

See `report.json` for typed timings and compact allocation summaries.
