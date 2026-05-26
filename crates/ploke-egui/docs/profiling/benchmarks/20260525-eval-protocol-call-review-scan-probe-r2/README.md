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
- `crates/ploke-egui/docs/profiling/benchmarks/20260525-eval-protocol-call-review-scan-probe/`
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

- `run_picker_discovery`: 13957 ns
- `FsRunStore::load`: 903474457 ns
- `FsRunStore::load_history_blocks`: 24037645 ns
- `FsRunStore::load_transition_journal`: 4696462 ns
- `FsRunStore::load_record_set`: 932212101 ns
- `compressed_run_record_profile_probe`: 118137732 ns
- `Graph::from_records`: 6776273 ns
- `graph_load_total`: 1057128810 ns
- note: run_picker_discovery=13957 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `eval_protocol_call_review_scan_300`: frames=300, median=3710999 ns, p95=3948811 ns, p99=5665642 ns, max=1933141574 ns, heap_slope=plateau
  - component `capture_overhead`: median=7705 ns, median_frame_share=0.20%
  - component `central_graph`: median=3120027 ns, median_frame_share=84.07%
  - component `run_navigation`: median=340918 ns, median_frame_share=9.18%
  - component `timeline`: median=119607 ns, median_frame_share=3.22%
  - component `top_strip`: median=104088 ns, median_frame_share=2.80%
  - top heap callsite by allocated bytes: `eframe_run_native -> tessellate_text` (83892480 wrapped bytes)

## Local Puffin Captures

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/puffin/benchmarks/20260525-eval-protocol-call-review-scan-probe-r2/standard.puffin`: 1043720 bytes, sha256 `a000b710f77c2a975f4d97366b4cb7bb6ff0f09aa7cf785438bdacf19b23efd1`

## Local Heap Profiles

- `/home/brasides/code/agent-dir/ploke-ui-wt-01/crates/ploke-egui/data/profiling/heap/benchmarks/20260525-eval-protocol-call-review-scan-probe-r2/eval_protocol_call_review_scan_300.heap.json`: 86755 bytes, sha256 `65c0a71e0804281b0b4a5f8648d0e3e693d8be2e0d0f1f75c4ee4d75368e3384`

See `report.json` for typed timings and compact allocation summaries.
