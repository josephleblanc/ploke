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

- `run_picker_discovery`: 6683 ns
- `FsRunStore::load`: 902982818 ns
- `FsRunStore::load_history_blocks`: 24298821 ns
- `FsRunStore::load_transition_journal`: 4686941 ns
- `FsRunStore::load_record_set`: 931972687 ns
- `compressed_run_record_profile_probe`: 118010071 ns
- `Graph::from_records`: 6458327 ns
- `graph_load_total`: 1056443971 ns
- note: run_picker_discovery=6683 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `eval_protocol_call_review_scan_300`: frames=300, median=2021159 ns, p95=2057599 ns, p99=2318282 ns, max=97266437 ns, heap_slope=plateau
  - component `capture_overhead`: median=7565 ns, median_frame_share=0.37%
  - component `central_graph`: median=1437443 ns, median_frame_share=71.11%
  - component `run_navigation`: median=338271 ns, median_frame_share=16.73%
  - component `timeline`: median=118815 ns, median_frame_share=5.87%
  - component `top_strip`: median=103396 ns, median_frame_share=5.11%
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (91256576 wrapped bytes)

## Local Puffin Captures

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/puffin/benchmarks/20260525-eval-protocol-call-review-scan-probe-r3/standard.puffin`: 1043244 bytes, sha256 `f620a6868d494f795c62134002be2e589efb97388e8a8ff82ae8017b7ffd01f7`

## Local Heap Profiles

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/heap/benchmarks/20260525-eval-protocol-call-review-scan-probe-r3/eval_protocol_call_review_scan_300.heap.json`: 79138 bytes, sha256 `bab64580e672fd7961c5f5c53764e8eb89e67e1c6e554e4d71fc4abb5664dbe5`

See `report.json` for typed timings and compact allocation summaries.
