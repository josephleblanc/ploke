# ploke-egui Native Benchmark

Verification surface: native interactive window, full `standard` benchmark suite,
with explicit `--run-root` for the five-generation Prototype 1 run.

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
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-run-picker-startup-fast/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-selection-metric-witness-benchmark-note.md`
- `docs/active/agents/collaboration-incidents/authority-boundary-violations/2026-05-17-headless-harness-probe-used-primary-gitdir.md`
- `docs/active/agents/collaboration-incidents/model-communication-failures/2026-05-18-filepath-heavy-triage-summary.md`
- `docs/active/agents/collaboration-incidents/verification-surface-drift/2026-05-18-selection-inspector-allocation-regression.md`

## Triage Report

Change summary:

- Added a lightweight typed `RunRootSummary` loader in `ploke-tree` for run-picker labels.
- Changed run-picker discovery to use `FsRunStore::load_run_root_summary()` instead of importing a full `Graph` for every campaign.
- Deferred default campaign discovery when an explicit `--run-root` is provided, while still inserting the selected run into the picker.
- Added regression tests proving picker discovery does not require passive evidence/full graph import.

Command:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --benchmark-suite standard \
  --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-run-picker-startup-fast-r2
```

Baseline comparison:

- Startup comparison uses `../20260518-c4a6acb3ab66-standard/report.json` on the same run root.
- Full scenario comparison is not treated as a strict regression comparison because the previous nearest report had different dirty paths and did not cover the same full scenario set in this pass.

Startup verdict:

- The 30s startup blocker was run-picker discovery, not compressed run-record decompression for the selected run.
- `run_picker_discovery` improved from `28,583,398,987 ns` to `11,211 ns`.
- `graph_load_total` remains approximately 1s: `1,067,155,875 ns`.
- `FsRunStore::load_record_set` is the remaining startup cost at `944,446,218 ns`.
- `compressed_run_record_profile_probe` is `116,425,656 ns`; this is no longer the dominant startup problem.

Measured improvements:

- Explicit-run-root startup no longer scans and imports every campaign into a full graph.
- The picker still has a selected entry after the explicit run is loaded, and its label is updated from the loaded graph.
- The startup target that motivated the triage moved from roughly 30s to roughly 1.07s for graph load plus rendered startup frames.

Measured regressions:

- No startup regression was observed in the measured native-window path.
- No heap-slope regression was observed; every scenario reported `plateau`.

Remaining startup work:

- Reaching less than 500ms would require reducing `FsRunStore::load_record_set`.
- The likely next target is splitting full passive evidence import from first-paint graph construction, not changing gzip first.
- No-explicit-run-root native startup was not benchmarked in this report; this report verifies the explicit `--run-root` path.

Allocation summary:

- `startup_frames_300`: `plateau`; median `1143` allocs/frame, `824969` object bytes/frame, `836008` wrapped bytes/frame; median live `2140536` object bytes and `2171680` wrapped bytes/frame; end live `2703471` object bytes and `2750432` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `warm_idle_300`: `plateau`; median `1143` allocs/frame, `824965` object bytes/frame, `836008` wrapped bytes/frame; median live `579537` object bytes and `596424` wrapped bytes/frame; end live `1142718` object bytes and `1175456` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `select_artifact_inspector_300`: `plateau`; median `1216` allocs/frame, `905987` object bytes/frame, `917640` wrapped bytes/frame; median live `779822` object bytes and `801616` wrapped bytes/frame; end live `1354019` object bytes and `1391616` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `inspector_run_records_expanded_300`: `plateau`; median `1446` allocs/frame, `1085188` object bytes/frame, `1098696` wrapped bytes/frame; median live `852279` object bytes and `871744` wrapped bytes/frame; end live `1416652` object bytes and `1451904` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `inspector_graph_edges_expanded_300`: `plateau`; median `1632` allocs/frame, `1101915` object bytes/frame, `1116912` wrapped bytes/frame; median live `769076` object bytes and `787128` wrapped bytes/frame; end live `1333324` object bytes and `1367176` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `inspector_artifact_edges_expanded_300`: `plateau`; median `1816` allocs/frame, `1117730` object bytes/frame, `1134200` wrapped bytes/frame; median live `607628` object bytes and `624696` wrapped bytes/frame; end live `1171788` object bytes and `1204648` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `inspector_patch_debug_expanded_300`: `plateau`; median `1876` allocs/frame, `1124357` object bytes/frame, `1141320` wrapped bytes/frame; median live `1677921` object bytes and `1734168` wrapped bytes/frame; end live `2265612` object bytes and `2337688` wrapped bytes; top allocated/count group: `root`; top retained group: `inspector_patch_debug`; callsite attribution: not captured.
- `inspector_source_refs_expanded_300`: `plateau`; median `1908` allocs/frame, `1128012` object bytes/frame, `1145232` wrapped bytes/frame; median live `598129` object bytes and `615384` wrapped bytes/frame; end live `1170540` object bytes and `1203640` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `inspector_artifact_ids_expanded_300`: `plateau`; median `1949` allocs/frame, `1132068` object bytes/frame, `1149616` wrapped bytes/frame; median live `598572` object bytes and `615848` wrapped bytes/frame; end live `1170606` object bytes and `1203704` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `patch_debug_cold_300`: `plateau`; median `1949` allocs/frame, `1132068` object bytes/frame, `1149616` wrapped bytes/frame; median live `1001856` object bytes and `1020272` wrapped bytes/frame; end live `1573933` object bytes and `1608176` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `patch_debug_warm_300`: `plateau`; median `1949` allocs/frame, `1132072` object bytes/frame, `1149624` wrapped bytes/frame; median live `591220` object bytes and `608344` wrapped bytes/frame; end live `1162991` object bytes and `1195920` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `mode_lineage_300`: `plateau`; median `1784` allocs/frame, `770506` object bytes/frame, `786664` wrapped bytes/frame; median live `645257` object bytes and `663152` wrapped bytes/frame; end live `1213991` object bytes and `1247688` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `mode_artifact_tree_300`: `plateau`; median `1949` allocs/frame, `1132065` object bytes/frame, `1149616` wrapped bytes/frame; median live `676197` object bytes and `695104` wrapped bytes/frame; end live `1247442` object bytes and `1282200` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.
- `toggle_hide_unconsidered_children_300`: `plateau`; median `1294` allocs/frame, `891531` object bytes/frame, `903824` wrapped bytes/frame; median live `686964` object bytes and `706952` wrapped bytes/frame; end live `1252713` object bytes and `1288544` wrapped bytes; top allocated/count/retained groups: `root`/`root`/`root`; callsite attribution: not captured.

Remaining allocation debt:

- Every measured scenario remains above both allocation tripwires: more than 100 allocations/frame and more than 64 KiB allocated object bytes/frame.
- Standard mode captured group-level totals but no callsite backtraces, so `root` is an attribution limit, not a source-code diagnosis.
- A follow-up should run or add focused callsite attribution for startup/warm idle and the expanded inspector scenarios before ranking specific UI allocation sources.

Unmeasured risk:

- The explicit-run-root native path is verified. The default no-argument run-picker path should be much cheaper because it now loads summaries instead of full graphs, but it was not timed as a native startup scenario in this run.
- The report did not change full graph import semantics; inspector/detail views still rely on full `Graph` construction after a run is selected.

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 11211 ns
- `FsRunStore::load`: 916331714 ns
- `FsRunStore::load_history_blocks`: 23564698 ns
- `FsRunStore::load_transition_journal`: 4545667 ns
- `FsRunStore::load_record_set`: 944446218 ns
- `compressed_run_record_profile_probe`: 116425656 ns
- `Graph::from_records`: 6281698 ns
- `graph_load_total`: 1067155875 ns
- note: run_picker_discovery=11211 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `startup_frames_300`: frames=300, median=1911139 ns, p95=1938952 ns, p99=3480587 ns, max=61062655 ns, heap_slope=plateau
  - component `capture_overhead`: median=3106 ns, median_frame_share=0.16%
  - component `central_graph`: median=1061954 ns, median_frame_share=55.56%
  - component `diagnostics`: median=75903 ns, median_frame_share=3.97%, nested_under=run_navigation
  - component `run_navigation`: median=322866 ns, median_frame_share=16.89%
  - component `selection_inspector`: median=246624 ns, median_frame_share=12.90%
  - component `timeline`: median=52218 ns, median_frame_share=2.73%
  - component `top_strip`: median=118503 ns, median_frame_share=6.20%
- `warm_idle_300`: frames=300, median=1916509 ns, p95=1943400 ns, p99=2445343 ns, max=3531102 ns, heap_slope=plateau
  - component `capture_overhead`: median=3116 ns, median_frame_share=0.16%
  - component `central_graph`: median=1063488 ns, median_frame_share=55.49%
  - component `diagnostics`: median=76303 ns, median_frame_share=3.98%, nested_under=run_navigation
  - component `run_navigation`: median=324960 ns, median_frame_share=16.95%
  - component `selection_inspector`: median=246934 ns, median_frame_share=12.88%
  - component `timeline`: median=52188 ns, median_frame_share=2.72%
  - component `top_strip`: median=119634 ns, median_frame_share=6.24%
- `select_artifact_inspector_300`: frames=300, median=2125461 ns, p95=2166569 ns, p99=2466984 ns, max=34394358 ns, heap_slope=plateau
  - component `capture_overhead`: median=3176 ns, median_frame_share=0.14%
  - component `central_graph`: median=1061354 ns, median_frame_share=49.93%
  - component `diagnostics`: median=76263 ns, median_frame_share=3.58%, nested_under=run_navigation
  - component `run_navigation`: median=324108 ns, median_frame_share=15.24%
  - component `selection_inspector`: median=455406 ns, median_frame_share=21.42%
  - component `timeline`: median=54593 ns, median_frame_share=2.56%
  - component `top_strip`: median=119234 ns, median_frame_share=5.60%
- `inspector_run_records_expanded_300`: frames=300, median=2408343 ns, p95=2440895 ns, p99=2825647 ns, max=24579134 ns, heap_slope=plateau
  - component `capture_overhead`: median=3186 ns, median_frame_share=0.13%
  - component `central_graph`: median=1062305 ns, median_frame_share=44.10%
  - component `diagnostics`: median=76523 ns, median_frame_share=3.17%, nested_under=run_navigation
  - component `run_navigation`: median=325431 ns, median_frame_share=13.51%
  - component `selection_inspector`: median=738507 ns, median_frame_share=30.66%
  - component `timeline`: median=53721 ns, median_frame_share=2.23%
  - component `top_strip`: median=120025 ns, median_frame_share=4.98%
- `inspector_graph_edges_expanded_300`: frames=300, median=2654606 ns, p95=2676376 ns, p99=3220328 ns, max=6723798 ns, heap_slope=plateau
  - component `capture_overhead`: median=3206 ns, median_frame_share=0.12%
  - component `central_graph`: median=1065231 ns, median_frame_share=40.12%
  - component `diagnostics`: median=76674 ns, median_frame_share=2.88%, nested_under=run_navigation
  - component `run_navigation`: median=325651 ns, median_frame_share=12.26%
  - component `selection_inspector`: median=981854 ns, median_frame_share=36.98%
  - component `timeline`: median=55024 ns, median_frame_share=2.07%
  - component `top_strip`: median=119385 ns, median_frame_share=4.49%
- `inspector_artifact_edges_expanded_300`: frames=300, median=2886351 ns, p95=2911278 ns, p99=3228333 ns, max=3429812 ns, heap_slope=plateau
  - component `capture_overhead`: median=3206 ns, median_frame_share=0.11%
  - component `central_graph`: median=1063187 ns, median_frame_share=36.83%
  - component `diagnostics`: median=76664 ns, median_frame_share=2.65%, nested_under=run_navigation
  - component `run_navigation`: median=326012 ns, median_frame_share=11.29%
  - component `selection_inspector`: median=1213580 ns, median_frame_share=42.04%
  - component `timeline`: median=54964 ns, median_frame_share=1.90%
  - component `top_strip`: median=119915 ns, median_frame_share=4.15%
- `inspector_patch_debug_expanded_300`: frames=300, median=2957765 ns, p95=2977512 ns, p99=2995706 ns, max=87887897 ns, heap_slope=plateau
  - component `capture_overhead`: median=3206 ns, median_frame_share=0.10%
  - component `central_graph`: median=1060282 ns, median_frame_share=35.84%
  - component `diagnostics`: median=76403 ns, median_frame_share=2.58%, nested_under=run_navigation
  - component `run_navigation`: median=324299 ns, median_frame_share=10.96%
  - component `selection_inspector`: median=1288119 ns, median_frame_share=43.55%
  - component `timeline`: median=55174 ns, median_frame_share=1.86%
  - component `top_strip`: median=119365 ns, median_frame_share=4.03%
- `inspector_source_refs_expanded_300`: frames=300, median=3014902 ns, p95=3046262 ns, p99=3442355 ns, max=3665805 ns, heap_slope=plateau
  - component `capture_overhead`: median=3206 ns, median_frame_share=0.10%
  - component `central_graph`: median=1064520 ns, median_frame_share=35.30%
  - component `diagnostics`: median=76784 ns, median_frame_share=2.54%, nested_under=run_navigation
  - component `run_navigation`: median=327074 ns, median_frame_share=10.84%
  - component `selection_inspector`: median=1336740 ns, median_frame_share=44.33%
  - component `timeline`: median=55655 ns, median_frame_share=1.84%
  - component `top_strip`: median=121839 ns, median_frame_share=4.04%
- `inspector_artifact_ids_expanded_300`: frames=300, median=3097487 ns, p95=3123045 ns, p99=3603598 ns, max=4845240 ns, heap_slope=plateau
  - component `capture_overhead`: median=3216 ns, median_frame_share=0.10%
  - component `central_graph`: median=1061944 ns, median_frame_share=34.28%
  - component `diagnostics`: median=76333 ns, median_frame_share=2.46%, nested_under=run_navigation
  - component `run_navigation`: median=324500 ns, median_frame_share=10.47%
  - component `selection_inspector`: median=1422823 ns, median_frame_share=45.93%
  - component `timeline`: median=57478 ns, median_frame_share=1.85%
  - component `top_strip`: median=119103 ns, median_frame_share=3.84%
- `patch_debug_cold_300`: frames=300, median=3102587 ns, p95=3177559 ns, p99=3232952 ns, max=9730135 ns, heap_slope=plateau
  - component `capture_overhead`: median=3236 ns, median_frame_share=0.10%
  - component `central_graph`: median=1064339 ns, median_frame_share=34.30%
  - component `diagnostics`: median=76534 ns, median_frame_share=2.46%, nested_under=run_navigation
  - component `run_navigation`: median=326022 ns, median_frame_share=10.50%
  - component `selection_inspector`: median=1424124 ns, median_frame_share=45.90%
  - component `timeline`: median=57498 ns, median_frame_share=1.85%
  - component `top_strip`: median=120275 ns, median_frame_share=3.87%
- `patch_debug_warm_300`: frames=300, median=3106214 ns, p95=3145428 ns, p99=3562020 ns, max=3614889 ns, heap_slope=plateau
  - component `capture_overhead`: median=3256 ns, median_frame_share=0.10%
  - component `central_graph`: median=1065832 ns, median_frame_share=34.31%
  - component `diagnostics`: median=76824 ns, median_frame_share=2.47%, nested_under=run_navigation
  - component `run_navigation`: median=328076 ns, median_frame_share=10.56%
  - component `selection_inspector`: median=1426589 ns, median_frame_share=45.92%
  - component `timeline`: median=57638 ns, median_frame_share=1.85%
  - component `top_strip`: median=122751 ns, median_frame_share=3.95%
- `mode_lineage_300`: frames=300, median=2262039 ns, p95=2604482 ns, p99=2759833 ns, max=3785089 ns, heap_slope=plateau
  - component `capture_overhead`: median=3236 ns, median_frame_share=0.14%
  - component `central_graph`: median=222978 ns, median_frame_share=9.85%
  - component `diagnostics`: median=77615 ns, median_frame_share=3.43%, nested_under=run_navigation
  - component `run_navigation`: median=327516 ns, median_frame_share=14.47%
  - component `selection_inspector`: median=1424856 ns, median_frame_share=62.98%
  - component `timeline`: median=57568 ns, median_frame_share=2.54%
  - component `top_strip`: median=119915 ns, median_frame_share=5.30%
- `mode_artifact_tree_300`: frames=300, median=3100984 ns, p95=3122564 ns, p99=3224536 ns, max=6092272 ns, heap_slope=plateau
  - component `capture_overhead`: median=3176 ns, median_frame_share=0.10%
  - component `central_graph`: median=1063457 ns, median_frame_share=34.29%
  - component `diagnostics`: median=76925 ns, median_frame_share=2.48%, nested_under=run_navigation
  - component `run_navigation`: median=325541 ns, median_frame_share=10.49%
  - component `selection_inspector`: median=1424225 ns, median_frame_share=45.92%
  - component `timeline`: median=57308 ns, median_frame_share=1.84%
  - component `top_strip`: median=119214 ns, median_frame_share=3.84%
- `toggle_hide_unconsidered_children_300`: frames=300, median=2062614 ns, p95=2215510 ns, p99=2672099 ns, max=6163145 ns, heap_slope=plateau
  - component `capture_overhead`: median=3076 ns, median_frame_share=0.14%
  - component `central_graph`: median=1065992 ns, median_frame_share=51.68%
  - component `diagnostics`: median=76794 ns, median_frame_share=3.72%, nested_under=run_navigation
  - component `run_navigation`: median=325742 ns, median_frame_share=15.79%
  - component `selection_inspector`: median=386826 ns, median_frame_share=18.75%
  - component `timeline`: median=51417 ns, median_frame_share=2.49%
  - component `top_strip`: median=118393 ns, median_frame_share=5.73%

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-run-picker-startup-fast-r2/standard.puffin`: 13354980 bytes, sha256 `41cf4f67b74cecb7b3d4a6f765bacdce7b7a44a24fec7f5d99bb2a5fda7a6144`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/startup_frames_300.heap.json`: 4132 bytes, sha256 `61a887a60f8525b69c47be58397d540c163adecb79c534c6f7eb70a82eb32805`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/warm_idle_300.heap.json`: 4084 bytes, sha256 `d5ac8fad86a6e934c639f772e950c1f00d5c53749843ae934f69fdb59985ae6f`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/select_artifact_inspector_300.heap.json`: 4113 bytes, sha256 `cada42740876127f6806d755417895de558c76f9f45f4b1836870efcaaa96a9d`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/inspector_run_records_expanded_300.heap.json`: 4512 bytes, sha256 `c7aab33acfcffdb663c9c992151bd3d5e3e20360a49844e4ae22692f99eacd47`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/inspector_graph_edges_expanded_300.heap.json`: 4927 bytes, sha256 `0bf57ecd0a80d5f31dab14950591f2e571f73e8be3db6706fb1542e1150eb2ad`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/inspector_artifact_edges_expanded_300.heap.json`: 5331 bytes, sha256 `15c03d50b75bfaee73b5f8554a9ec5a09bca48e746b522bf20f08437396e6441`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/inspector_patch_debug_expanded_300.heap.json`: 5751 bytes, sha256 `5e2d49b5f0fe4445b325f561f8faa004d4eae2c8974dfe023598a22fce10ce52`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/inspector_source_refs_expanded_300.heap.json`: 6151 bytes, sha256 `425a40b08417b5eaca21ab207931f5082abcfb8a9391ae045b9411862e965e40`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/inspector_artifact_ids_expanded_300.heap.json`: 6560 bytes, sha256 `c93d4061af5574945a9f18713baa5c5430e413df0ef448d0be082297fc4343c3`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/patch_debug_cold_300.heap.json`: 6567 bytes, sha256 `2493d59c4ef5ea62293129adae66fcffc8a6eba421836e03f4cc4fbff88269c2`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/patch_debug_warm_300.heap.json`: 6554 bytes, sha256 `b07f07e7c5f46be3196db2f05627dd07330d5e554366a7c04b4fb1dc8e2aa725`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/mode_lineage_300.heap.json`: 6570 bytes, sha256 `471bd0d4c0b59108528475840a26bb36b7f5ece13ff2b9aeabb12cdbd219f68e`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/mode_artifact_tree_300.heap.json`: 6575 bytes, sha256 `fcab005b47985e0c749bdaa1f825126bb0b8b2d0350971284886dfbdba4e63f4`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-picker-startup-fast-r2/toggle_hide_unconsidered_children_300.heap.json`: 4107 bytes, sha256 `a475b86a466b0f042517f5341d6c8060a64f2928b38458a059b21f46bcd4f670`

See `report.json` for typed timings and compact allocation summaries.
