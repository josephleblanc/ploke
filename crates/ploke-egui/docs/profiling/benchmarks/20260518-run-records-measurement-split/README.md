# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `44149346472b`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `Cargo.lock`
- `crates/ploke-egui/docs/profiling/20260518-nine-phase-selection-run-questions.md`
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-records/Cargo.toml`
- `crates/ploke-records/src/lib.rs`
- `crates/ploke-records/src/record.rs`
- `crates/ploke-records/src/llm_response.rs`

unrelated dirty paths:
- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/lib.rs`
- `crates/ploke-eval/src/record.rs`
- `crates/ploke-llm/src/lib.rs`
- `crates/ploke-llm/src/manager/mod.rs`
- `crates/ploke-llm/src/manager/session.rs`
- `crates/ploke-tui/src/llm/manager/mod.rs`
- `crates/ploke-tui/src/llm/manager/session.rs`
- `crates/ploke-tui/src/llm/mod.rs`
- `crates/ploke-tui/src/tools/mod.rs`
- `crates/ploke-eval/src/replay/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 9227 ns
- `FsRunStore::load`: 932038292 ns
- `FsRunStore::load_history_blocks`: 23712526 ns
- `FsRunStore::load_transition_journal`: 4541177 ns
- `FsRunStore::load_record_set`: 960298326 ns
- `compressed_run_record_profile_probe`: 116582195 ns
- `Graph::from_records`: 7051688 ns
- `graph_load_total`: 1083935105 ns
- note: run_picker_discovery=9227 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=1899460 ns, p95=2954140 ns, p99=25432771 ns, max=62399516 ns, heap_slope=growing
  - component `capture_overhead`: median=3066 ns, median_frame_share=0.16%
  - component `central_graph`: median=1016860 ns, median_frame_share=53.53%
  - component `diagnostics`: median=72376 ns, median_frame_share=3.81%, nested_under=run_navigation
  - component `run_navigation`: median=288699 ns, median_frame_share=15.19%
  - component `selection_inspector`: median=387995 ns, median_frame_share=20.42%
  - component `timeline`: median=54422 ns, median_frame_share=2.86%
  - component `top_strip`: median=77424 ns, median_frame_share=4.07%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1159, median_object_bytes=871853, median_wrapped_bytes=883072, median_live_object_bytes=1689855
  - phase 2 `select_node`: frames=31..60, median_allocs=1232, median_object_bytes=952871, median_wrapped_bytes=964720, median_live_object_bytes=1918373
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1232, median_object_bytes=952878, median_wrapped_bytes=964728, median_live_object_bytes=2053408
  - phase 4 `expand_section`: frames=91..120, median_allocs=1462, median_object_bytes=1132066, median_wrapped_bytes=1145752, median_live_object_bytes=2417585
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1462, median_object_bytes=1132067, median_wrapped_bytes=1145752, median_live_object_bytes=2556574
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1232, median_object_bytes=952880, median_wrapped_bytes=964728, median_live_object_bytes=2683760
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1217, median_object_bytes=940113, median_wrapped_bytes=951784, median_live_object_bytes=2817494
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1143, median_object_bytes=849388, median_wrapped_bytes=860416, median_live_object_bytes=2925398
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1143, median_object_bytes=849391, median_wrapped_bytes=860424, median_live_object_bytes=3029785
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=1855457 ns, p95=2776078 ns, p99=3489973 ns, max=4242398 ns, heap_slope=plateau
  - component `capture_overhead`: median=3066 ns, median_frame_share=0.16%
  - component `central_graph`: median=998296 ns, median_frame_share=53.80%
  - component `diagnostics`: median=70923 ns, median_frame_share=3.82%, nested_under=run_navigation
  - component `run_navigation`: median=281466 ns, median_frame_share=15.16%
  - component `selection_inspector`: median=370282 ns, median_frame_share=19.95%
  - component `timeline`: median=51937 ns, median_frame_share=2.79%
  - component `top_strip`: median=76133 ns, median_frame_share=4.10%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1143, median_object_bytes=849391, median_wrapped_bytes=860424, median_live_object_bytes=89524
  - phase 2 `select_node`: frames=31..60, median_allocs=1214, median_object_bytes=939318, median_wrapped_bytes=950968, median_live_object_bytes=240420
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1214, median_object_bytes=939308, median_wrapped_bytes=950960, median_live_object_bytes=352634
  - phase 4 `expand_section`: frames=91..120, median_allocs=1447, median_object_bytes=1119296, median_wrapped_bytes=1132800, median_live_object_bytes=624909
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1447, median_object_bytes=1119283, median_wrapped_bytes=1132792, median_live_object_bytes=742342
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1214, median_object_bytes=939317, median_wrapped_bytes=950968, median_live_object_bytes=847891
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1214, median_object_bytes=939307, median_wrapped_bytes=950960, median_live_object_bytes=955826
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1143, median_object_bytes=849387, median_wrapped_bytes=860416, median_live_object_bytes=1062567
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1143, median_object_bytes=849394, median_wrapped_bytes=860424, median_live_object_bytes=1166549

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-run-records-measurement-split/standard.puffin`: 1801985 bytes, sha256 `527c92b1650ca2004515d5ba9a9d86b5b4fb3d8888083d0130b26527d29b8258`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-records-measurement-split/inspector_run_records_phase_sequence_30.heap.json`: 6631 bytes, sha256 `1f327d031b5fb27772ed14b319177eedb516446a50b3491630848ff9cf7d5a32`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-run-records-measurement-split/inspector_run_records_phase_sequence_alternate_30.heap.json`: 5757 bytes, sha256 `aae85ce8c13d396a87d350b78c8f28391647f4af2573b694f2a1c06d158a8eb0`

See `report.json` for typed timings and compact allocation summaries.
