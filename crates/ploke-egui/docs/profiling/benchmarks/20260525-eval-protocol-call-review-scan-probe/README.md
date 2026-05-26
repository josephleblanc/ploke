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
- `crates/ploke-egui/docs/profiling/benchmarks/20260525-eval-protocol-row-hover-samply-note.md`
- `proc_macros/ploke-egui-macros/`

## Callsite Sampling

- sample every: `32` matching allocations
- accounting: `scaled sampled estimates; callsite totals are not exact allocator totals`
- scopes:
  - `eframe_run_native`
  - `selection_inspector`
  - `eval_protocol`
  - `eval_protocol_call_review_scan`
  - `eval_protocol_protocol_drilldowns`
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

- `run_picker_discovery`: 13596 ns
- `FsRunStore::load`: 910900567 ns
- `FsRunStore::load_history_blocks`: 23808442 ns
- `FsRunStore::load_transition_journal`: 4743083 ns
- `FsRunStore::load_record_set`: 939455948 ns
- `compressed_run_record_profile_probe`: 115740665 ns
- `Graph::from_records`: 6460351 ns
- `graph_load_total`: 1061659629 ns
- note: run_picker_discovery=13596 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `eval_protocol_call_review_scan_300`: frames=300, median=3639736 ns, p95=3827724 ns, p99=5741789 ns, max=2014267471 ns, heap_slope=plateau
  - component `capture_overhead`: median=7514 ns, median_frame_share=0.20%
  - component `central_graph`: median=3064157 ns, median_frame_share=84.18%
  - component `run_navigation`: median=333586 ns, median_frame_share=9.16%
  - component `timeline`: median=116963 ns, median_frame_share=3.21%
  - component `top_strip`: median=101063 ns, median_frame_share=2.77%
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (79181056 wrapped bytes)

## Local Puffin Captures

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/puffin/benchmarks/20260525-eval-protocol-call-review-scan-probe/standard.puffin`: 1043189 bytes, sha256 `9027a4f5ee6d2dcae56b1d640c0af62e427cedc5e31ec4d34637f444ff9149a5`

## Local Heap Profiles

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/heap/benchmarks/20260525-eval-protocol-call-review-scan-probe/eval_protocol_call_review_scan_300.heap.json`: 87830 bytes, sha256 `756df35f0221e96cbb83a82b632e8736cd60d2183fb61a67f0394aa43377a16f`

See `report.json` for typed timings and compact allocation summaries.
