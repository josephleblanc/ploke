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

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 8105 ns
- `FsRunStore::load`: 1010469262 ns
- `FsRunStore::load_history_blocks`: 23519905 ns
- `FsRunStore::load_transition_journal`: 4642591 ns
- `FsRunStore::load_record_set`: 1038636547 ns
- `compressed_run_record_profile_probe`: 115217085 ns
- `Graph::from_records`: 6839373 ns
- `graph_load_total`: 1160695881 ns
- note: run_picker_discovery=8105 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=2064963 ns, p95=2292692 ns, p99=2709496 ns, max=62596177 ns, heap_slope=plateau
  - component `capture_overhead`: median=7975 ns, median_frame_share=0.38%
  - component `central_graph`: median=1316406 ns, median_frame_share=63.74%
  - component `diagnostics`: median=69561 ns, median_frame_share=3.36%, nested_under=run_navigation
  - component `run_navigation`: median=283763 ns, median_frame_share=13.74%
  - component `selection_inspector`: median=251172 ns, median_frame_share=12.16%
  - component `timeline`: median=51948 ns, median_frame_share=2.51%
  - component `top_strip`: median=75392 ns, median_frame_share=3.65%
- `warm_idle_300`: frames=300, median=2062529 ns, p95=2309914 ns, p99=2616490 ns, max=3146288 ns, heap_slope=plateau
  - component `capture_overhead`: median=7945 ns, median_frame_share=0.38%
  - component `central_graph`: median=1315935 ns, median_frame_share=63.80%
  - component `diagnostics`: median=69250 ns, median_frame_share=3.35%, nested_under=run_navigation
  - component `run_navigation`: median=280407 ns, median_frame_share=13.59%
  - component `selection_inspector`: median=249950 ns, median_frame_share=12.11%
  - component `timeline`: median=51777 ns, median_frame_share=2.51%
  - component `top_strip`: median=75081 ns, median_frame_share=3.64%
- `inspector_run_records_phase_sequence_30`: frames=270, median=2219203 ns, p95=3133243 ns, p99=10049761 ns, max=35330867 ns, heap_slope=growing
  - component `capture_overhead`: median=8016 ns, median_frame_share=0.36%
  - component `central_graph`: median=1321215 ns, median_frame_share=59.53%
  - component `diagnostics`: median=69160 ns, median_frame_share=3.11%, nested_under=run_navigation
  - component `run_navigation`: median=279606 ns, median_frame_share=12.59%
  - component `selection_inspector`: median=399892 ns, median_frame_share=18.01%
  - component `timeline`: median=53210 ns, median_frame_share=2.39%
  - component `top_strip`: median=74981 ns, median_frame_share=3.37%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1167, median_object_bytes=827687, median_wrapped_bytes=839008, median_live_object_bytes=73903
  - phase 2 `select_node`: frames=31..60, median_allocs=1243, median_object_bytes=909066, median_wrapped_bytes=921032, median_live_object_bytes=371313
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1243, median_object_bytes=909062, median_wrapped_bytes=921032, median_live_object_bytes=480816
  - phase 4 `expand_section`: frames=91..120, median_allocs=1478, median_object_bytes=1088910, median_wrapped_bytes=1102784, median_live_object_bytes=832881
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1478, median_object_bytes=1088910, median_wrapped_bytes=1102784, median_live_object_bytes=940042
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1248, median_object_bytes=909727, median_wrapped_bytes=921752, median_live_object_bytes=1049014
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1248, median_object_bytes=909709, median_wrapped_bytes=921736, median_live_object_bytes=1158384
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1174, median_object_bytes=819025, median_wrapped_bytes=830424, median_live_object_bytes=1269443
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1174, median_object_bytes=819016, median_wrapped_bytes=830416, median_live_object_bytes=1376649
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=2219864 ns, p95=3293163 ns, p99=3888263 ns, max=4708165 ns, heap_slope=plateau
  - component `capture_overhead`: median=8025 ns, median_frame_share=0.36%
  - component `central_graph`: median=1321866 ns, median_frame_share=59.54%
  - component `diagnostics`: median=69280 ns, median_frame_share=3.12%, nested_under=run_navigation
  - component `run_navigation`: median=280237 ns, median_frame_share=12.62%
  - component `selection_inspector`: median=401485 ns, median_frame_share=18.08%
  - component `timeline`: median=53591 ns, median_frame_share=2.41%
  - component `top_strip`: median=75272 ns, median_frame_share=3.39%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818001, median_wrapped_bytes=829312, median_live_object_bytes=73846
  - phase 2 `select_node`: frames=31..60, median_allocs=1240, median_object_bytes=908289, median_wrapped_bytes=920240, median_live_object_bytes=223359
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1240, median_object_bytes=908285, median_wrapped_bytes=920240, median_live_object_bytes=331868
  - phase 4 `expand_section`: frames=91..120, median_allocs=1476, median_object_bytes=1088659, median_wrapped_bytes=1102504, median_live_object_bytes=606944
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1476, median_object_bytes=1088666, median_wrapped_bytes=1102512, median_live_object_bytes=730578
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1243, median_object_bytes=908695, median_wrapped_bytes=920680, median_live_object_bytes=838938
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1243, median_object_bytes=908689, median_wrapped_bytes=920672, median_live_object_bytes=947455
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1172, median_object_bytes=818768, median_wrapped_bytes=830136, median_live_object_bytes=1059913
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1172, median_object_bytes=818764, median_wrapped_bytes=830136, median_live_object_bytes=1167053
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=2228741 ns, p95=3171064 ns, p99=3499050 ns, max=6966391 ns, heap_slope=growing
  - component `capture_overhead`: median=8065 ns, median_frame_share=0.36%
  - component `central_graph`: median=1322507 ns, median_frame_share=59.33%
  - component `diagnostics`: median=69311 ns, median_frame_share=3.10%, nested_under=run_navigation
  - component `run_navigation`: median=281299 ns, median_frame_share=12.62%
  - component `selection_inspector`: median=401385 ns, median_frame_share=18.00%
  - component `timeline`: median=53511 ns, median_frame_share=2.40%
  - component `top_strip`: median=75572 ns, median_frame_share=3.39%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818004, median_wrapped_bytes=829312, median_live_object_bytes=73688
  - phase 2 `select_node`: frames=31..60, median_allocs=1242, median_object_bytes=908944, median_wrapped_bytes=920896, median_live_object_bytes=197918
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1242, median_object_bytes=908948, median_wrapped_bytes=920904, median_live_object_bytes=307481
  - phase 4 `expand_section`: frames=91..120, median_allocs=2139, median_object_bytes=1136425, median_wrapped_bytes=1156424, median_live_object_bytes=700677
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2139, median_object_bytes=1136430, median_wrapped_bytes=1156424, median_live_object_bytes=807483
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1245, median_object_bytes=909317, median_wrapped_bytes=921304, median_live_object_bytes=907636
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1245, median_object_bytes=909304, median_wrapped_bytes=921288, median_live_object_bytes=1016861
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1171, median_object_bytes=818606, median_wrapped_bytes=829968, median_live_object_bytes=1128074
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1171, median_object_bytes=818608, median_wrapped_bytes=829968, median_live_object_bytes=1238040
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=3716439 ns, p95=8018230 ns, p99=10029152 ns, max=11080299 ns, heap_slope=plateau
  - component `capture_overhead`: median=13315 ns, median_frame_share=0.35%
  - component `central_graph`: median=2040137 ns, median_frame_share=54.89%
  - component `diagnostics`: median=116889 ns, median_frame_share=3.14%, nested_under=run_navigation
  - component `run_navigation`: median=473019 ns, median_frame_share=12.72%
  - component `selection_inspector`: median=527211 ns, median_frame_share=14.18%
  - component `timeline`: median=89408 ns, median_frame_share=2.40%
  - component `top_strip`: median=121769 ns, median_frame_share=3.27%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818005, median_wrapped_bytes=829312, median_live_object_bytes=73887
  - phase 2 `select_node`: frames=31..60, median_allocs=1239, median_object_bytes=908172, median_wrapped_bytes=920112, median_live_object_bytes=195480
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1239, median_object_bytes=908173, median_wrapped_bytes=920120, median_live_object_bytes=304159
  - phase 4 `expand_section`: frames=91..120, median_allocs=2572, median_object_bytes=1194672, median_wrapped_bytes=1218600, median_live_object_bytes=501949
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2572, median_object_bytes=1194716, median_wrapped_bytes=1218648, median_live_object_bytes=609348
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1243, median_object_bytes=908608, median_wrapped_bytes=920592, median_live_object_bytes=707333
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1242, median_object_bytes=908585, median_wrapped_bytes=920560, median_live_object_bytes=817202
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1171, median_object_bytes=818670, median_wrapped_bytes=830032, median_live_object_bytes=931358
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1171, median_object_bytes=818654, median_wrapped_bytes=830016, median_live_object_bytes=1039903

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-eframe-root-attribution/standard.puffin`: 5322038 bytes, sha256 `8523575bbbfbb15f409c1d07623875b4ce6a48f736a4b5f1fa5bf94a5014addb`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-eframe-root-attribution/startup_frames_300.heap.json`: 11587 bytes, sha256 `4424e357c52b1421ddd504a608bec4b7dfeb7b8db47bd4d4705da09008be4b9f`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-eframe-root-attribution/warm_idle_300.heap.json`: 9893 bytes, sha256 `74ab2412d1ec60f047d17d8e2fc6d8182d3d43fd4a0ce88c2de35ab0da6b0198`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-eframe-root-attribution/inspector_run_records_phase_sequence_30.heap.json`: 13218 bytes, sha256 `cd0089bfe0bc356e61cd6a1411be9303c92f3f5cb97b6685a4aa125c172208ce`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-eframe-root-attribution/inspector_run_records_phase_sequence_alternate_30.heap.json`: 12376 bytes, sha256 `99171f35bc7c8a7c454959042fe002c45c4c24e57c08c141228dc1a54c63c8e2`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-eframe-root-attribution/inspector_llm_calls_phase_sequence_30.heap.json`: 11970 bytes, sha256 `8c8e24d5c925f98d2234b670485f37a3c2fb1aca17ac15e9fd9962222be4d535`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-eframe-root-attribution/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 11960 bytes, sha256 `ffc61f80478c46073691cbceb8ad70a4f0787b709a0427b17002ab9bcc4d6727`

See `report.json` for typed timings and compact allocation summaries.
