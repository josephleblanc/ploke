# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `c7a9bab87e76`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/diff/mod.rs`
- `crates/ploke-egui/src/ui/id_display/mod.rs`
- `crates/ploke-egui/src/ui/inspector.rs`

unrelated dirty paths:
- `AGENTS.md`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `docs/active/agents/collaboration-incidents/README.md`
- `docs/active/agents/collaboration-incidents/diagnosis-without-evidence/README.md`
- `docs/active/agents/readme.md`
- `docs/active/archaeology/INDEX.md`
- `docs/active/archaeology/ploke-tree-graph/README.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260517-c7a9bab87e76-standard/`
- `docs/active/agents/collaboration-incidents/diagnosis-without-evidence/2026-05-17-ploke-egui-allocation-priorities-overclaimed.md`
- `docs/active/agents/collaboration-incidents/model-communication-failures/`
- `docs/active/archaeology/ploke-tree-graph/runtime-role.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 33481328983 ns
- `FsRunStore::load`: 902879101 ns
- `FsRunStore::load_history_blocks`: 23317128 ns
- `FsRunStore::load_transition_journal`: 4740467 ns
- `FsRunStore::load_record_set`: 930940904 ns
- `compressed_run_record_profile_probe`: 115389002 ns
- `Graph::from_records`: 5233417 ns
- `graph_load_total`: 1051565527 ns
- note: run_picker_discovery=33481328983 ns (kept in startup spans)
- note: warning: run_picker_discovery exceeded 250000000 ns
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=1720397 ns, p95=1955421 ns, p99=2362539 ns, max=61339820 ns, heap_slope=plateau
  - component `capture_overhead`: median=3066 ns, median_frame_share=0.17%
  - component `central_graph`: median=1011319 ns, median_frame_share=58.78%
  - component `diagnostics`: median=68960 ns, median_frame_share=4.00%, nested_under=run_navigation
  - component `run_navigation`: median=277323 ns, median_frame_share=16.11%
  - component `selection_inspector`: median=227619 ns, median_frame_share=13.23%
  - component `timeline`: median=49354 ns, median_frame_share=2.86%
  - component `top_strip`: median=75563 ns, median_frame_share=4.39%
- `warm_idle_300`: frames=300, median=1719505 ns, p95=1825695 ns, p99=1972262 ns, max=2347571 ns, heap_slope=plateau
  - component `capture_overhead`: median=3035 ns, median_frame_share=0.17%
  - component `central_graph`: median=1009926 ns, median_frame_share=58.73%
  - component `diagnostics`: median=69732 ns, median_frame_share=4.05%, nested_under=run_navigation
  - component `run_navigation`: median=280219 ns, median_frame_share=16.29%
  - component `selection_inspector`: median=228451 ns, median_frame_share=13.28%
  - component `timeline`: median=49384 ns, median_frame_share=2.87%
  - component `top_strip`: median=75403 ns, median_frame_share=4.38%
- `select_artifact_inspector_300`: frames=300, median=1870260 ns, p95=1970939 ns, p99=2773193 ns, max=32133000 ns, heap_slope=plateau
  - component `capture_overhead`: median=3115 ns, median_frame_share=0.16%
  - component `central_graph`: median=1015586 ns, median_frame_share=54.30%
  - component `diagnostics`: median=69531 ns, median_frame_share=3.71%, nested_under=run_navigation
  - component `run_navigation`: median=279447 ns, median_frame_share=14.94%
  - component `selection_inspector`: median=372213 ns, median_frame_share=19.90%
  - component `timeline`: median=51718 ns, median_frame_share=2.76%
  - component `top_strip`: median=75993 ns, median_frame_share=4.06%
- `inspector_run_records_expanded_300`: frames=300, median=2130530 ns, p95=2207486 ns, p99=2632768 ns, max=26617540 ns, heap_slope=plateau
  - component `capture_overhead`: median=3066 ns, median_frame_share=0.14%
  - component `central_graph`: median=1012220 ns, median_frame_share=47.51%
  - component `diagnostics`: median=69271 ns, median_frame_share=3.25%, nested_under=run_navigation
  - component `run_navigation`: median=278115 ns, median_frame_share=13.05%
  - component `selection_inspector`: median=638285 ns, median_frame_share=29.95%
  - component `timeline`: median=51407 ns, median_frame_share=2.41%
  - component `top_strip`: median=75172 ns, median_frame_share=3.52%
- `inspector_graph_edges_expanded_300`: frames=300, median=2364432 ns, p95=2767782 ns, p99=3172717 ns, max=6748895 ns, heap_slope=plateau
  - component `capture_overhead`: median=3085 ns, median_frame_share=0.13%
  - component `central_graph`: median=1013772 ns, median_frame_share=42.87%
  - component `diagnostics`: median=69381 ns, median_frame_share=2.93%, nested_under=run_navigation
  - component `run_navigation`: median=278666 ns, median_frame_share=11.78%
  - component `selection_inspector`: median=868930 ns, median_frame_share=36.75%
  - component `timeline`: median=52539 ns, median_frame_share=2.22%
  - component `top_strip`: median=75062 ns, median_frame_share=3.17%
- `inspector_artifact_edges_expanded_300`: frames=300, median=2592461 ns, p95=2728719 ns, p99=2938665 ns, max=3187054 ns, heap_slope=plateau
  - component `capture_overhead`: median=3086 ns, median_frame_share=0.11%
  - component `central_graph`: median=1013833 ns, median_frame_share=39.10%
  - component `diagnostics`: median=69632 ns, median_frame_share=2.68%, nested_under=run_navigation
  - component `run_navigation`: median=280900 ns, median_frame_share=10.83%
  - component `selection_inspector`: median=1093463 ns, median_frame_share=42.17%
  - component `timeline`: median=52489 ns, median_frame_share=2.02%
  - component `top_strip`: median=75743 ns, median_frame_share=2.92%
- `inspector_patch_debug_expanded_300`: frames=300, median=2729770 ns, p95=3060986 ns, p99=3480797 ns, max=87047837 ns, heap_slope=plateau
  - component `capture_overhead`: median=3136 ns, median_frame_share=0.11%
  - component `central_graph`: median=1014715 ns, median_frame_share=37.17%
  - component `diagnostics`: median=69491 ns, median_frame_share=2.54%, nested_under=run_navigation
  - component `run_navigation`: median=279127 ns, median_frame_share=10.22%
  - component `selection_inspector`: median=1229650 ns, median_frame_share=45.04%
  - component `timeline`: median=52910 ns, median_frame_share=1.93%
  - component `top_strip`: median=75433 ns, median_frame_share=2.76%
- `inspector_source_refs_expanded_300`: frames=300, median=2765608 ns, p95=2839147 ns, p99=3269348 ns, max=4053277 ns, heap_slope=plateau
  - component `capture_overhead`: median=3116 ns, median_frame_share=0.11%
  - component `central_graph`: median=1013442 ns, median_frame_share=36.64%
  - component `diagnostics`: median=69531 ns, median_frame_share=2.51%, nested_under=run_navigation
  - component `run_navigation`: median=279527 ns, median_frame_share=10.10%
  - component `selection_inspector`: median=1270096 ns, median_frame_share=45.92%
  - component `timeline`: median=52649 ns, median_frame_share=1.90%
  - component `top_strip`: median=75402 ns, median_frame_share=2.72%
- `inspector_artifact_ids_expanded_300`: frames=300, median=2851530 ns, p95=2996443 ns, p99=3332217 ns, max=4865670 ns, heap_slope=plateau
  - component `capture_overhead`: median=3116 ns, median_frame_share=0.10%
  - component `central_graph`: median=1015386 ns, median_frame_share=35.60%
  - component `diagnostics`: median=69331 ns, median_frame_share=2.43%, nested_under=run_navigation
  - component `run_navigation`: median=277884 ns, median_frame_share=9.74%
  - component `selection_inspector`: median=1352592 ns, median_frame_share=47.43%
  - component `timeline`: median=54774 ns, median_frame_share=1.92%
  - component `top_strip`: median=75272 ns, median_frame_share=2.63%
- `patch_debug_cold_300`: frames=300, median=2849646 ns, p95=2952681 ns, p99=3340192 ns, max=9748121 ns, heap_slope=plateau
  - component `capture_overhead`: median=3106 ns, median_frame_share=0.10%
  - component `central_graph`: median=1013522 ns, median_frame_share=35.56%
  - component `diagnostics`: median=69421 ns, median_frame_share=2.43%, nested_under=run_navigation
  - component `run_navigation`: median=278105 ns, median_frame_share=9.75%
  - component `selection_inspector`: median=1352121 ns, median_frame_share=47.44%
  - component `timeline`: median=54653 ns, median_frame_share=1.91%
  - component `top_strip`: median=75192 ns, median_frame_share=2.63%
- `patch_debug_warm_300`: frames=300, median=2860315 ns, p95=3171754 ns, p99=3439118 ns, max=3977053 ns, heap_slope=plateau
  - component `capture_overhead`: median=3156 ns, median_frame_share=0.11%
  - component `central_graph`: median=1016838 ns, median_frame_share=35.54%
  - component `diagnostics`: median=69641 ns, median_frame_share=2.43%, nested_under=run_navigation
  - component `run_navigation`: median=279347 ns, median_frame_share=9.76%
  - component `selection_inspector`: median=1355247 ns, median_frame_share=47.38%
  - component `timeline`: median=54893 ns, median_frame_share=1.91%
  - component `top_strip`: median=75713 ns, median_frame_share=2.64%
- `mode_lineage_300`: frames=300, median=2021694 ns, p95=2158532 ns, p99=2525014 ns, max=3877084 ns, heap_slope=plateau
  - component `capture_overhead`: median=3106 ns, median_frame_share=0.15%
  - component `central_graph`: median=185120 ns, median_frame_share=9.15%
  - component `diagnostics`: median=69381 ns, median_frame_share=3.43%, nested_under=run_navigation
  - component `run_navigation`: median=278275 ns, median_frame_share=13.76%
  - component `selection_inspector`: median=1353073 ns, median_frame_share=66.92%
  - component `timeline`: median=54674 ns, median_frame_share=2.70%
  - component `top_strip`: median=75342 ns, median_frame_share=3.72%
- `mode_artifact_tree_300`: frames=300, median=2856498 ns, p95=3165361 ns, p99=3428477 ns, max=6105267 ns, heap_slope=plateau
  - component `capture_overhead`: median=3156 ns, median_frame_share=0.11%
  - component `central_graph`: median=1016998 ns, median_frame_share=35.60%
  - component `diagnostics`: median=69491 ns, median_frame_share=2.43%, nested_under=run_navigation
  - component `run_navigation`: median=279487 ns, median_frame_share=9.78%
  - component `selection_inspector`: median=1355347 ns, median_frame_share=47.44%
  - component `timeline`: median=54863 ns, median_frame_share=1.92%
  - component `top_strip`: median=75662 ns, median_frame_share=2.64%
- `toggle_hide_unconsidered_children_300`: frames=300, median=1851533 ns, p95=1996807 ns, p99=2480479 ns, max=6120415 ns, heap_slope=plateau
  - component `capture_overhead`: median=3026 ns, median_frame_share=0.16%
  - component `central_graph`: median=1007991 ns, median_frame_share=54.44%
  - component `diagnostics`: median=69471 ns, median_frame_share=3.75%, nested_under=run_navigation
  - component `run_navigation`: median=279457 ns, median_frame_share=15.09%
  - component `selection_inspector`: median=364217 ns, median_frame_share=19.67%
  - component `timeline`: median=48973 ns, median_frame_share=2.64%
  - component `top_strip`: median=75232 ns, median_frame_share=4.06%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260517-c7a9bab87e76-standard/standard.puffin`: 13289629 bytes, sha256 `e8586a24cdd4720b8af39da522ba3df25209d363ddc71aec98c9f293d3f01e6e`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/startup_frames_300.heap.json`: 4132 bytes, sha256 `103ad046a8c794f936f115a5f5941cc4eab90b5f9b92ac69e74217a8857a3972`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/warm_idle_300.heap.json`: 4084 bytes, sha256 `88cf3ff7aab21a95ab6a31762d8d4a27bca487719cca5188388ed364a438a0e5`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/select_artifact_inspector_300.heap.json`: 4113 bytes, sha256 `2aae134a1e9c1fe2738309db6ca908c4a492943b09b591b2fdc68f6544cb8af7`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/inspector_run_records_expanded_300.heap.json`: 4511 bytes, sha256 `fd9eac9121f1d137290a10da78354b0e4499104efad6449d7aba44174efaa1a1`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/inspector_graph_edges_expanded_300.heap.json`: 4927 bytes, sha256 `c0a38b2d8bb55218342eb698bd96adfeb55619fa87bd3aa020639da7da90be18`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/inspector_artifact_edges_expanded_300.heap.json`: 5331 bytes, sha256 `0de62832362906810c385f4b0f488a2369b090619ace103b2c32d41a2d4b8061`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/inspector_patch_debug_expanded_300.heap.json`: 5751 bytes, sha256 `dde98f59a0362c5d21e453f64220f8f6300af24134fbf78ed1d7c00c9327c0ab`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/inspector_source_refs_expanded_300.heap.json`: 6155 bytes, sha256 `e96e22af75b3682310b58eb2df219997812fe4a1446b4b09212c05f96f76da36`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/inspector_artifact_ids_expanded_300.heap.json`: 6563 bytes, sha256 `e87e771d2ed33e3bdadc61441be0db5a4e534fac9e49a540b295c7893249356e`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/patch_debug_cold_300.heap.json`: 6566 bytes, sha256 `7d47921b459b442e44d1dc234f0e0175723dc8cfd79677d83d57a666b67798e0`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/patch_debug_warm_300.heap.json`: 6557 bytes, sha256 `ebcddf826b6ed987ea3e52ae74400b75c03e833bb1821f5e0698d9dbec455535`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/mode_lineage_300.heap.json`: 6573 bytes, sha256 `72b26f277b8164a3e416a3f54ca1c444eaf015881366c1fc463c6807d24b7bc8`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/mode_artifact_tree_300.heap.json`: 6578 bytes, sha256 `8033bb26c7c549421e511c621850ba0a5d3aa0ecf3ca9c17cd133078d168920a`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-c7a9bab87e76-standard/toggle_hide_unconsidered_children_300.heap.json`: 4107 bytes, sha256 `391b19a2212fab657ad795e5543e5782a9dee32468ba896b74cca01134d440b9`

See `report.json` for typed timings and compact allocation summaries.
