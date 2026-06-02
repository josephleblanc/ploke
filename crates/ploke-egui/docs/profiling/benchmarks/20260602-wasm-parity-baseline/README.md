# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `87c18b830a19`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/import/mod.rs`
- `crates/ploke-egui/src/import/tests.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/benchmark-fixtures/`

unrelated dirty paths:
- `crates/ploke-egui/docs/profiling/benchmarks/README.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260602-wasm-parity-baseline-benchmark-note.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260602-wasm-parity-baseline/`
- `docs/active/agents/2026-06-02_egui-wasm-parity-phase0-audit.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 7093 ns
- `FsRunStore::load`: 909748308 ns
- `FsRunStore::load_history_blocks`: 23022520 ns
- `FsRunStore::load_transition_journal`: 4470825 ns
- `FsRunStore::load_record_set`: 937248347 ns
- `compressed_run_record_profile_probe`: 115331173 ns
- `Graph::from_records`: 6653594 ns
- `graph_load_total`: 1059236110 ns
- note: run_picker_discovery=7093 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=3131280 ns, p95=3507777 ns, p99=3947983 ns, max=116194875 ns, heap_slope=plateau
  - component `capture_overhead`: median=7744 ns, median_frame_share=0.24%
  - component `central_graph`: median=2495256 ns, median_frame_share=79.68%
  - component `diagnostics`: median=72136 ns, median_frame_share=2.30%, nested_under=run_navigation
  - component `run_navigation`: median=415270 ns, median_frame_share=13.26%
  - component `timeline`: median=118162 ns, median_frame_share=3.77%
  - component `top_strip`: median=76784 ns, median_frame_share=2.45%
- `warm_idle_300`: frames=300, median=3098869 ns, p95=3425944 ns, p99=3527864 ns, max=4147038 ns, heap_slope=plateau
  - component `capture_overhead`: median=7674 ns, median_frame_share=0.24%
  - component `central_graph`: median=2472663 ns, median_frame_share=79.79%
  - component `diagnostics`: median=70493 ns, median_frame_share=2.27%, nested_under=run_navigation
  - component `run_navigation`: median=409589 ns, median_frame_share=13.21%
  - component `timeline`: median=116218 ns, median_frame_share=3.75%
  - component `top_strip`: median=76013 ns, median_frame_share=2.45%
- `select_artifact_inspector_300`: frames=300, median=3292483 ns, p95=3647880 ns, p99=3845381 ns, max=19005608 ns, heap_slope=plateau
  - component `capture_overhead`: median=7755 ns, median_frame_share=0.23%
  - component `central_graph`: median=2666838 ns, median_frame_share=80.99%
  - component `diagnostics`: median=70312 ns, median_frame_share=2.13%, nested_under=run_navigation
  - component `run_navigation`: median=409659 ns, median_frame_share=12.44%
  - component `timeline`: median=116268 ns, median_frame_share=3.53%
  - component `top_strip`: median=76473 ns, median_frame_share=2.32%
- `inspector_run_records_expanded_300`: frames=300, median=3291541 ns, p95=3688967 ns, p99=4159360 ns, max=5419326 ns, heap_slope=plateau
  - component `capture_overhead`: median=7714 ns, median_frame_share=0.23%
  - component `central_graph`: median=2664854 ns, median_frame_share=80.96%
  - component `diagnostics`: median=70362 ns, median_frame_share=2.13%, nested_under=run_navigation
  - component `run_navigation`: median=409358 ns, median_frame_share=12.43%
  - component `timeline`: median=116889 ns, median_frame_share=3.55%
  - component `top_strip`: median=76083 ns, median_frame_share=2.31%
- `inspector_graph_edges_expanded_300`: frames=300, median=3331365 ns, p95=3742878 ns, p99=4654540 ns, max=5394339 ns, heap_slope=plateau
  - component `capture_overhead`: median=7785 ns, median_frame_share=0.23%
  - component `central_graph`: median=2680414 ns, median_frame_share=80.45%
  - component `diagnostics`: median=71885 ns, median_frame_share=2.15%, nested_under=run_navigation
  - component `run_navigation`: median=415811 ns, median_frame_share=12.48%
  - component `timeline`: median=119324 ns, median_frame_share=3.58%
  - component `top_strip`: median=77836 ns, median_frame_share=2.33%
- `inspector_artifact_edges_expanded_300`: frames=300, median=3287223 ns, p95=3580213 ns, p99=4312818 ns, max=5364693 ns, heap_slope=plateau
  - component `capture_overhead`: median=7724 ns, median_frame_share=0.23%
  - component `central_graph`: median=2662679 ns, median_frame_share=81.00%
  - component `diagnostics`: median=70082 ns, median_frame_share=2.13%, nested_under=run_navigation
  - component `run_navigation`: median=408768 ns, median_frame_share=12.43%
  - component `timeline`: median=115908 ns, median_frame_share=3.52%
  - component `top_strip`: median=75822 ns, median_frame_share=2.30%
- `inspector_patch_debug_expanded_300`: frames=300, median=3306690 ns, p95=3625187 ns, p99=3957170 ns, max=4203874 ns, heap_slope=plateau
  - component `capture_overhead`: median=7724 ns, median_frame_share=0.23%
  - component `central_graph`: median=2672018 ns, median_frame_share=80.80%
  - component `diagnostics`: median=70762 ns, median_frame_share=2.13%, nested_under=run_navigation
  - component `run_navigation`: median=410741 ns, median_frame_share=12.42%
  - component `timeline`: median=118372 ns, median_frame_share=3.57%
  - component `top_strip`: median=76443 ns, median_frame_share=2.31%
- `inspector_llm_calls_expanded_300`: frames=300, median=4246053 ns, p95=4298441 ns, p99=4650623 ns, max=12840371 ns, heap_slope=plateau
  - component `capture_overhead`: median=7694 ns, median_frame_share=0.18%
  - component `central_graph`: median=3622272 ns, median_frame_share=85.30%
  - component `diagnostics`: median=70061 ns, median_frame_share=1.65%, nested_under=run_navigation
  - component `run_navigation`: median=409179 ns, median_frame_share=9.63%
  - component `timeline`: median=116108 ns, median_frame_share=2.73%
  - component `top_strip`: median=75612 ns, median_frame_share=1.78%
- `inspector_source_refs_expanded_300`: frames=300, median=4256743 ns, p95=4695427 ns, p99=5368030 ns, max=7057312 ns, heap_slope=plateau
  - component `capture_overhead`: median=7715 ns, median_frame_share=0.18%
  - component `central_graph`: median=3630026 ns, median_frame_share=85.27%
  - component `diagnostics`: median=70162 ns, median_frame_share=1.64%, nested_under=run_navigation
  - component `run_navigation`: median=409990 ns, median_frame_share=9.63%
  - component `timeline`: median=116138 ns, median_frame_share=2.72%
  - component `top_strip`: median=76133 ns, median_frame_share=1.78%
- `inspector_artifact_ids_expanded_300`: frames=300, median=4345810 ns, p95=4786588 ns, p99=4914298 ns, max=5942389 ns, heap_slope=plateau
  - component `capture_overhead`: median=7755 ns, median_frame_share=0.17%
  - component `central_graph`: median=3717480 ns, median_frame_share=85.54%
  - component `diagnostics`: median=70181 ns, median_frame_share=1.61%, nested_under=run_navigation
  - component `run_navigation`: median=410410 ns, median_frame_share=9.44%
  - component `timeline`: median=116078 ns, median_frame_share=2.67%
  - component `top_strip`: median=76233 ns, median_frame_share=1.75%
- `patch_debug_cold_300`: frames=300, median=4332996 ns, p95=4768134 ns, p99=4837654 ns, max=5813616 ns, heap_slope=plateau
  - component `capture_overhead`: median=7694 ns, median_frame_share=0.17%
  - component `central_graph`: median=3709215 ns, median_frame_share=85.60%
  - component `diagnostics`: median=70021 ns, median_frame_share=1.61%, nested_under=run_navigation
  - component `run_navigation`: median=407796 ns, median_frame_share=9.41%
  - component `timeline`: median=115847 ns, median_frame_share=2.67%
  - component `top_strip`: median=75693 ns, median_frame_share=1.74%
- `patch_debug_warm_300`: frames=300, median=4339899 ns, p95=4811575 ns, p99=5156764 ns, max=5650190 ns, heap_slope=plateau
  - component `capture_overhead`: median=7714 ns, median_frame_share=0.17%
  - component `central_graph`: median=3717430 ns, median_frame_share=85.65%
  - component `diagnostics`: median=70111 ns, median_frame_share=1.61%, nested_under=run_navigation
  - component `run_navigation`: median=409349 ns, median_frame_share=9.43%
  - component `timeline`: median=115597 ns, median_frame_share=2.66%
  - component `top_strip`: median=75531 ns, median_frame_share=1.74%
- `mode_lineage_300`: frames=300, median=3410525 ns, p95=3848346 ns, p99=4556727 ns, max=4884041 ns, heap_slope=plateau
  - component `capture_overhead`: median=7714 ns, median_frame_share=0.22%
  - component `central_graph`: median=2785341 ns, median_frame_share=81.66%
  - component `diagnostics`: median=70282 ns, median_frame_share=2.06%, nested_under=run_navigation
  - component `run_navigation`: median=410080 ns, median_frame_share=12.02%
  - component `timeline`: median=115637 ns, median_frame_share=3.39%
  - component `top_strip`: median=75883 ns, median_frame_share=2.22%
- `mode_artifact_tree_300`: frames=300, median=4341132 ns, p95=4828396 ns, p99=5090078 ns, max=7953224 ns, heap_slope=plateau
  - component `capture_overhead`: median=7675 ns, median_frame_share=0.17%
  - component `central_graph`: median=3714395 ns, median_frame_share=85.56%
  - component `diagnostics`: median=70312 ns, median_frame_share=1.61%, nested_under=run_navigation
  - component `run_navigation`: median=410060 ns, median_frame_share=9.44%
  - component `timeline`: median=116038 ns, median_frame_share=2.67%
  - component `top_strip`: median=75562 ns, median_frame_share=1.74%
- `toggle_hide_unconsidered_children_300`: frames=300, median=3145506 ns, p95=3505262 ns, p99=4982566 ns, max=7414944 ns, heap_slope=plateau
  - component `capture_overhead`: median=7664 ns, median_frame_share=0.24%
  - component `central_graph`: median=2519121 ns, median_frame_share=80.08%
  - component `diagnostics`: median=70713 ns, median_frame_share=2.24%, nested_under=run_navigation
  - component `run_navigation`: median=411673 ns, median_frame_share=13.08%
  - component `timeline`: median=117200 ns, median_frame_share=3.72%
  - component `top_strip`: median=76183 ns, median_frame_share=2.42%
- `graph_snapshot_replace_cold`: frames=300, median=3331416 ns, p95=3717772 ns, p99=3837847 ns, max=170042539 ns, heap_slope=plateau
  - component `capture_overhead`: median=8006 ns, median_frame_share=0.24%
  - component `central_graph`: median=2660736 ns, median_frame_share=79.86%
  - component `diagnostics`: median=75752 ns, median_frame_share=2.27%, nested_under=run_navigation
  - component `run_navigation`: median=438293 ns, median_frame_share=13.15%
  - component `timeline`: median=125145 ns, median_frame_share=3.75%
  - component `top_strip`: median=82164 ns, median_frame_share=2.46%
- `inspector_tool_decode_expanded_300`: frames=300, median=3316718 ns, p95=3659301 ns, p99=3773455 ns, max=4902296 ns, heap_slope=plateau
  - component `capture_overhead`: median=7784 ns, median_frame_share=0.23%
  - component `central_graph`: median=2680824 ns, median_frame_share=80.82%
  - component `diagnostics`: median=71364 ns, median_frame_share=2.15%, nested_under=run_navigation
  - component `run_navigation`: median=414037 ns, median_frame_share=12.48%
  - component `timeline`: median=118543 ns, median_frame_share=3.57%
  - component `top_strip`: median=77266 ns, median_frame_share=2.32%
- `graph_catalog_idle_300`: frames=300, median=3307991 ns, p95=3748890 ns, p99=3796690 ns, max=5225052 ns, heap_slope=plateau
  - component `capture_overhead`: median=7725 ns, median_frame_share=0.23%
  - component `central_graph`: median=2661938 ns, median_frame_share=80.46%
  - component `diagnostics`: median=70653 ns, median_frame_share=2.13%, nested_under=run_navigation
  - component `graph_catalog`: median=18936 ns, median_frame_share=0.57%, nested_under=run_navigation
  - component `run_navigation`: median=428134 ns, median_frame_share=12.94%
  - component `timeline`: median=117300 ns, median_frame_share=3.54%
  - component `top_strip`: median=76023 ns, median_frame_share=2.29%

## Local Puffin Captures

- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260602-wasm-parity-baseline/standard.puffin`: 17812685 bytes, sha256 `7dde257f1b287f608485a3cf0790657cbace8380160c74a38fa86414cc4aa5de`

## Local Heap Profiles

- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/startup_frames_300.heap.json`: 11589 bytes, sha256 `3b923286b3dca5a115feaee53d48042ccccb6f45fcb3f0970c0dd3d94f4f2634`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/warm_idle_300.heap.json`: 9477 bytes, sha256 `383145ac8e05e040d5750583da323a6d6b1deaa35cca6275e034cbf33c430a92`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/select_artifact_inspector_300.heap.json`: 9929 bytes, sha256 `48176ea27cdedc851d29153321b9c587762b2494a8341e3aa125c14482775029`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/inspector_run_records_expanded_300.heap.json`: 9501 bytes, sha256 `6d95fabc822d0b2c5c6d7ae4f3f1da5e45e56ad98fcfab6ab36cd4205a6bb3c3`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/inspector_graph_edges_expanded_300.heap.json`: 9501 bytes, sha256 `de5256b632abf5a27cdd74f18e3ad7b88f49ddb8849ce4981686be0af5dc2dd9`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/inspector_artifact_edges_expanded_300.heap.json`: 9887 bytes, sha256 `48790bcfd882087c2ce222cffee117e91aaf05853b1501617d2432973e1399f0`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/inspector_patch_debug_expanded_300.heap.json`: 9501 bytes, sha256 `214ecd2bbd8c3f614fd7e7c64a4500f906fbe9fc41c593c94b7f6a0db40cb5aa`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/inspector_llm_calls_expanded_300.heap.json`: 11171 bytes, sha256 `b6588cbbb61f7a995e086fe50a549f33a1adbfc6e4ceaa233f982759e0d849e6`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/inspector_source_refs_expanded_300.heap.json`: 10340 bytes, sha256 `c43ac02864ae28459e7a35ceee9dacf322d9d8e79d8b2d833aaf19b238af9026`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/inspector_artifact_ids_expanded_300.heap.json`: 11165 bytes, sha256 `57bdcfde0732ad0ec791dbbe9ee829e95b9f5e8d5e16cdad6f16d0d8c9e0bfcb`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/patch_debug_cold_300.heap.json`: 10749 bytes, sha256 `aed5e9c54613b8a99080fbff58d728748923065c07f3ef6170008dd5aac37e82`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/patch_debug_warm_300.heap.json`: 11135 bytes, sha256 `61002b9932d648ec77fe5426feff4586490f1a4477bfd96627cfe7fed70b4c68`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/mode_lineage_300.heap.json`: 11119 bytes, sha256 `fad6493f4f9e2950c2f00d5ea6f666ba549ac286f03590b977ca19a852ccd23d`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/mode_artifact_tree_300.heap.json`: 11579 bytes, sha256 `2f6c0063d43633ea9d064409d3e639b396017d315cd4fab45657ca9376702fa3`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/toggle_hide_unconsidered_children_300.heap.json`: 10736 bytes, sha256 `478fdcfa9fb590006e43b7b17e387db16dc5abcd04fa6ae87c4cc41acab9c5d3`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/graph_snapshot_replace_cold.heap.json`: 11201 bytes, sha256 `c9e6297f6341d30ce9ab62fa496e09aec4017d91505aa749129809bd964d6bc9`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/inspector_tool_decode_expanded_300.heap.json`: 9926 bytes, sha256 `5019b4afb433876b0bc34593c498e9a67cf44d1f59f1f241bc773b495da7e6ad`
- `/home/brasides/code/agent-dir/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260602-wasm-parity-baseline/graph_catalog_idle_300.heap.json`: 9871 bytes, sha256 `704a70cde5097c0ed579cc4a80ef9923647ebeefd931170f9f73209fe41f761f`

See `report.json` for typed timings and compact allocation summaries.
