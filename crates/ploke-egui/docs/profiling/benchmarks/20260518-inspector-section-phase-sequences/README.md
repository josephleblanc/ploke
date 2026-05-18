# ploke-egui Native Benchmark

suite: `standard`

run_root: `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`

commit: `19561dec186b`

dirty_state: `dirty_relevant`

benchmark-relevant dirty paths:
- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/benchmark.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `crates/ploke-egui/src/ui/diff/mod.rs`
- `crates/ploke-egui/src/ui/view/mod.rs`
- `crates/ploke-egui/src/ui/view/projection.rs`

unrelated dirty paths:
- `crates/ploke-db/src/bm25_index/mod.rs`
- `crates/ploke-db/src/database.rs`
- `crates/ploke-db/src/multi_embedding/db_ext.rs`
- `crates/ploke-db/src/multi_embedding/hnsw_ext.rs`
- `crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs`
- `crates/ploke-tui/tests/integration/workspace_subset_remove.rs`
- `crates/test-utils/src/fixture_dbs.rs`
- `crates/test-utils/src/lib.rs`
- `docs/how-to/recreate-backup-db-fixtures.md`
- `docs/testing/BACKUP_DB_FIXTURES.md`
- `xtask/src/commands/db.rs`
- `xtask/src/context.rs`
- `xtask/src/main.rs`
- `xtask/tests/command_acceptance_db.rs`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-19561dec186b-standard/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-inspector-patch-debug-focused/`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-inspector-section-sequence-allocation.md`

## Run Readiness

- ready: `true`
- reason: `sealed_history_reached_policy_generation`
- history blocks: `6` / `6` expected, max height `Some(5)`
- spawned children: `18` / `5` expected minimum

## Startup

- `run_picker_discovery`: 7995 ns
- `FsRunStore::load`: 951778858 ns
- `FsRunStore::load_history_blocks`: 23503661 ns
- `FsRunStore::load_transition_journal`: 4416325 ns
- `FsRunStore::load_record_set`: 979703483 ns
- `compressed_run_record_profile_probe`: 116481498 ns
- `Graph::from_records`: 6608153 ns
- `graph_load_total`: 1102795928 ns
- note: run_picker_discovery=7995 ns (kept in startup spans)
- note: standard_run_readiness_heuristic: sealed_history_reached_policy_generation; history_blocks=6/6 max_history_height=Some(5) spawned_children=18/5 node_records=19

## Scenarios

- `inspector_run_records_phase_sequence_30`: frames=270, median=1902911 ns, p95=2398036 ns, p99=23933090 ns, max=61439692 ns, heap_slope=growing
  - component `capture_overhead`: median=3105 ns, median_frame_share=0.16%
  - component `central_graph`: median=1012201 ns, median_frame_share=53.19%
  - component `diagnostics`: median=70824 ns, median_frame_share=3.72%, nested_under=run_navigation
  - component `run_navigation`: median=283205 ns, median_frame_share=14.88%
  - component `selection_inspector`: median=374597 ns, median_frame_share=19.68%
  - component `timeline`: median=52259 ns, median_frame_share=2.74%
  - component `top_strip`: median=76925 ns, median_frame_share=4.04%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1143, median_object_bytes=824990, median_wrapped_bytes=836032, median_live_object_bytes=1627441
  - phase 2 `select_node`: frames=31..60, median_allocs=1216, median_object_bytes=905994, median_wrapped_bytes=917648, median_live_object_bytes=1838193
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1216, median_object_bytes=906005, median_wrapped_bytes=917664, median_live_object_bytes=1955094
  - phase 4 `expand_section`: frames=91..120, median_allocs=1446, median_object_bytes=1085199, median_wrapped_bytes=1098704, median_live_object_bytes=2301167
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1446, median_object_bytes=1085199, median_wrapped_bytes=1098704, median_live_object_bytes=2422245
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1216, median_object_bytes=906003, median_wrapped_bytes=917656, median_live_object_bytes=2531793
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1216, median_object_bytes=906005, median_wrapped_bytes=917664, median_live_object_bytes=2640829
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815312, median_wrapped_bytes=826344, median_live_object_bytes=2752095
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815297, median_wrapped_bytes=826328, median_live_object_bytes=2859092
- `inspector_run_records_phase_sequence_alternate_30`: frames=270, median=1920003 ns, p95=2451928 ns, p99=2661933 ns, max=4426714 ns, heap_slope=growing
  - component `capture_overhead`: median=3116 ns, median_frame_share=0.16%
  - component `central_graph`: median=1017140 ns, median_frame_share=52.97%
  - component `diagnostics`: median=71705 ns, median_frame_share=3.73%, nested_under=run_navigation
  - component `run_navigation`: median=287042 ns, median_frame_share=14.95%
  - component `selection_inspector`: median=376781 ns, median_frame_share=19.62%
  - component `timeline`: median=52299 ns, median_frame_share=2.72%
  - component `top_strip`: median=77707 ns, median_frame_share=4.04%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815304, median_wrapped_bytes=826336, median_live_object_bytes=72643
  - phase 2 `select_node`: frames=31..60, median_allocs=1213, median_object_bytes=905220, median_wrapped_bytes=916864, median_live_object_bytes=227240
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1213, median_object_bytes=905215, median_wrapped_bytes=916856, median_live_object_bytes=342594
  - phase 4 `expand_section`: frames=91..120, median_allocs=1446, median_object_bytes=1085189, median_wrapped_bytes=1098696, median_live_object_bytes=617295
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1446, median_object_bytes=1085188, median_wrapped_bytes=1098696, median_live_object_bytes=737964
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1213, median_object_bytes=905215, median_wrapped_bytes=916856, median_live_object_bytes=846362
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1213, median_object_bytes=905210, median_wrapped_bytes=916856, median_live_object_bytes=957625
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815295, median_wrapped_bytes=826328, median_live_object_bytes=1067249
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815291, median_wrapped_bytes=826320, median_live_object_bytes=1174319
- `inspector_graph_edges_phase_sequence_30`: frames=270, median=1880349 ns, p95=2418394 ns, p99=2742867 ns, max=6075957 ns, heap_slope=plateau
  - component `capture_overhead`: median=3076 ns, median_frame_share=0.16%
  - component `central_graph`: median=1013954 ns, median_frame_share=53.92%
  - component `diagnostics`: median=71054 ns, median_frame_share=3.77%, nested_under=run_navigation
  - component `run_navigation`: median=284167 ns, median_frame_share=15.11%
  - component `selection_inspector`: median=372393 ns, median_frame_share=19.80%
  - component `timeline`: median=52198 ns, median_frame_share=2.77%
  - component `top_strip`: median=76454 ns, median_frame_share=4.06%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815289, median_wrapped_bytes=826320, median_live_object_bytes=72675
  - phase 2 `select_node`: frames=31..60, median_allocs=1216, median_object_bytes=905987, median_wrapped_bytes=917640, median_live_object_bytes=201748
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1216, median_object_bytes=905983, median_wrapped_bytes=917640, median_live_object_bytes=317654
  - phase 4 `expand_section`: frames=91..120, median_allocs=1404, median_object_bytes=1042043, median_wrapped_bytes=1055216, median_live_object_bytes=455393
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1404, median_object_bytes=1042036, median_wrapped_bytes=1055208, median_live_object_bytes=578601
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1216, median_object_bytes=905997, median_wrapped_bytes=917656, median_live_object_bytes=689019
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1216, median_object_bytes=905988, median_wrapped_bytes=917640, median_live_object_bytes=798193
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815290, median_wrapped_bytes=826320, median_live_object_bytes=909526
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815296, median_wrapped_bytes=826328, median_live_object_bytes=1016627
- `inspector_graph_edges_phase_sequence_alternate_30`: frames=270, median=1945141 ns, p95=2258051 ns, p99=2464471 ns, max=3084471 ns, heap_slope=plateau
  - component `capture_overhead`: median=3106 ns, median_frame_share=0.15%
  - component `central_graph`: median=1019073 ns, median_frame_share=52.39%
  - component `diagnostics`: median=73578 ns, median_frame_share=3.78%, nested_under=run_navigation
  - component `run_navigation`: median=289867 ns, median_frame_share=14.90%
  - component `selection_inspector`: median=377151 ns, median_frame_share=19.38%
  - component `timeline`: median=52269 ns, median_frame_share=2.68%
  - component `top_strip`: median=81664 ns, median_frame_share=4.19%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815292, median_wrapped_bytes=826320, median_live_object_bytes=72612
  - phase 2 `select_node`: frames=31..60, median_allocs=1213, median_object_bytes=905217, median_wrapped_bytes=916864, median_live_object_bytes=199227
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1213, median_object_bytes=905217, median_wrapped_bytes=916864, median_live_object_bytes=314295
  - phase 4 `expand_section`: frames=91..120, median_allocs=1301, median_object_bytes=916539, median_wrapped_bytes=928880, median_live_object_bytes=437124
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1301, median_object_bytes=916546, median_wrapped_bytes=928888, median_live_object_bytes=560174
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1213, median_object_bytes=905208, median_wrapped_bytes=916848, median_live_object_bytes=667208
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1213, median_object_bytes=905220, median_wrapped_bytes=916864, median_live_object_bytes=791594
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815288, median_wrapped_bytes=826320, median_live_object_bytes=904331
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815294, median_wrapped_bytes=826328, median_live_object_bytes=1011606
- `inspector_artifact_edges_phase_sequence_30`: frames=270, median=1924071 ns, p95=2431438 ns, p99=2590398 ns, max=3086004 ns, heap_slope=plateau
  - component `capture_overhead`: median=3186 ns, median_frame_share=0.16%
  - component `central_graph`: median=1019854 ns, median_frame_share=53.00%
  - component `diagnostics`: median=74972 ns, median_frame_share=3.89%, nested_under=run_navigation
  - component `run_navigation`: median=293333 ns, median_frame_share=15.24%
  - component `selection_inspector`: median=383935 ns, median_frame_share=19.95%
  - component `timeline`: median=53250 ns, median_frame_share=2.76%
  - component `top_strip`: median=85070 ns, median_frame_share=4.42%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815288, median_wrapped_bytes=826320, median_live_object_bytes=72599
  - phase 2 `select_node`: frames=31..60, median_allocs=1216, median_object_bytes=905997, median_wrapped_bytes=917656, median_live_object_bytes=201700
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1216, median_object_bytes=905986, median_wrapped_bytes=917640, median_live_object_bytes=317732
  - phase 4 `expand_section`: frames=91..120, median_allocs=1404, median_object_bytes=1042042, median_wrapped_bytes=1055216, median_live_object_bytes=428171
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1404, median_object_bytes=1042048, median_wrapped_bytes=1055216, median_live_object_bytes=555415
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1216, median_object_bytes=906000, median_wrapped_bytes=917656, median_live_object_bytes=666264
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1216, median_object_bytes=905986, median_wrapped_bytes=917640, median_live_object_bytes=775313
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815298, median_wrapped_bytes=826328, median_live_object_bytes=886524
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815293, median_wrapped_bytes=826328, median_live_object_bytes=993820
- `inspector_artifact_edges_phase_sequence_alternate_30`: frames=270, median=1915243 ns, p95=2256888 ns, p99=2422311 ns, max=3064803 ns, heap_slope=plateau
  - component `capture_overhead`: median=3106 ns, median_frame_share=0.16%
  - component `central_graph`: median=1014995 ns, median_frame_share=52.99%
  - component `diagnostics`: median=71996 ns, median_frame_share=3.75%, nested_under=run_navigation
  - component `run_navigation`: median=287192 ns, median_frame_share=14.99%
  - component `selection_inspector`: median=375108 ns, median_frame_share=19.58%
  - component `timeline`: median=52048 ns, median_frame_share=2.71%
  - component `top_strip`: median=77256 ns, median_frame_share=4.03%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815288, median_wrapped_bytes=826320, median_live_object_bytes=72864
  - phase 2 `select_node`: frames=31..60, median_allocs=1213, median_object_bytes=905216, median_wrapped_bytes=916856, median_live_object_bytes=201869
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1213, median_object_bytes=905217, median_wrapped_bytes=916864, median_live_object_bytes=314011
  - phase 4 `expand_section`: frames=91..120, median_allocs=1301, median_object_bytes=916539, median_wrapped_bytes=928880, median_live_object_bytes=426089
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1301, median_object_bytes=916540, median_wrapped_bytes=928888, median_live_object_bytes=548697
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1213, median_object_bytes=905218, median_wrapped_bytes=916864, median_live_object_bytes=655911
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1213, median_object_bytes=905212, median_wrapped_bytes=916856, median_live_object_bytes=764379
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815294, median_wrapped_bytes=826328, median_live_object_bytes=877064
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815286, median_wrapped_bytes=826320, median_live_object_bytes=984334
- `inspector_patch_debug_phase_sequence_30`: frames=270, median=1924201 ns, p95=2238715 ns, p99=2947421 ns, max=87479074 ns, heap_slope=growing
  - component `capture_overhead`: median=3096 ns, median_frame_share=0.16%
  - component `central_graph`: median=1013102 ns, median_frame_share=52.65%
  - component `diagnostics`: median=72276 ns, median_frame_share=3.75%, nested_under=run_navigation
  - component `run_navigation`: median=286240 ns, median_frame_share=14.87%
  - component `selection_inspector`: median=385628 ns, median_frame_share=20.04%
  - component `timeline`: median=52018 ns, median_frame_share=2.70%
  - component `top_strip`: median=76875 ns, median_frame_share=3.99%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815284, median_wrapped_bytes=826312, median_live_object_bytes=72891
  - phase 2 `select_node`: frames=31..60, median_allocs=1216, median_object_bytes=905986, median_wrapped_bytes=917640, median_live_object_bytes=201506
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1216, median_object_bytes=905989, median_wrapped_bytes=917648, median_live_object_bytes=317738
  - phase 4 `expand_section`: frames=91..120, median_allocs=1282, median_object_bytes=1066079, median_wrapped_bytes=1078304, median_live_object_bytes=1524630
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1282, median_object_bytes=1066096, median_wrapped_bytes=1078328, median_live_object_bytes=1642525
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1216, median_object_bytes=905995, median_wrapped_bytes=917648, median_live_object_bytes=1751413
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1216, median_object_bytes=905989, median_wrapped_bytes=917648, median_live_object_bytes=1860717
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815297, median_wrapped_bytes=826328, median_live_object_bytes=1971976
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815282, median_wrapped_bytes=826312, median_live_object_bytes=2079067
- `inspector_patch_debug_phase_sequence_alternate_30`: frames=270, median=1916937 ns, p95=2247561 ns, p99=2336769 ns, max=14398948 ns, heap_slope=growing
  - component `capture_overhead`: median=3096 ns, median_frame_share=0.16%
  - component `central_graph`: median=1017961 ns, median_frame_share=53.10%
  - component `diagnostics`: median=73279 ns, median_frame_share=3.82%, nested_under=run_navigation
  - component `run_navigation`: median=287883 ns, median_frame_share=15.01%
  - component `selection_inspector`: median=376240 ns, median_frame_share=19.62%
  - component `timeline`: median=52429 ns, median_frame_share=2.73%
  - component `top_strip`: median=80462 ns, median_frame_share=4.19%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815285, median_wrapped_bytes=826320, median_live_object_bytes=69819
  - phase 2 `select_node`: frames=31..60, median_allocs=1213, median_object_bytes=905217, median_wrapped_bytes=916864, median_live_object_bytes=199109
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1213, median_object_bytes=905217, median_wrapped_bytes=916864, median_live_object_bytes=314220
  - phase 4 `expand_section`: frames=91..120, median_allocs=1282, median_object_bytes=951698, median_wrapped_bytes=963928, median_live_object_bytes=591677
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1282, median_object_bytes=951694, median_wrapped_bytes=963920, median_live_object_bytes=732992
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1213, median_object_bytes=905211, median_wrapped_bytes=916856, median_live_object_bytes=841294
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1213, median_object_bytes=905212, median_wrapped_bytes=916856, median_live_object_bytes=949479
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815291, median_wrapped_bytes=826320, median_live_object_bytes=1062065
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815290, median_wrapped_bytes=826320, median_live_object_bytes=1169263
- `inspector_source_refs_phase_sequence_30`: frames=270, median=1917709 ns, p95=2195513 ns, p99=2234826 ns, max=3056477 ns, heap_slope=plateau
  - component `capture_overhead`: median=3096 ns, median_frame_share=0.16%
  - component `central_graph`: median=1013753 ns, median_frame_share=52.86%
  - component `diagnostics`: median=72216 ns, median_frame_share=3.76%, nested_under=run_navigation
  - component `run_navigation`: median=287533 ns, median_frame_share=14.99%
  - component `selection_inspector`: median=380117 ns, median_frame_share=19.82%
  - component `timeline`: median=52409 ns, median_frame_share=2.73%
  - component `top_strip`: median=79700 ns, median_frame_share=4.15%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815295, median_wrapped_bytes=826328, median_live_object_bytes=72930
  - phase 2 `select_node`: frames=31..60, median_allocs=1216, median_object_bytes=905985, median_wrapped_bytes=917640, median_live_object_bytes=201723
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1216, median_object_bytes=905989, median_wrapped_bytes=917648, median_live_object_bytes=317887
  - phase 4 `expand_section`: frames=91..120, median_allocs=1248, median_object_bytes=910418, median_wrapped_bytes=922328, median_live_object_bytes=435604
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1248, median_object_bytes=910410, median_wrapped_bytes=922320, median_live_object_bytes=558334
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1216, median_object_bytes=905996, median_wrapped_bytes=917648, median_live_object_bytes=671177
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1216, median_object_bytes=905988, median_wrapped_bytes=917640, median_live_object_bytes=777863
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815283, median_wrapped_bytes=826312, median_live_object_bytes=889027
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815291, median_wrapped_bytes=826320, median_live_object_bytes=996238
- `inspector_source_refs_phase_sequence_alternate_30`: frames=270, median=1894504 ns, p95=2171026 ns, p99=2326680 ns, max=3290207 ns, heap_slope=plateau
  - component `capture_overhead`: median=3046 ns, median_frame_share=0.16%
  - component `central_graph`: median=1012039 ns, median_frame_share=53.41%
  - component `diagnostics`: median=71034 ns, median_frame_share=3.74%, nested_under=run_navigation
  - component `run_navigation`: median=283685 ns, median_frame_share=14.97%
  - component `selection_inspector`: median=377572 ns, median_frame_share=19.92%
  - component `timeline`: median=51778 ns, median_frame_share=2.73%
  - component `top_strip`: median=76344 ns, median_frame_share=4.02%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815289, median_wrapped_bytes=826320, median_live_object_bytes=72689
  - phase 2 `select_node`: frames=31..60, median_allocs=1213, median_object_bytes=905216, median_wrapped_bytes=916856, median_live_object_bytes=199205
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1213, median_object_bytes=905215, median_wrapped_bytes=916856, median_live_object_bytes=314356
  - phase 4 `expand_section`: frames=91..120, median_allocs=1235, median_object_bytes=908556, median_wrapped_bytes=920360, median_live_object_bytes=423984
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1235, median_object_bytes=908550, median_wrapped_bytes=920360, median_live_object_bytes=546616
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1213, median_object_bytes=905218, median_wrapped_bytes=916864, median_live_object_bytes=656175
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1213, median_object_bytes=905208, median_wrapped_bytes=916848, median_live_object_bytes=764761
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815292, median_wrapped_bytes=826320, median_live_object_bytes=877258
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815290, median_wrapped_bytes=826320, median_live_object_bytes=984345
- `inspector_artifact_ids_phase_sequence_30`: frames=270, median=1906737 ns, p95=2268781 ns, p99=2376493 ns, max=3034244 ns, heap_slope=plateau
  - component `capture_overhead`: median=3086 ns, median_frame_share=0.16%
  - component `central_graph`: median=1020295 ns, median_frame_share=53.51%
  - component `diagnostics`: median=72627 ns, median_frame_share=3.80%, nested_under=run_navigation
  - component `run_navigation`: median=289116 ns, median_frame_share=15.16%
  - component `selection_inspector`: median=376009 ns, median_frame_share=19.72%
  - component `timeline`: median=52339 ns, median_frame_share=2.74%
  - component `top_strip`: median=81203 ns, median_frame_share=4.25%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815296, median_wrapped_bytes=826328, median_live_object_bytes=72756
  - phase 2 `select_node`: frames=31..60, median_allocs=1216, median_object_bytes=905983, median_wrapped_bytes=917640, median_live_object_bytes=201605
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1216, median_object_bytes=905990, median_wrapped_bytes=917648, median_live_object_bytes=317530
  - phase 4 `expand_section`: frames=91..120, median_allocs=1257, median_object_bytes=911492, median_wrapped_bytes=923472, median_live_object_bytes=436210
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1257, median_object_bytes=911478, median_wrapped_bytes=923464, median_live_object_bytes=558825
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1216, median_object_bytes=905985, median_wrapped_bytes=917640, median_live_object_bytes=669207
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1216, median_object_bytes=905985, median_wrapped_bytes=917640, median_live_object_bytes=778517
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815290, median_wrapped_bytes=826320, median_live_object_bytes=889635
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815295, median_wrapped_bytes=826328, median_live_object_bytes=996893
- `inspector_artifact_ids_phase_sequence_alternate_30`: frames=270, median=1912268 ns, p95=2215149 ns, p99=2704783 ns, max=3290498 ns, heap_slope=plateau
  - component `capture_overhead`: median=3096 ns, median_frame_share=0.16%
  - component `central_graph`: median=1017790 ns, median_frame_share=53.22%
  - component `diagnostics`: median=72247 ns, median_frame_share=3.77%, nested_under=run_navigation
  - component `run_navigation`: median=288043 ns, median_frame_share=15.06%
  - component `selection_inspector`: median=374967 ns, median_frame_share=19.60%
  - component `timeline`: median=52078 ns, median_frame_share=2.72%
  - component `top_strip`: median=80071 ns, median_frame_share=4.18%
  - phase 1 `idle_before_selection`: frames=1..30, median_allocs=1142, median_object_bytes=815280, median_wrapped_bytes=826312, median_live_object_bytes=72678
  - phase 2 `select_node`: frames=31..60, median_allocs=1213, median_object_bytes=905225, median_wrapped_bytes=916872, median_live_object_bytes=199089
  - phase 3 `idle_selected_collapsed`: frames=61..90, median_allocs=1213, median_object_bytes=905199, median_wrapped_bytes=916840, median_live_object_bytes=313595
  - phase 4 `expand_section`: frames=91..120, median_allocs=1266, median_object_bytes=911816, median_wrapped_bytes=923896, median_live_object_bytes=430269
  - phase 5 `idle_expanded`: frames=121..150, median_allocs=1266, median_object_bytes=911814, median_wrapped_bytes=923896, median_live_object_bytes=552069
  - phase 6 `collapse_section`: frames=151..180, median_allocs=1213, median_object_bytes=905218, median_wrapped_bytes=916864, median_live_object_bytes=653143
  - phase 7 `idle_collapsed`: frames=181..210, median_allocs=1213, median_object_bytes=905207, median_wrapped_bytes=916848, median_live_object_bytes=761465
  - phase 8 `unselect_node`: frames=211..240, median_allocs=1142, median_object_bytes=815295, median_wrapped_bytes=826328, median_live_object_bytes=874014
  - phase 9 `idle_after_unselect`: frames=241..270, median_allocs=1142, median_object_bytes=815296, median_wrapped_bytes=826328, median_live_object_bytes=981262

## Local Puffin Captures

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/puffin/benchmarks/20260518-inspector-section-phase-sequences/standard.puffin`: 10326199 bytes, sha256 `6a7591682787c631abd2e9f20fe532c3466eebbb8f93b5e5bf1132bbdf00c913`

## Local Heap Profiles

- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_run_records_phase_sequence_30.heap.json`: 4557 bytes, sha256 `a624cad4a7838c97a9d74afab9d03905c0796f580b9a92a7f42e2bcdd5754415`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_run_records_phase_sequence_alternate_30.heap.json`: 4505 bytes, sha256 `2035447daff1c9fbe26cf2a42799265ae196ff04a58dc72df07162c8c383d47e`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_graph_edges_phase_sequence_30.heap.json`: 4503 bytes, sha256 `711758b612da832cbdfa28bd72f827118ee108bf9ac8d886f76f7adffbec06d3`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_graph_edges_phase_sequence_alternate_30.heap.json`: 4503 bytes, sha256 `79b1af318ecaad815d604b980b00670162b885fdebc6fb80f2cf313482863ab6`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_artifact_edges_phase_sequence_30.heap.json`: 4498 bytes, sha256 `33beeac95bf1c604eb07d4697d88a4edac1546aa73a06b7f386abbdd4118df1e`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_artifact_edges_phase_sequence_alternate_30.heap.json`: 4498 bytes, sha256 `28e17d4b7364e6d6752f8b345891792afdea36513bbf1d39199fc309eb75f57e`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_patch_debug_phase_sequence_30.heap.json`: 7430 bytes, sha256 `38b0ec6d35910614ec7951ce96f10ca81ca089ef705e05c2179eea8bd38c3171`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_patch_debug_phase_sequence_alternate_30.heap.json`: 7016 bytes, sha256 `bfcbd7c77d9c49839a3a4794d5512b0358ddfecb2d6bc660ac9ea8236a5beff0`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_source_refs_phase_sequence_30.heap.json`: 4499 bytes, sha256 `a2dd2c833454e61f5523be1505bea128674b75464bd52e1e2513230bdbd17899`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_source_refs_phase_sequence_alternate_30.heap.json`: 4493 bytes, sha256 `66500ac96ea4db415778e5d09a3774ca31881710b5f5329cde0c88fc03f00af2`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_artifact_ids_phase_sequence_30.heap.json`: 4496 bytes, sha256 `054af4cc4d386ac9e002583ad83d87fe1c52837af12b51d7a619b82f77c4b706`
- `/home/brasides/code/ploke/crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/inspector_artifact_ids_phase_sequence_alternate_30.heap.json`: 4496 bytes, sha256 `1fb7cb4c475998111bf3d084fba61d74c1e157630a5b2b32f8579b6f62939a3f`

See `report.json` for typed timings and compact allocation summaries.
