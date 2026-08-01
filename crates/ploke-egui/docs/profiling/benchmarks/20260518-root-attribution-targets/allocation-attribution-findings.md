Native interactive window allocation benchmarking was run for the root-attribution tracking targets.

Command:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-root-attribution-targets --benchmark-scenario startup_frames_300 --benchmark-scenario warm_idle_300 --benchmark-scenario inspector_run_records_phase_sequence_30 --benchmark-scenario inspector_run_records_phase_sequence_alternate_30
```

Generated files:

- `README.md`: native benchmark summary.
- `report.json`: typed benchmark report.
- `allocation-attribution-findings.md`: this findings note.

## Change Summary

This run adds three attribution targets:

- `root_ui_thread` / `root_other_thread`: split fallback root allocations by the benchmark UI thread versus other threads.
- `central_graph_widget_add`: isolate the `egui_graphs::GraphView` widget add call from the surrounding central graph projection and diagnostics work.
- `egui_text_font_layout`: isolate cached egui text/font galley layout from app-owned string prep and cache bookkeeping.

## Baseline Comparison

This is an attribution comparison, not a strict optimization comparison. The new groups intentionally redistribute allocations that previously appeared under fallback `root`, so same-scenario byte totals remain useful but group names are not directly comparable to earlier reports.

Nearest context reports:

- `../20260518-run-picker-startup-fast-r2/report.json`
- `../20260518-run-records-phase-heap-deltas/report.json`

## Scenario Summary

| Scenario | Heap slope | Median allocs/frame | Median object bytes/frame | Scenario object bytes | End live object bytes |
| --- | --- | ---: | ---: | ---: | ---: |
| `startup_frames_300` | `plateau` | `1155` | `826199` | `254305877` | `2649807` |
| `warm_idle_300` | `plateau` | `1155` | `826191` | `247102688` | `1091740` |
| `inspector_run_records_phase_sequence_30` | `growing` | `1229` | `907353` | `251183184` | `1427403` |
| `inspector_run_records_phase_sequence_alternate_30` | `plateau` | `1226` | `906571` | `247596258` | `1219142` |

Every measured scenario remains above both current allocation tripwires: more than 100 allocations/frame and more than 64 KiB allocated object bytes/frame.

## Attribution Findings

| Scenario | `root_ui_thread` | `root_other_thread` | `central_graph_widget_add` | `egui_text_font_layout` |
| --- | ---: | ---: | ---: | ---: |
| `startup_frames_300` | `237081107` bytes, `93.22%` | `35426` bytes, `0.01%` | `5730948` bytes, `2.25%` | not in top groups |
| `warm_idle_300` | `234839934` bytes, `95.03%` | `28560` bytes, `0.01%` | `4525500` bytes, `1.83%` | not in top groups |
| `inspector_run_records_phase_sequence_30` | `235998920` bytes, `93.95%` | `25680` bytes, `0.01%` | `4484598` bytes, `1.78%` | `2691504` bytes, `1.07%` |
| `inspector_run_records_phase_sequence_alternate_30` | `235441700` bytes, `95.09%` | `25536` bytes, `0.01%` | `4121910` bytes, `1.66%` | `303076` bytes, `0.12%` |

The important finding is that the old fallback `root` bucket is almost entirely UI-thread work. It is not explained by background thread noise in this run.

`central_graph_widget_add` is a real steady baseline contributor, but it explains only about 1.6% to 2.3% of scenario object bytes. It is worth keeping as a permanent baseline group, but it is not the bulk of the old root bucket.

`egui_text_font_layout` is visible in the Run Records phase scenario, especially the primary target. It confirms that egui text/font layout is a real cold/open cost, but it is still much smaller than `root_ui_thread` at the scenario level.

## Phase Findings

The selected-collapsed to idle-expanded Run Records delta remains dominated by `root_ui_thread`.

| Scenario | Selected-collapsed median | Idle-expanded median | Delta |
| --- | ---: | ---: | ---: |
| `inspector_run_records_phase_sequence_30` | `1229` allocs/frame, `907332` bytes/frame | `1464` allocs/frame, `1087185` bytes/frame | `+235` allocs/frame, `+179853` bytes/frame |
| `inspector_run_records_phase_sequence_alternate_30` | `1226` allocs/frame, `906557` bytes/frame | `1462` allocs/frame, `1086922` bytes/frame | `+236` allocs/frame, `+180365` bytes/frame |

The open-section steady delta is still not primarily explained by named Run Records row or text spans. The previous conclusion still holds, but it is now sharper: the unexplained work is UI-thread fallback allocation, not off-thread allocation.

## Interpretation

The next target should split the UI-thread fallback bucket inside egui frame/layout boundaries rather than chase background workers.

Recommended next attribution targets:

- `egui_panel_layout`: wrap the outer panel `show_inside` calls and egui panel/frame layout machinery, especially the right inspector panel.
- `inspector_scroll_collapsing_layout`: wrap the right-panel `ScrollArea` and `CollapsingHeader` chrome, separate from section bodies.
- `egui_graphs_internal_layout`: if feasible, split `central_graph_widget_add` further around egui_graphs node/edge layout versus interaction/paint work.

Do not treat this report as proving a source-code root cause. Standard mode still has no callsite backtraces. It only narrows the high fallback bucket to UI-thread work and identifies two smaller named contributors.
