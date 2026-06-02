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
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-phase-heap-deltas/`
- `crates/ploke-eval/src/replay/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 11211 ns
- `FsRunStore::load`: 917294451 ns
- `FsRunStore::load_history_blocks`: 24112070 ns
- `FsRunStore::load_transition_journal`: 4589300 ns
- `FsRunStore::load_record_set`: 946000620 ns
- `compressed_run_record_profile_probe`: 114992414 ns
- `Graph::from_records`: 6794626 ns
- `graph_load_total`: 1067790515 ns
- note: run_picker_discovery=11211 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=1883187 ns, p95=2678429 ns, p99=25994075 ns, max=60701852 ns, heap_slope=growing
  - component `capture_overhead`: median=3086 ns, median_frame_share=0.16%
  - component `central_graph`: median=1014517 ns, median_frame_share=53.87%
  - component `diagnostics`: median=69822 ns, median_frame_share=3.70%, nested_under=run_navigation
  - component `run_navigation`: median=281442 ns, median_frame_share=14.94%
  - component `selection_inspector`: median=373215 ns, median_frame_share=19.81%
  - component `timeline`: median=51848 ns, median_frame_share=2.75%
  - component `top_strip`: median=76174 ns, median_frame_share=4.04%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1154, median_object_bytes=860060, median_wrapped_bytes=871208, median_live_object_bytes=1677520
  - phase 2 `select_node`: frames=31..60, median_allocs=1227, median_object_bytes=941510, median_wrapped_bytes=953304, median_live_object_bytes=1881259
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1227, median_object_bytes=941083, median_wrapped_bytes=952864, median_live_object_bytes=1988911
  - phase 4 `expand_section`: frames=91..120, median_allocs=1463, median_object_bytes=1121064, median_wrapped_bytes=1134752, median_live_object_bytes=2332866
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1463, median_object_bytes=1121065, median_wrapped_bytes=1134752, median_live_object_bytes=2437501
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1234, median_object_bytes=942278, median_wrapped_bytes=954128, median_live_object_bytes=2544208
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1233, median_object_bytes=941880, median_wrapped_bytes=953728, median_live_object_bytes=2651266
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1159, median_object_bytes=851175, median_wrapped_bytes=862384, median_live_object_bytes=2759669
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1159, median_object_bytes=851164, median_wrapped_bytes=862376, median_live_object_bytes=2863995
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=1873779 ns, p95=2717704 ns, p99=3245050 ns, max=4443974 ns, heap_slope=plateau
  - component `capture_overhead`: median=3056 ns, median_frame_share=0.16%
  - component `central_graph`: median=1015097 ns, median_frame_share=54.17%
  - component `diagnostics`: median=69731 ns, median_frame_share=3.72%, nested_under=run_navigation
  - component `run_navigation`: median=281772 ns, median_frame_share=15.03%
  - component `selection_inspector`: median=373776 ns, median_frame_share=19.94%
  - component `timeline`: median=51648 ns, median_frame_share=2.75%
  - component `top_strip`: median=75943 ns, median_frame_share=4.05%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1153, median_object_bytes=850379, median_wrapped_bytes=861520, median_live_object_bytes=89012
  - phase 2 `select_node`: frames=31..60, median_allocs=1224, median_object_bytes=940662, median_wrapped_bytes=952456, median_live_object_bytes=236092
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1224, median_object_bytes=940304, median_wrapped_bytes=952064, median_live_object_bytes=341894
  - phase 4 `expand_section`: frames=91..120, median_allocs=1461, median_object_bytes=1120813, median_wrapped_bytes=1134472, median_live_object_bytes=614489
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1461, median_object_bytes=1120810, median_wrapped_bytes=1134464, median_live_object_bytes=718528
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1228, median_object_bytes=940848, median_wrapped_bytes=952648, median_live_object_bytes=824052
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1228, median_object_bytes=940839, median_wrapped_bytes=952640, median_live_object_bytes=932106
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1157, median_object_bytes=850915, median_wrapped_bytes=862096, median_live_object_bytes=1038905
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1157, median_object_bytes=850915, median_wrapped_bytes=862096, median_live_object_bytes=1143270

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-run-records-phase-heap-deltas/standard.puffin`: 1676980 bytes, sha256 `e58f188fe23ddef3d09b21bf05176db619996041aa0039f4ff6cde02b525b2fb`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-records-phase-heap-deltas/inspector_run_records_phase_sequence_30.heap.json`: 6634 bytes, sha256 `0923b9e2345c402ca89e58b35daf72fbb9f87e77ece82bc3842497baf894056d`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-records-phase-heap-deltas/inspector_run_records_phase_sequence_alternate_30.heap.json`: 5762 bytes, sha256 `4bc1108a5a903fdf39f153590b18ea46d20b55aecde5af348b0b192abbe1c381`

See `report.json` for typed timings and compact allocation summaries.
