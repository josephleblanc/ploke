# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `69f4201e1091`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `Cargo.lock`
- `crates/ploke-egui/docs/profiling/20260518-nine-phase-selection-run-questions.md`
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/view/mod.rs`
- `crates/ploke-records/Cargo.toml`
- `crates/ploke-records/src/lib.rs`
- `crates/ploke-records/src/record.rs`
- `crates/ploke-records/src/llm_response.rs`

unrelated dirty paths:
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-focused-spans-benchmark-note.md`
- `crates/ploke-llm/src/lib.rs`
- `crates/ploke-llm/src/manager/mod.rs`
- `crates/ploke-llm/src/manager/session.rs`
- `crates/ploke-tui/src/llm/manager/mod.rs`
- `crates/ploke-tui/src/llm/manager/session.rs`
- `crates/ploke-tui/src/llm/mod.rs`
- `docs/active/plans/self-improvement-loop/typed-data-coverage-report.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/implementation-slices.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.jsonl`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/traceability-matrix.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-llm-calls-phase-heap-deltas/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-measurement-split-benchmark-note.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-measurement-split/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-phase-heap-deltas-benchmark-note.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-phase-heap-deltas/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 10710 ns
- `FsRunStore::load`: 943047843 ns
- `FsRunStore::load_history_blocks`: 24763033 ns
- `FsRunStore::load_transition_journal`: 4751412 ns
- `FsRunStore::load_record_set`: 972566596 ns
- `compressed_run_record_profile_probe`: 117444786 ns
- `Graph::from_records`: 6903019 ns
- `graph_load_total`: 1096916946 ns
- note: run_picker_discovery=10710 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=1720189 ns, p95=1920684 ns, p99=2226156 ns, max=62706414 ns, heap_slope=plateau
  - component `capture_overhead`: median=3046 ns, median_frame_share=0.17%
  - component `central_graph`: median=1012204 ns, median_frame_share=58.84%
  - component `diagnostics`: median=69751 ns, median_frame_share=4.05%, nested_under=run_navigation
  - component `run_navigation`: median=280385 ns, median_frame_share=16.29%
  - component `selection_inspector`: median=227606 ns, median_frame_share=13.23%
  - component `timeline`: median=49573 ns, median_frame_share=2.88%
  - component `top_strip`: median=76143 ns, median_frame_share=4.42%
- `warm_idle_300`: frames=300, median=1717545 ns, p95=1916307 ns, p99=1993230 ns, max=2682541 ns, heap_slope=plateau
  - component `capture_overhead`: median=2975 ns, median_frame_share=0.17%
  - component `central_graph`: median=1013046 ns, median_frame_share=58.98%
  - component `diagnostics`: median=69410 ns, median_frame_share=4.04%, nested_under=run_navigation
  - component `run_navigation`: median=278331 ns, median_frame_share=16.20%
  - component `selection_inspector`: median=226665 ns, median_frame_share=13.19%
  - component `timeline`: median=49373 ns, median_frame_share=2.87%
  - component `top_strip`: median=75701 ns, median_frame_share=4.40%
- `inspector_run_records_phase_sequence_30`: frames=270, median=1870591 ns, p95=2638208 ns, p99=9338527 ns, max=34511912 ns, heap_slope=growing
  - component `capture_overhead`: median=3086 ns, median_frame_share=0.16%
  - component `central_graph`: median=1016141 ns, median_frame_share=54.32%
  - component `diagnostics`: median=69590 ns, median_frame_share=3.72%, nested_under=run_navigation
  - component `run_navigation`: median=278612 ns, median_frame_share=14.89%
  - component `selection_inspector`: median=373148 ns, median_frame_share=19.94%
  - component `timeline`: median=51857 ns, median_frame_share=2.77%
  - component `top_strip`: median=76012 ns, median_frame_share=4.06%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1155, median_object_bytes=826187, median_wrapped_bytes=837360, median_live_object_bytes=71523
  - phase 2 `select_node`: frames=31..60, median_allocs=1229, median_object_bytes=907336, median_wrapped_bytes=919136, median_live_object_bytes=368968
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1229, median_object_bytes=907332, median_wrapped_bytes=919128, median_live_object_bytes=478404
  - phase 4 `expand_section`: frames=91..120, median_allocs=1464, median_object_bytes=1087186, median_wrapped_bytes=1100896, median_live_object_bytes=830018
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1464, median_object_bytes=1087185, median_wrapped_bytes=1100896, median_live_object_bytes=937617
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1234, median_object_bytes=908003, median_wrapped_bytes=919856, median_live_object_bytes=1046676
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1234, median_object_bytes=907988, median_wrapped_bytes=919848, median_live_object_bytes=1155929
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1160, median_object_bytes=817287, median_wrapped_bytes=828520, median_live_object_bytes=1266923
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1160, median_object_bytes=817292, median_wrapped_bytes=828528, median_live_object_bytes=1374000
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=1869499 ns, p95=2631766 ns, p99=2895039 ns, max=4296030 ns, heap_slope=plateau
  - component `capture_overhead`: median=3076 ns, median_frame_share=0.16%
  - component `central_graph`: median=1016283 ns, median_frame_share=54.36%
  - component `diagnostics`: median=69591 ns, median_frame_share=3.72%, nested_under=run_navigation
  - component `run_navigation`: median=278982 ns, median_frame_share=14.92%
  - component `selection_inspector`: median=370824 ns, median_frame_share=19.83%
  - component `timeline`: median=51867 ns, median_frame_share=2.77%
  - component `top_strip`: median=75972 ns, median_frame_share=4.06%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1154, median_object_bytes=816521, median_wrapped_bytes=827688, median_live_object_bytes=72931
  - phase 2 `select_node`: frames=31..60, median_allocs=1226, median_object_bytes=906554, median_wrapped_bytes=918344, median_live_object_bytes=222474
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1226, median_object_bytes=906557, median_wrapped_bytes=918344, median_live_object_bytes=330852
  - phase 4 `expand_section`: frames=91..120, median_allocs=1462, median_object_bytes=1086930, median_wrapped_bytes=1100608, median_live_object_bytes=605946
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1462, median_object_bytes=1086922, median_wrapped_bytes=1100600, median_live_object_bytes=729108
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1229, median_object_bytes=906971, median_wrapped_bytes=918784, median_live_object_bytes=837482
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1229, median_object_bytes=906959, median_wrapped_bytes=918776, median_live_object_bytes=946150
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1158, median_object_bytes=817043, median_wrapped_bytes=828248, median_live_object_bytes=1058695
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1158, median_object_bytes=817036, median_wrapped_bytes=828240, median_live_object_bytes=1165708

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-root-attribution-targets/standard.puffin`: 3605472 bytes, sha256 `5437e0d35399fea6d78739904686083e8372b62a1f8d2ae5d5a192067a9437d9`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-root-attribution-targets/startup_frames_300.heap.json`: 4964 bytes, sha256 `08819652780ff4ded9fbcc98896b7af0de388d90d7ff1c960a205c1bc25e735d`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-root-attribution-targets/warm_idle_300.heap.json`: 4910 bytes, sha256 `58aaa7d64cdef201683a2afa55c0864000edaf2f1ed460ae2e84d0c60d698434`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-root-attribution-targets/inspector_run_records_phase_sequence_30.heap.json`: 7403 bytes, sha256 `5d2c6a7549025a76f281c4befdae8b69e8fa88d504a81d7ff50c29234ed6a634`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-root-attribution-targets/inspector_run_records_phase_sequence_alternate_30.heap.json`: 6569 bytes, sha256 `b7337494cba2a555303ed80a8b317ddb39f33fc66aac952b7f6019ee92a2d648`

See `report.json` for typed timings and compact allocation summaries.
