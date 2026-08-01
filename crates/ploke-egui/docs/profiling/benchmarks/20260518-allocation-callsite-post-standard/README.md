# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `571576ed4472`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/benchmark/allocation_breakdown.rs`
- `crates/ploke-egui/src/cli/mod.rs`
- `crates/ploke-egui/src/native.rs`
- `crates/ploke-egui/src/ui/view/projection.rs`

unrelated dirty paths:
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`
- `docs/active/agents/2026-05-08_bounded-edit-surface-handoff.md`
- `docs/workflow/evalnomicon/drafts/edit-surface/proof-index.md`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-571576ed4472-standard/`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 6693 ns
- `FsRunStore::load`: 944126054 ns
- `FsRunStore::load_history_blocks`: 24080714 ns
- `FsRunStore::load_transition_journal`: 4644295 ns
- `FsRunStore::load_record_set`: 972854661 ns
- `compressed_run_record_profile_probe`: 115507133 ns
- `Graph::from_records`: 6415076 ns
- `graph_load_total`: 1094779365 ns
- note: run_picker_discovery=6693 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=2260812 ns, p95=3078931 ns, p99=26403183 ns, max=63696480 ns, heap_slope=growing
  - component `capture_overhead`: median=8035 ns, median_frame_share=0.35%
  - component `central_graph`: median=1330553 ns, median_frame_share=58.85%
  - component `diagnostics`: median=72567 ns, median_frame_share=3.20%, nested_under=run_navigation
  - component `run_navigation`: median=291107 ns, median_frame_share=12.87%
  - component `selection_inspector`: median=414410 ns, median_frame_share=18.33%
  - component `timeline`: median=55324 ns, median_frame_share=2.44%
  - component `top_strip`: median=76714 ns, median_frame_share=3.39%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1171, median_object_bytes=828189, median_wrapped_bytes=839552, median_live_object_bytes=1626786
  - phase 2 `select_node`: frames=31..60, median_allocs=1247, median_object_bytes=909568, median_wrapped_bytes=921576, median_live_object_bytes=1832987
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1247, median_object_bytes=909574, median_wrapped_bytes=921584, median_live_object_bytes=1943340
  - phase 4 `expand_section`: frames=91..120, median_allocs=1482, median_object_bytes=1089415, median_wrapped_bytes=1103328, median_live_object_bytes=2290197
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1482, median_object_bytes=1089410, median_wrapped_bytes=1103328, median_live_object_bytes=2397818
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1252, median_object_bytes=910229, median_wrapped_bytes=922296, median_live_object_bytes=2506607
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1252, median_object_bytes=910213, median_wrapped_bytes=922280, median_live_object_bytes=2616278
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1178, median_object_bytes=819516, median_wrapped_bytes=830960, median_live_object_bytes=2727294
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1178, median_object_bytes=819515, median_wrapped_bytes=830952, median_live_object_bytes=2834436
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=2253478 ns, p95=3195480 ns, p99=3374898 ns, max=4996579 ns, heap_slope=plateau
  - component `capture_overhead`: median=7985 ns, median_frame_share=0.35%
  - component `central_graph`: median=1330552 ns, median_frame_share=59.04%
  - component `diagnostics`: median=72406 ns, median_frame_share=3.21%, nested_under=run_navigation
  - component `run_navigation`: median=290035 ns, median_frame_share=12.87%
  - component `selection_inspector`: median=410442 ns, median_frame_share=18.21%
  - component `timeline`: median=55395 ns, median_frame_share=2.45%
  - component `top_strip`: median=76324 ns, median_frame_share=3.38%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818015, median_wrapped_bytes=829328, median_live_object_bytes=73852
  - phase 2 `select_node`: frames=31..60, median_allocs=1240, median_object_bytes=908282, median_wrapped_bytes=920240, median_live_object_bytes=223906
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1240, median_object_bytes=908283, median_wrapped_bytes=920240, median_live_object_bytes=331667
  - phase 4 `expand_section`: frames=91..120, median_allocs=1476, median_object_bytes=1088664, median_wrapped_bytes=1102512, median_live_object_bytes=606857
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1476, median_object_bytes=1088664, median_wrapped_bytes=1102512, median_live_object_bytes=714229
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1243, median_object_bytes=908697, median_wrapped_bytes=920680, median_live_object_bytes=822410
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1243, median_object_bytes=908691, median_wrapped_bytes=920672, median_live_object_bytes=933481
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1172, median_object_bytes=818768, median_wrapped_bytes=830136, median_live_object_bytes=1043270
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1172, median_object_bytes=818767, median_wrapped_bytes=830136, median_live_object_bytes=1150490
- `inspector_llm_calls_phase_sequence_30`: frames=270, median=2257997 ns, p95=3249763 ns, p99=3697445 ns, max=7242152 ns, heap_slope=growing
  - component `capture_overhead`: median=8085 ns, median_frame_share=0.35%
  - component `central_graph`: median=1334580 ns, median_frame_share=59.10%
  - component `diagnostics`: median=72326 ns, median_frame_share=3.20%, nested_under=run_navigation
  - component `run_navigation`: median=290056 ns, median_frame_share=12.84%
  - component `selection_inspector`: median=413006 ns, median_frame_share=18.29%
  - component `timeline`: median=55044 ns, median_frame_share=2.43%
  - component `top_strip`: median=76985 ns, median_frame_share=3.40%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=818001, median_wrapped_bytes=829312, median_live_object_bytes=73775
  - phase 2 `select_node`: frames=31..60, median_allocs=1242, median_object_bytes=908952, median_wrapped_bytes=920904, median_live_object_bytes=197607
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1242, median_object_bytes=908943, median_wrapped_bytes=920896, median_live_object_bytes=307242
  - phase 4 `expand_section`: frames=91..120, median_allocs=2139, median_object_bytes=1136423, median_wrapped_bytes=1156416, median_live_object_bytes=700749
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2139, median_object_bytes=1136434, median_wrapped_bytes=1156432, median_live_object_bytes=807478
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1245, median_object_bytes=909324, median_wrapped_bytes=921312, median_live_object_bytes=907182
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1245, median_object_bytes=909301, median_wrapped_bytes=921288, median_live_object_bytes=1016752
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1171, median_object_bytes=818614, median_wrapped_bytes=829976, median_live_object_bytes=1127670
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1171, median_object_bytes=818608, median_wrapped_bytes=829968, median_live_object_bytes=1234904
- `inspector_llm_calls_phase_sequence_alternate_30`: frames=270, median=2267495 ns, p95=3659443 ns, p99=3994343 ns, max=4298525 ns, heap_slope=plateau
  - component `capture_overhead`: median=8135 ns, median_frame_share=0.35%
  - component `central_graph`: median=1338167 ns, median_frame_share=59.01%
  - component `diagnostics`: median=72626 ns, median_frame_share=3.20%, nested_under=run_navigation
  - component `run_navigation`: median=292811 ns, median_frame_share=12.91%
  - component `selection_inspector`: median=414840 ns, median_frame_share=18.29%
  - component `timeline`: median=55665 ns, median_frame_share=2.45%
  - component `top_strip`: median=77336 ns, median_frame_share=3.41%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1166, median_object_bytes=817998, median_wrapped_bytes=829312, median_live_object_bytes=73945
  - phase 2 `select_node`: frames=31..60, median_allocs=1239, median_object_bytes=908167, median_wrapped_bytes=920112, median_live_object_bytes=195683
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1239, median_object_bytes=908166, median_wrapped_bytes=920112, median_live_object_bytes=304083
  - phase 4 `expand_section`: frames=91..120, median_allocs=2572, median_object_bytes=1194672, median_wrapped_bytes=1218600, median_live_object_bytes=501753
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=2572, median_object_bytes=1194660, median_wrapped_bytes=1218592, median_live_object_bytes=608497
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1242, median_object_bytes=908540, median_wrapped_bytes=920512, median_live_object_bytes=704859
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1242, median_object_bytes=908532, median_wrapped_bytes=920504, median_live_object_bytes=829233
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1171, median_object_bytes=818609, median_wrapped_bytes=829968, median_live_object_bytes=941460
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1171, median_object_bytes=818609, median_wrapped_bytes=829968, median_live_object_bytes=1048939

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-allocation-callsite-post-standard/standard.puffin`: 3434562 bytes, sha256 `e6680136ead10255d15924ea190a4e1423b21b20041cc34487f869f310616069`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-allocation-callsite-post-standard/inspector_run_records_phase_sequence_30.heap.json`: 14890 bytes, sha256 `72e3c06c79b7f9cc2241d37ffc31c77f0e5dd47cb4110ec30044f7917fb30fa4`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-allocation-callsite-post-standard/inspector_run_records_phase_sequence_alternate_30.heap.json`: 12376 bytes, sha256 `6bec537c608c219460a4d0311b23b63fd221e758b9812a4c11f53f8b33a09a8c`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-allocation-callsite-post-standard/inspector_llm_calls_phase_sequence_30.heap.json`: 11970 bytes, sha256 `3a66011d758cefa4f2782de455b490a3faa0b9f6daa90b68942fb8da051e5eb9`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-allocation-callsite-post-standard/inspector_llm_calls_phase_sequence_alternate_30.heap.json`: 11960 bytes, sha256 `ba541df1e17aa7818b19d74dd76c0a100660bb57505ecfa12554436c07f34bac`

See `report.json` for typed timings and compact allocation summaries.
