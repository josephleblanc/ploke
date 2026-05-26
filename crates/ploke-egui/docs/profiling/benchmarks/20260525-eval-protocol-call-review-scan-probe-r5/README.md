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
- `crates/ploke-egui/docs/profiling/benchmarks/20260525-eval-protocol-call-review-scan-probe-r4/`
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

- `run_picker_discovery`: 10259 ns
- `FsRunStore::load`: 915184280 ns
- `FsRunStore::load_history_blocks`: 23481260 ns
- `FsRunStore::load_transition_journal`: 4442029 ns
- `FsRunStore::load_record_set`: 943111939 ns
- `compressed_run_record_profile_probe`: 116890621 ns
- `Graph::from_records`: 6526513 ns
- `graph_load_total`: 1066531989 ns
- note: run_picker_discovery=10259 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `eval_protocol_call_review_scan_300`: frames=300, median=1445750 ns, p95=1674922 ns, p99=1793355 ns, max=72896296 ns, heap_slope=plateau
  - component `capture_overhead`: median=7534 ns, median_frame_share=0.52%
  - component `central_graph`: median=860369 ns, median_frame_share=59.51%
  - component `run_navigation`: median=340110 ns, median_frame_share=23.52%
  - component `timeline`: median=119245 ns, median_frame_share=8.24%
  - component `top_strip`: median=103435 ns, median_frame_share=7.15%
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (57748736 wrapped bytes)

## Local Puffin Captures

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/puffin/benchmarks/20260525-eval-protocol-call-review-scan-probe-r5/standard.puffin`: 1033057 bytes, sha256 `6ff01248efb81d597ebf47db277c9ff9a03314da321edd76a8c9000a6d9270e1`

## Local Heap Profiles

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/heap/benchmarks/20260525-eval-protocol-call-review-scan-probe-r5/eval_protocol_call_review_scan_300.heap.json`: 82256 bytes, sha256 `783e894b1486e1efc5670b2913a9644b9fe762055a5c2cd72b5b9f8e67c1b278`

See `report.json` for typed timings and compact allocation summaries.
