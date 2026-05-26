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

- `run_picker_discovery`: 9889 ns
- `FsRunStore::load`: 961183425 ns
- `FsRunStore::load_history_blocks`: 30359549 ns
- `FsRunStore::load_transition_journal`: 4557025 ns
- `FsRunStore::load_record_set`: 996106571 ns
- `compressed_run_record_profile_probe`: 118969232 ns
- `Graph::from_records`: 7375864 ns
- `graph_load_total`: 1122456766 ns
- note: run_picker_discovery=9889 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `warm_idle_300`: frames=300, median=3782414 ns, p95=4010000 ns, p99=6258712 ns, max=2035446532 ns, heap_slope=plateau
  - component `capture_overhead`: median=7754 ns, median_frame_share=0.20%
  - component `central_graph`: median=3191597 ns, median_frame_share=84.37%
  - component `run_navigation`: median=339295 ns, median_frame_share=8.97%
  - component `timeline`: median=118753 ns, median_frame_share=3.13%
  - component `top_strip`: median=103524 ns, median_frame_share=2.73%
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (130894080 wrapped bytes)

## Local Puffin Captures

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/puffin/benchmarks/20260525-eval-protocol-allocation-probe/standard.puffin`: 1035048 bytes, sha256 `6eb8ef2603cf2595d5fe29b39e0ebade07592a2ba03d718da20c67b03415ae9f`

## Local Heap Profiles

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/heap/benchmarks/20260525-eval-protocol-allocation-probe/warm_idle_300.heap.json`: 85622 bytes, sha256 `8587c070e48ca8c2f45fd2456e4be84a8694792a22b285fc2b57991ca9e64eae`

See `report.json` for typed timings and compact allocation summaries.
