# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `44149346472b`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `Cargo.lock`
- `crates/ploke-egui/docs/profiling/20260518-nine-phase-selection-run-questions.md`
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-records/Cargo.toml`
- `crates/ploke-records/src/lib.rs`
- `crates/ploke-records/src/record.rs`
- `crates/ploke-records/src/llm_response.rs`

unrelated dirty paths:
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-focused-spans-benchmark-note.md`
- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/lib.rs`
- `crates/ploke-eval/src/record.rs`
- `crates/ploke-eval/src/tests/replay.rs`
- `crates/ploke-llm/src/lib.rs`
- `crates/ploke-llm/src/manager/mod.rs`
- `crates/ploke-llm/src/manager/session.rs`
- `crates/ploke-tui/src/llm/manager/mod.rs`
- `crates/ploke-tui/src/llm/manager/session.rs`
- `crates/ploke-tui/src/llm/mod.rs`
- `crates/ploke-tui/src/tools/mod.rs`
- `docs/active/plans/self-improvement-loop/typed-data-coverage-report.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/implementation-slices.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.jsonl`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/traceability-matrix.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-measurement-split-benchmark-note.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-measurement-split/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-phase-heap-deltas-benchmark-note.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-phase-heap-deltas/`
- `crates/ploke-eval/src/replay/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 9718 ns
- `FsRunStore::load`: 959661775 ns
- `FsRunStore::load_history_blocks`: 23591475 ns
- `FsRunStore::load_transition_journal`: 4707060 ns
- `FsRunStore::load_record_set`: 987964016 ns
- `compressed_run_record_profile_probe`: 114884636 ns
- `Graph::from_records`: 6449876 ns
- `graph_load_total`: 1109301313 ns
- note: run_picker_discovery=9718 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=1853183 ns, p95=2693287 ns, p99=24677740 ns, max=61239468 ns, heap_slope=growing
  - component `capture_overhead`: median=3076 ns, median_frame_share=0.16%
  - component `central_graph`: median=998301 ns, median_frame_share=53.86%
  - component `diagnostics`: median=69951 ns, median_frame_share=3.77%, nested_under=run_navigation
  - component `run_navigation`: median=278993 ns, median_frame_share=15.05%
  - component `selection_inspector`: median=371627 ns, median_frame_share=20.05%
  - component `timeline`: median=51827 ns, median_frame_share=2.79%
  - component `top_strip`: median=75552 ns, median_frame_share=4.07%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1154, median_object_bytes=860059, median_wrapped_bytes=871208, median_live_object_bytes=1677440
  - phase 2 `select_node`: frames=31..60, median_allocs=1227, median_object_bytes=941076, median_wrapped_bytes=952856, median_live_object_bytes=1880231
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1227, median_object_bytes=941073, median_wrapped_bytes=952856, median_live_object_bytes=1987436
  - phase 4 `expand_section`: frames=91..120, median_allocs=1463, median_object_bytes=1121066, median_wrapped_bytes=1134752, median_live_object_bytes=2330527
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1463, median_object_bytes=1121062, median_wrapped_bytes=1134752, median_live_object_bytes=2435935
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1233, median_object_bytes=941878, median_wrapped_bytes=953728, median_live_object_bytes=2541963
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1233, median_object_bytes=941867, median_wrapped_bytes=953720, median_live_object_bytes=2648303
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1159, median_object_bytes=851175, median_wrapped_bytes=862384, median_live_object_bytes=2756443
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1159, median_object_bytes=851170, median_wrapped_bytes=862376, median_live_object_bytes=2860769
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=1850878 ns, p95=2631732 ns, p99=2943315 ns, max=4222071 ns, heap_slope=plateau
  - component `capture_overhead`: median=3075 ns, median_frame_share=0.16%
  - component `central_graph`: median=997408 ns, median_frame_share=53.88%
  - component `diagnostics`: median=69981 ns, median_frame_share=3.78%, nested_under=run_navigation
  - component `run_navigation`: median=279764 ns, median_frame_share=15.11%
  - component `selection_inspector`: median=370684 ns, median_frame_share=20.02%
  - component `timeline`: median=51647 ns, median_frame_share=2.79%
  - component `top_strip`: median=75731 ns, median_frame_share=4.09%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1153, median_object_bytes=850378, median_wrapped_bytes=861520, median_live_object_bytes=88758
  - phase 2 `select_node`: frames=31..60, median_allocs=1224, median_object_bytes=940308, median_wrapped_bytes=952072, median_live_object_bytes=235584
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1224, median_object_bytes=940293, median_wrapped_bytes=952056, median_live_object_bytes=340633
  - phase 4 `expand_section`: frames=91..120, median_allocs=1461, median_object_bytes=1120801, median_wrapped_bytes=1134456, median_live_object_bytes=612860
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1461, median_object_bytes=1120810, median_wrapped_bytes=1134464, median_live_object_bytes=717273
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1228, median_object_bytes=940849, median_wrapped_bytes=952648, median_live_object_bytes=822837
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1228, median_object_bytes=940839, median_wrapped_bytes=952640, median_live_object_bytes=931209
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1157, median_object_bytes=850910, median_wrapped_bytes=862088, median_live_object_bytes=1037778
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1157, median_object_bytes=850916, median_wrapped_bytes=862096, median_live_object_bytes=1142078
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=1854184 ns, p95=2898702 ns, p99=3296437 ns, max=6380035 ns, heap_slope=growing
  - component `capture_overhead`: median=3095 ns, median_frame_share=0.16%
  - component `central_graph`: median=999814 ns, median_frame_share=53.92%
  - component `diagnostics`: median=70001 ns, median_frame_share=3.77%, nested_under=run_navigation
  - component `run_navigation`: median=279765 ns, median_frame_share=15.08%
  - component `selection_inspector`: median=370604 ns, median_frame_share=19.98%
  - component `timeline`: median=52007 ns, median_frame_share=2.80%
  - component `top_strip`: median=75581 ns, median_frame_share=4.07%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1153, median_object_bytes=850372, median_wrapped_bytes=861512, median_live_object_bytes=88844
  - phase 2 `select_node`: frames=31..60, median_allocs=1227, median_object_bytes=941069, median_wrapped_bytes=952856, median_live_object_bytes=209561
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1227, median_object_bytes=941078, median_wrapped_bytes=952864, median_live_object_bytes=316097
  - phase 4 `expand_section`: frames=91..120, median_allocs=2123, median_object_bytes=1168441, median_wrapped_bytes=1188240, median_live_object_bytes=706470
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2123, median_object_bytes=1168439, median_wrapped_bytes=1188232, median_live_object_bytes=810160
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1229, median_object_bytes=941333, median_wrapped_bytes=953136, median_live_object_bytes=907303
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1229, median_object_bytes=941324, median_wrapped_bytes=953128, median_live_object_bytes=1014016
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1155, median_object_bytes=850615, median_wrapped_bytes=861776, median_live_object_bytes=1122154
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1155, median_object_bytes=850621, median_wrapped_bytes=861784, median_live_object_bytes=1226211
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=1853232 ns, p95=3505890 ns, p99=3698781 ns, max=3987863 ns, heap_slope=plateau
  - component `capture_overhead`: median=3076 ns, median_frame_share=0.16%
  - component `central_graph`: median=998872 ns, median_frame_share=53.89%
  - component `diagnostics`: median=69972 ns, median_frame_share=3.77%, nested_under=run_navigation
  - component `run_navigation`: median=278842 ns, median_frame_share=15.04%
  - component `selection_inspector`: median=369152 ns, median_frame_share=19.91%
  - component `timeline`: median=51887 ns, median_frame_share=2.79%
  - component `top_strip`: median=75641 ns, median_frame_share=4.08%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1153, median_object_bytes=850376, median_wrapped_bytes=861520, median_live_object_bytes=88783
  - phase 2 `select_node`: frames=31..60, median_allocs=1224, median_object_bytes=940297, median_wrapped_bytes=952064, median_live_object_bytes=206899
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1224, median_object_bytes=940296, median_wrapped_bytes=952056, median_live_object_bytes=312376
  - phase 4 `expand_section`: frames=91..120, median_allocs=2556, median_object_bytes=1226681, median_wrapped_bytes=1250408, median_live_object_bytes=506989
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2556, median_object_bytes=1226681, median_wrapped_bytes=1250408, median_live_object_bytes=610750
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1226, median_object_bytes=940555, median_wrapped_bytes=952336, median_live_object_bytes=704465
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1226, median_object_bytes=940536, median_wrapped_bytes=952320, median_live_object_bytes=825901
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1155, median_object_bytes=850624, median_wrapped_bytes=861784, median_live_object_bytes=935379
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1155, median_object_bytes=850625, median_wrapped_bytes=861784, median_live_object_bytes=1039885

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-llm-calls-phase-heap-deltas/standard.puffin`: 3325575 bytes, sha256 `5d1628c746813eabba30f5d0e28117c7526960746c1b73700910dcb1f55bfeb0`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-llm-calls-phase-heap-deltas/inspector_run_records_phase_sequence_30.heap.json`: 6634 bytes, sha256 `39e9bd347f991f08312f644c29ad1020546c18a4990b259a09901fb25dfcaa27`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-llm-calls-phase-heap-deltas/inspector_run_records_phase_sequence_alternate_30.heap.json`: 5762 bytes, sha256 `dcdcc627303b2deb79d2f11d1d58640eb4e4bf8094f33f66e36527c5c7ff4337`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-llm-calls-phase-heap-deltas/inspector_llm_calls_phase_sequence_30.heap.json`: 4944 bytes, sha256 `bf89a6fd5192262253a1c288f4e8f0d4c98d17598b58cb172d5e2f4dd24451cb`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-llm-calls-phase-heap-deltas/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 4942 bytes, sha256 `080782385b98ab86c43672b19957f55fb0d4fcbcff11e3cd25d2a182198d8084`

See `report.json` for typed timings and compact allocation summaries.
