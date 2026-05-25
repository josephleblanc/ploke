# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `c4a6acb3ab66`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/native.rs`
- `crates/ploke-egui/src/run_picker/mod.rs`
- `crates/ploke-tree/src/store/fs.rs`
- `crates/ploke-tree/src/store/record_set.rs`

unrelated dirty paths:
- `docs/active/agents/collaboration-incidents/authority-boundary-violations/README.md`
- `docs/active/agents/collaboration-incidents/model-communication-failures/README.md`
- `docs/active/agents/collaboration-incidents/verification-surface-drift/README.md`
- `docs/active/archaeology/INDEX.md`
- `docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-c4a6acb3ab66-standard/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-selection-metric-witness-benchmark-note.md`
- `docs/active/agents/collaboration-incidents/authority-boundary-violations/2026-05-17-headless-harness-probe-used-primary-gitdir.md`
- `docs/active/agents/collaboration-incidents/model-communication-failures/2026-05-18-filepath-heavy-triage-summary.md`
- `docs/active/agents/collaboration-incidents/verification-surface-drift/2026-05-18-selection-inspector-allocation-regression.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 7684 ns
- `FsRunStore::load`: 915938395 ns
- `FsRunStore::load_history_blocks`: 23017070 ns
- `FsRunStore::load_transition_journal`: 4418779 ns
- `FsRunStore::load_record_set`: 943378222 ns
- `compressed_run_record_profile_probe`: 115000431 ns
- `Graph::from_records`: 6568787 ns
- `graph_load_total`: 1064950637 ns
- note: run_picker_discovery=7684 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=1711204 ns, p95=1925276 ns, p99=2617596 ns, max=60910351 ns, heap_slope=plateau
  - component `capture_overhead`: median=3066 ns, median_frame_share=0.17%
  - component `central_graph`: median=1007883 ns, median_frame_share=58.89%
  - component `diagnostics`: median=69260 ns, median_frame_share=4.04%, nested_under=run_navigation
  - component `run_navigation`: median=278142 ns, median_frame_share=16.25%
  - component `selection_inspector`: median=227647 ns, median_frame_share=13.30%
  - component `timeline`: median=49623 ns, median_frame_share=2.89%
  - component `top_strip`: median=75101 ns, median_frame_share=4.38%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-run-picker-startup-fast/standard.puffin`: 959362 bytes, sha256 `4bece59893aa1e2f1236fd65d812aae610b1ba3052355d6401da203513c3b4c7`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast/startup_frames_300.heap.json`: 4132 bytes, sha256 `ef4b47fa3960fe4e40b1aac900a8938605a5e30741bc83a3a9336d666766c05a`

See `report.json` for typed timings and compact allocation summaries.
