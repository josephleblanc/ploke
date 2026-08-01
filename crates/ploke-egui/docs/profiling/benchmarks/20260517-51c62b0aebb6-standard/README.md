# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `51c62b0aebb6`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `Cargo.lock`
- `crates/ploke-egui/Cargo.toml`
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/import/tests.rs`
- `crates/ploke-egui/src/lib.rs`
- `crates/ploke-egui/src/native.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/view/mod.rs`
- `crates/ploke-egui/src/ui/view/projection.rs`
- `crates/ploke-records/src/history/payload.rs`
- `crates/ploke-records/src/run_profile.rs`
- `crates/ploke-records/src/selection.rs`
- `crates/ploke-tree/src/graph/build/passive.rs`
- `crates/ploke-tree/src/graph/build/selection.rs`
- `crates/ploke-tree/src/graph/build/selection/tests.rs`
- `crates/ploke-tree/src/graph/types.rs`
- `crates/ploke-tree/src/graph/types/selection.rs`
- `crates/ploke-tree/src/graph/types/warning.rs`
- `crates/ploke-tree/src/playback/fine/tests.rs`
- `crates/ploke-tree/src/tests.rs`

unrelated dirty paths:
- `crates/ploke-eval/docs/prototype1-run-profile.md`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs`
- `crates/ploke-eval/src/successor_selection/mod.rs`
- `crates/ploke-eval/src/successor_selection/traversal.rs`
- `crates/ploke-llm/src/lib.rs`
- `crates/ploke-llm/src/router_only/mod.rs`
- `crates/ploke-llm/src/router_only/tests/builder_tests.rs`
- `crates/ploke-llm/src/types/enums.rs`
- `crates/ploke-llm/src/types/params.rs`
- `crates/ploke-protocol/Cargo.toml`
- `crates/ploke-protocol/src/llm.rs`
- `crates/ploke-protocol/src/tool_calls/segment.rs`
- `docs/active/agents/collaboration-incidents/diagnosis-without-evidence/README.md`
- `docs/active/agents/readme.md`
- `docs/workflow/evalnomicon/drafts/eval/hyperagents-gap-review-2026-05-17.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260517-51c62b0aebb6-standard/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260517-661f758fcce2-standard/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260517-run-readiness-heuristic-benchmark-note.md`
- `crates/ploke-eval/src/successor_selection/metrics.rs`
- `docs/active/agents/2026-05-17-selection-metrics-graph-handoff.md`
- `docs/active/agents/collaboration-incidents/diagnosis-without-evidence/2026-05-17-ploke-egui-native-benchmark-tunnel-vision.md`
- `docs/workflow/evalnomicon/drafts/eval/ha-review-plan.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 30681943271 ns
- `FsRunStore::load`: 1170322227 ns
- `FsRunStore::load_history_blocks`: 23333833 ns
- `FsRunStore::load_transition_journal`: 4560051 ns
- `FsRunStore::load_record_set`: 1198221551 ns
- `compressed_run_record_profile_probe`: 117514900 ns
- `Graph::from_records`: 5707410 ns
- `graph_load_total`: 1321446706 ns
- note: run_picker_discovery=30681943271 ns (kept in startup spans)
- note: warning: run_picker_discovery exceeded 250000000 ns
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=1774664 ns, p95=5529556 ns, p99=8530598 ns, max=61400590 ns, heap_slope=plateau
  - component `capture_overhead`: median=3256 ns, median_frame_share=0.18%
  - component `central_graph`: median=1031392 ns, median_frame_share=58.11%
  - component `diagnostics`: median=70552 ns, median_frame_share=3.97%, nested_under=run_navigation
  - component `run_navigation`: median=289562 ns, median_frame_share=16.31%
  - component `selection_inspector`: median=232205 ns, median_frame_share=13.08%
  - component `timeline`: median=50655 ns, median_frame_share=2.85%
  - component `top_strip`: median=78978 ns, median_frame_share=4.45%
- `warm_idle_300`: frames=300, median=1715263 ns, p95=4537128 ns, p99=5567427 ns, max=7070733 ns, heap_slope=plateau
  - component `capture_overhead`: median=3136 ns, median_frame_share=0.18%
  - component `central_graph`: median=1002808 ns, median_frame_share=58.46%
  - component `diagnostics`: median=69440 ns, median_frame_share=4.04%, nested_under=run_navigation
  - component `run_navigation`: median=280535 ns, median_frame_share=16.35%
  - component `selection_inspector`: median=227967 ns, median_frame_share=13.29%
  - component `timeline`: median=49553 ns, median_frame_share=2.88%
  - component `top_strip`: median=76172 ns, median_frame_share=4.44%
- `select_artifact_inspector_300`: frames=300, median=1879851 ns, p95=2309246 ns, p99=3058870 ns, max=33332182 ns, heap_slope=plateau
  - component `capture_overhead`: median=3156 ns, median_frame_share=0.16%
  - component `central_graph`: median=1001876 ns, median_frame_share=53.29%
  - component `diagnostics`: median=69270 ns, median_frame_share=3.68%, nested_under=run_navigation
  - component `run_navigation`: median=278712 ns, median_frame_share=14.82%
  - component `selection_inspector`: median=396112 ns, median_frame_share=21.07%
  - component `timeline`: median=50985 ns, median_frame_share=2.71%
  - component `top_strip`: median=75661 ns, median_frame_share=4.02%
- `inspector_run_records_expanded_300`: frames=300, median=2246578 ns, p95=2827857 ns, p99=3299140 ns, max=26183654 ns, heap_slope=plateau
  - component `capture_overhead`: median=3186 ns, median_frame_share=0.14%
  - component `central_graph`: median=1008950 ns, median_frame_share=44.91%
  - component `diagnostics`: median=68879 ns, median_frame_share=3.06%, nested_under=run_navigation
  - component `run_navigation`: median=278081 ns, median_frame_share=12.37%
  - component `selection_inspector`: median=742501 ns, median_frame_share=33.05%
  - component `timeline`: median=51226 ns, median_frame_share=2.28%
  - component `top_strip`: median=75501 ns, median_frame_share=3.36%
- `inspector_graph_edges_expanded_300`: frames=300, median=2463174 ns, p95=2881838 ns, p99=3683629 ns, max=6854239 ns, heap_slope=plateau
  - component `capture_overhead`: median=3186 ns, median_frame_share=0.12%
  - component `central_graph`: median=1003520 ns, median_frame_share=40.74%
  - component `diagnostics`: median=68248 ns, median_frame_share=2.77%, nested_under=run_navigation
  - component `run_navigation`: median=276037 ns, median_frame_share=11.20%
  - component `selection_inspector`: median=980196 ns, median_frame_share=39.79%
  - component `timeline`: median=51546 ns, median_frame_share=2.09%
  - component `top_strip`: median=75161 ns, median_frame_share=3.05%
- `inspector_artifact_edges_expanded_300`: frames=300, median=2740113 ns, p95=3449291 ns, p99=4246324 ns, max=4586101 ns, heap_slope=plateau
  - component `capture_overhead`: median=3226 ns, median_frame_share=0.11%
  - component `central_graph`: median=1009000 ns, median_frame_share=36.82%
  - component `diagnostics`: median=68439 ns, median_frame_share=2.49%, nested_under=run_navigation
  - component `run_navigation`: median=277179 ns, median_frame_share=10.11%
  - component `selection_inspector`: median=1225826 ns, median_frame_share=44.73%
  - component `timeline`: median=52428 ns, median_frame_share=1.91%
  - component `top_strip`: median=75552 ns, median_frame_share=2.75%
- `inspector_patch_debug_expanded_300`: frames=300, median=3029265 ns, p95=3787534 ns, p99=4539624 ns, max=90204558 ns, heap_slope=plateau
  - component `capture_overhead`: median=3216 ns, median_frame_share=0.10%
  - component `central_graph`: median=1007016 ns, median_frame_share=33.24%
  - component `diagnostics`: median=68589 ns, median_frame_share=2.26%, nested_under=run_navigation
  - component `run_navigation`: median=277410 ns, median_frame_share=9.15%
  - component `selection_inspector`: median=1498557 ns, median_frame_share=49.46%
  - component `timeline`: median=53791 ns, median_frame_share=1.77%
  - component `top_strip`: median=75020 ns, median_frame_share=2.47%
- `inspector_source_refs_expanded_300`: frames=300, median=3071694 ns, p95=3629138 ns, p99=3966269 ns, max=4583587 ns, heap_slope=plateau
  - component `capture_overhead`: median=3226 ns, median_frame_share=0.10%
  - component `central_graph`: median=1007877 ns, median_frame_share=32.81%
  - component `diagnostics`: median=68358 ns, median_frame_share=2.22%, nested_under=run_navigation
  - component `run_navigation`: median=275657 ns, median_frame_share=8.97%
  - component `selection_inspector`: median=1552358 ns, median_frame_share=50.53%
  - component `timeline`: median=52609 ns, median_frame_share=1.71%
  - component `top_strip`: median=75311 ns, median_frame_share=2.45%
- `inspector_artifact_ids_expanded_300`: frames=300, median=3149781 ns, p95=3703608 ns, p99=4633630 ns, max=5213847 ns, heap_slope=plateau
  - component `capture_overhead`: median=3247 ns, median_frame_share=0.10%
  - component `central_graph`: median=1008750 ns, median_frame_share=32.02%
  - component `diagnostics`: median=68438 ns, median_frame_share=2.17%, nested_under=run_navigation
  - component `run_navigation`: median=277379 ns, median_frame_share=8.80%
  - component `selection_inspector`: median=1633370 ns, median_frame_share=51.85%
  - component `timeline`: median=53199 ns, median_frame_share=1.68%
  - component `top_strip`: median=75341 ns, median_frame_share=2.39%
- `patch_debug_cold_300`: frames=300, median=3180648 ns, p95=3794327 ns, p99=4186493 ns, max=9190125 ns, heap_slope=plateau
  - component `capture_overhead`: median=3266 ns, median_frame_share=0.10%
  - component `central_graph`: median=1010593 ns, median_frame_share=31.77%
  - component `diagnostics`: median=68388 ns, median_frame_share=2.15%, nested_under=run_navigation
  - component `run_navigation`: median=276378 ns, median_frame_share=8.68%
  - component `selection_inspector`: median=1666281 ns, median_frame_share=52.38%
  - component `timeline`: median=53691 ns, median_frame_share=1.68%
  - component `top_strip`: median=75321 ns, median_frame_share=2.36%
- `patch_debug_warm_300`: frames=300, median=3312785 ns, p95=6806160 ns, p99=10958068 ns, max=12795621 ns, heap_slope=plateau
  - component `capture_overhead`: median=3326 ns, median_frame_share=0.10%
  - component `central_graph`: median=1022666 ns, median_frame_share=30.87%
  - component `diagnostics`: median=70222 ns, median_frame_share=2.11%, nested_under=run_navigation
  - component `run_navigation`: median=283461 ns, median_frame_share=8.55%
  - component `selection_inspector`: median=1761791 ns, median_frame_share=53.18%
  - component `timeline`: median=55985 ns, median_frame_share=1.68%
  - component `top_strip`: median=79348 ns, median_frame_share=2.39%
- `mode_lineage_300`: frames=300, median=3314991 ns, p95=4751972 ns, p99=7817755 ns, max=10713220 ns, heap_slope=plateau
  - component `capture_overhead`: median=4819 ns, median_frame_share=0.14%
  - component `central_graph`: median=278992 ns, median_frame_share=8.41%
  - component `diagnostics`: median=115186 ns, median_frame_share=3.47%, nested_under=run_navigation
  - component `run_navigation`: median=450534 ns, median_frame_share=13.59%
  - component `selection_inspector`: median=2325055 ns, median_frame_share=70.13%
  - component `timeline`: median=72827 ns, median_frame_share=2.19%
  - component `top_strip`: median=117861 ns, median_frame_share=3.55%
- `mode_artifact_tree_300`: frames=300, median=3186669 ns, p95=3992298 ns, p99=5158264 ns, max=6811381 ns, heap_slope=plateau
  - component `capture_overhead`: median=3287 ns, median_frame_share=0.10%
  - component `central_graph`: median=1015142 ns, median_frame_share=31.85%
  - component `diagnostics`: median=69880 ns, median_frame_share=2.19%, nested_under=run_navigation
  - component `run_navigation`: median=283141 ns, median_frame_share=8.88%
  - component `selection_inspector`: median=1662555 ns, median_frame_share=52.17%
  - component `timeline`: median=53560 ns, median_frame_share=1.68%
  - component `top_strip`: median=78277 ns, median_frame_share=2.45%
- `toggle_hide_unconsidered_children_300`: frames=300, median=1888197 ns, p95=2345614 ns, p99=2776211 ns, max=6910276 ns, heap_slope=plateau
  - component `capture_overhead`: median=3226 ns, median_frame_share=0.17%
  - component `central_graph`: median=1012337 ns, median_frame_share=53.61%
  - component `diagnostics`: median=69811 ns, median_frame_share=3.69%, nested_under=run_navigation
  - component `run_navigation`: median=284463 ns, median_frame_share=15.06%
  - component `selection_inspector`: median=365424 ns, median_frame_share=19.35%
  - component `timeline`: median=49603 ns, median_frame_share=2.62%
  - component `top_strip`: median=77365 ns, median_frame_share=4.09%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260517-51c62b0aebb6-standard/standard.puffin`: 12911996 bytes, sha256 `4158732456d69a322b9cc54e07fd67251c43d0e34a1049c00811602ce4c0f418`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/startup_frames_300.heap.json`: 4132 bytes, sha256 `3dc5ceae92577246ae7a46768aa17069d8c4fd6ddd53139fa7f11ccdf49d72ed`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/warm_idle_300.heap.json`: 4084 bytes, sha256 `cc0ccbbf344620cb26916aca4417310b9262d6d3738275d30a8766aa2b4dc9d5`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/select_artifact_inspector_300.heap.json`: 4109 bytes, sha256 `dddd0605d12b24bf2533669ec529108303e4b712e96dbff26907c6a2cc8c119a`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/inspector_run_records_expanded_300.heap.json`: 4512 bytes, sha256 `686499ae07965ad7f2e88b3fe123f7b9bf4256efb9db82b02b447d3588f7838b`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/inspector_graph_edges_expanded_300.heap.json`: 4932 bytes, sha256 `9048bcea2f0b1321d473c237c1f5e7640fa166bce46ee667908f9c20f437472c`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/inspector_artifact_edges_expanded_300.heap.json`: 5331 bytes, sha256 `20be71cf9806f4299610497a5fbcba87cb860571ddc9bf5ef29d943851dfc842`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/inspector_patch_debug_expanded_300.heap.json`: 5763 bytes, sha256 `be220724c9b84e432efd3681c4d3855ecb6866361773401bab3315ab7b10f358`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/inspector_source_refs_expanded_300.heap.json`: 6175 bytes, sha256 `2ad842d0fa438c45164b9fdeba066a5af890cda1f50f6ccc8d7de25ba540cb94`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/inspector_artifact_ids_expanded_300.heap.json`: 6577 bytes, sha256 `ab0ddd4fbe6f8d08a3fac2a3de06d0b8d48de6096925ea743619278100cff17a`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/patch_debug_cold_300.heap.json`: 6574 bytes, sha256 `3e907baa813507076efc074784c5043528e13522fc23a70814b28fd545d3f100`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/patch_debug_warm_300.heap.json`: 6570 bytes, sha256 `876fb9655818c0723ba7b12b7e49504fe7e73d9bb14e10b53f819dc510fd26dd`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/mode_lineage_300.heap.json`: 6587 bytes, sha256 `54a3303985f5886d53efd794a8465e533d6a90b348a283cbcdbbfb811eb6b21e`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/mode_artifact_tree_300.heap.json`: 6591 bytes, sha256 `05d8fb8ca31fb027599a59e0e65c34adc5b20c60c98386e36e1b47dee9312214`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260517-51c62b0aebb6-standard/toggle_hide_unconsidered_children_300.heap.json`: 4108 bytes, sha256 `a670e05bb71ae32cf93589a885f0a03c8fe1a1c6252f8975a3a6552a77e45fcc`

See `report.json` for typed timings and compact allocation summaries.
