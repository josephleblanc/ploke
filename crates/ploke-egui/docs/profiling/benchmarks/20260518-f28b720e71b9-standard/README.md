# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `f28b720e71b9`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/ui/app/shell.rs`

unrelated dirty paths:
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-f28b720e71b9-standard/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 6372 ns
- `FsRunStore::load`: 914347229 ns
- `FsRunStore::load_history_blocks`: 24062216 ns
- `FsRunStore::load_transition_journal`: 4707932 ns
- `FsRunStore::load_record_set`: 943121244 ns
- `compressed_run_record_profile_probe`: 118276216 ns
- `Graph::from_records`: 6918690 ns
- `graph_load_total`: 1068318696 ns
- note: run_picker_discovery=6372 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=2214496 ns, p95=3238308 ns, p99=25014664 ns, max=62744083 ns, heap_slope=growing
  - component `capture_overhead`: median=8075 ns, median_frame_share=0.36%
  - component `central_graph`: median=1309806 ns, median_frame_share=59.14%
  - component `diagnostics`: median=70462 ns, median_frame_share=3.18%, nested_under=run_navigation
  - component `run_navigation`: median=283419 ns, median_frame_share=12.79%
  - component `selection_inspector`: median=401841 ns, median_frame_share=18.14%
  - component `timeline`: median=54462 ns, median_frame_share=2.45%
  - component `top_strip`: median=75611 ns, median_frame_share=3.41%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1171, median_object_bytes=828187, median_wrapped_bytes=839544, median_live_object_bytes=1633407
  - phase 2 `select_node`: frames=31..60, median_allocs=1247, median_object_bytes=909560, median_wrapped_bytes=921568, median_live_object_bytes=1839524
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1247, median_object_bytes=909571, median_wrapped_bytes=921576, median_live_object_bytes=1949923
  - phase 4 `expand_section`: frames=91..120, median_allocs=1482, median_object_bytes=1089424, median_wrapped_bytes=1103344, median_live_object_bytes=2296602
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1482, median_object_bytes=1089423, median_wrapped_bytes=1103336, median_live_object_bytes=2404620
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1253, median_object_bytes=910255, median_wrapped_bytes=922328, median_live_object_bytes=2513821
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1252, median_object_bytes=910219, median_wrapped_bytes=922280, median_live_object_bytes=2623361
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1178, median_object_bytes=819526, median_wrapped_bytes=830968, median_live_object_bytes=2734504
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1178, median_object_bytes=819516, median_wrapped_bytes=830960, median_live_object_bytes=2841808
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=2215848 ns, p95=3018518 ns, p99=3326753 ns, max=4691091 ns, heap_slope=plateau
  - component `capture_overhead`: median=8045 ns, median_frame_share=0.36%
  - component `central_graph`: median=1309986 ns, median_frame_share=59.11%
  - component `diagnostics`: median=70271 ns, median_frame_share=3.17%, nested_under=run_navigation
  - component `run_navigation`: median=281686 ns, median_frame_share=12.71%
  - component `selection_inspector`: median=402151 ns, median_frame_share=18.14%
  - component `timeline`: median=54101 ns, median_frame_share=2.44%
  - component `top_strip`: median=75271 ns, median_frame_share=3.39%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818015, median_wrapped_bytes=829328, median_live_object_bytes=73931
  - phase 2 `select_node`: frames=31..60, median_allocs=1240, median_object_bytes=908293, median_wrapped_bytes=920248, median_live_object_bytes=223683
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1240, median_object_bytes=908286, median_wrapped_bytes=920240, median_live_object_bytes=332413
  - phase 4 `expand_section`: frames=91..120, median_allocs=1476, median_object_bytes=1088677, median_wrapped_bytes=1102520, median_live_object_bytes=607517
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1476, median_object_bytes=1088660, median_wrapped_bytes=1102504, median_live_object_bytes=714872
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1243, median_object_bytes=908694, median_wrapped_bytes=920680, median_live_object_bytes=823328
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1243, median_object_bytes=908693, median_wrapped_bytes=920672, median_live_object_bytes=934343
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1172, median_object_bytes=818766, median_wrapped_bytes=830136, median_live_object_bytes=1044471
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1172, median_object_bytes=818768, median_wrapped_bytes=830136, median_live_object_bytes=1151526
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=2226719 ns, p95=3532007 ns, p99=5214088 ns, max=7739746 ns, heap_slope=growing
  - component `capture_overhead`: median=8155 ns, median_frame_share=0.36%
  - component `central_graph`: median=1315386 ns, median_frame_share=59.07%
  - component `diagnostics`: median=70913 ns, median_frame_share=3.18%, nested_under=run_navigation
  - component `run_navigation`: median=285613 ns, median_frame_share=12.82%
  - component `selection_inspector`: median=405056 ns, median_frame_share=18.19%
  - component `timeline`: median=54702 ns, median_frame_share=2.45%
  - component `top_strip`: median=76493 ns, median_frame_share=3.43%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818001, median_wrapped_bytes=829312, median_live_object_bytes=73806
  - phase 2 `select_node`: frames=31..60, median_allocs=1242, median_object_bytes=908949, median_wrapped_bytes=920904, median_live_object_bytes=197704
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1242, median_object_bytes=908949, median_wrapped_bytes=920904, median_live_object_bytes=307515
  - phase 4 `expand_section`: frames=91..120, median_allocs=2139, median_object_bytes=1136435, median_wrapped_bytes=1156432, median_live_object_bytes=700899
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2139, median_object_bytes=1136436, median_wrapped_bytes=1156432, median_live_object_bytes=807670
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1245, median_object_bytes=909321, median_wrapped_bytes=921312, median_live_object_bytes=908041
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1245, median_object_bytes=909314, median_wrapped_bytes=921296, median_live_object_bytes=1017609
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1171, median_object_bytes=818613, median_wrapped_bytes=829976, median_live_object_bytes=1128852
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1171, median_object_bytes=818607, median_wrapped_bytes=829968, median_live_object_bytes=1235934
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=2213774 ns, p95=3615984 ns, p99=4160881 ns, max=4370843 ns, heap_slope=plateau
  - component `capture_overhead`: median=8065 ns, median_frame_share=0.36%
  - component `central_graph`: median=1311048 ns, median_frame_share=59.22%
  - component `diagnostics`: median=70311 ns, median_frame_share=3.17%, nested_under=run_navigation
  - component `run_navigation`: median=281596 ns, median_frame_share=12.72%
  - component `selection_inspector`: median=399707 ns, median_frame_share=18.05%
  - component `timeline`: median=53880 ns, median_frame_share=2.43%
  - component `top_strip`: median=75531 ns, median_frame_share=3.41%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818012, median_wrapped_bytes=829320, median_live_object_bytes=73953
  - phase 2 `select_node`: frames=31..60, median_allocs=1239, median_object_bytes=908165, median_wrapped_bytes=920112, median_live_object_bytes=195656
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1239, median_object_bytes=908172, median_wrapped_bytes=920112, median_live_object_bytes=304116
  - phase 4 `expand_section`: frames=91..120, median_allocs=2572, median_object_bytes=1194674, median_wrapped_bytes=1218608, median_live_object_bytes=502076
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2572, median_object_bytes=1194671, median_wrapped_bytes=1218600, median_live_object_bytes=608859
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1242, median_object_bytes=908547, median_wrapped_bytes=920520, median_live_object_bytes=705372
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1242, median_object_bytes=908539, median_wrapped_bytes=920512, median_live_object_bytes=829651
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1171, median_object_bytes=818611, median_wrapped_bytes=829968, median_live_object_bytes=942326
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1171, median_object_bytes=818606, median_wrapped_bytes=829968, median_live_object_bytes=1049708

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-f28b720e71b9-standard/standard.puffin`: 3438168 bytes, sha256 `53bdcdec51f2b0f1b6511f6da10be2023d5184cd1531f6a822b3258637739f97`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-f28b720e71b9-standard/inspector_run_records_phase_sequence_30.heap.json`: 14890 bytes, sha256 `5c9ffb853be0d0c865b92fd63e283cb732adfc090083c7f28f4fd9020d4a0318`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-f28b720e71b9-standard/inspector_run_records_phase_sequence_alternate_30.heap.json`: 12376 bytes, sha256 `4f3bebaff4f7a1ca7d9a6ed56e3338d40d5b75440e93c001a89daa31d66eadc1`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-f28b720e71b9-standard/inspector_llm_calls_phase_sequence_30.heap.json`: 11970 bytes, sha256 `35518c78535813e8ce5e1f48169cc34da30b8dd9f8cab983653d366ba10758c2`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-f28b720e71b9-standard/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 11960 bytes, sha256 `94dc0260c4bace1fbdc1e0721418680e350648427f23a745fc3dbe08d83953fd`

See `report.json` for typed timings and compact allocation summaries.
