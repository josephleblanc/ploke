# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `fe0dd2bd7679`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `Cargo.lock`
- `Cargo.toml`
- `crates/ploke-egui/Cargo.toml`
- `crates/ploke-egui/docs/README.md`
- `crates/ploke-egui/docs/model/README.md`
- `crates/ploke-egui/docs/plan/README.md`
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/ui/app/layout.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/charts/bar.rs`
- `crates/ploke-egui/src/ui/charts/mod.rs`
- `crates/ploke-egui/src/ui/dashboard/tiles.rs`
- `crates/ploke-egui/src/ui/eval_protocol.rs`
- `crates/ploke-egui/src/ui/id_display/mod.rs`
- `crates/ploke-egui/src/ui/text/mod.rs`
- `crates/ploke-egui/docs/model/analyst-representation.md`
- `crates/ploke-egui/docs/plan/eval-protocol-analyst-surface/`
- `crates/ploke-egui/docs/style/`
- `crates/ploke-egui/src/ui/text/style.rs`

unrelated dirty paths:
- `crates/ploke-egui/docs/profiling/benchmarks/README.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260525-eval-protocol-allocation-probe/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260525-eval-protocol-call-review-scan-probe-r2/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260525-eval-protocol-call-review-scan-probe-r3/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260525-eval-protocol-call-review-scan-probe/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260525-eval-protocol-row-hover-samply-note.md`
- `proc_macros/ploke-egui-macros/`

## Callsite Sampling

- sample every: `32` matching allocations
- accounting: `scaled sampled estimates; callsite totals are not exact allocator totals`
- scopes:
  - `eframe_run_native`
  - `selection_inspector`
  - `eval_protocol_visual_summary`
  - `eval_protocol_call_review_scan`
  - `eval_protocol_call_review_spotlight`
  - `eval_protocol_call_review_row`
  - `inspector_agent_trace_llm_trace`
  - `inspector_patch_generation_record`
  - `inspector_run_record_tool_steps`
  - `central_graph_widget_add`
  - `inspector_run_record_tool_step`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 8987 ns
- `FsRunStore::load`: 904340064 ns
- `FsRunStore::load_history_blocks`: 23912705 ns
- `FsRunStore::load_transition_journal`: 4646353 ns
- `FsRunStore::load_record_set`: 932902928 ns
- `compressed_run_record_profile_probe`: 116695139 ns
- `Graph::from_records`: 6327739 ns
- `graph_load_total`: 1055928270 ns
- note: run_picker_discovery=8987 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `eval_protocol_call_review_scan_300`: frames=300, median=1433440 ns, p95=1648645 ns, p99=1673472 ns, max=74177708 ns, heap_slope=plateau
  - component `capture_overhead`: median=7585 ns, median_frame_share=0.52%
  - component `central_graph`: median=850171 ns, median_frame_share=59.30%
  - component `run_navigation`: median=337856 ns, median_frame_share=23.56%
  - component `timeline`: median=118814 ns, median_frame_share=8.28%
  - component `top_strip`: median=103345 ns, median_frame_share=7.20%
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (56584448 wrapped bytes)

## Local Puffin Captures

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/puffin/benchmarks/20260525-eval-protocol-call-review-scan-probe-r4/standard.puffin`: 1033887 bytes, sha256 `addbe0407299550ff6fadce5166bfbc76318dd423bd8aca75f6bc9b2e47f8125`

## Local Heap Profiles

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/heap/benchmarks/20260525-eval-protocol-call-review-scan-probe-r4/eval_protocol_call_review_scan_300.heap.json`: 81790 bytes, sha256 `e27ca3d879baf652dda7f6f74d1aa547924bad71992fc5e87e86cfbfb521b159`

See `report.json` for typed timings and compact allocation summaries.
