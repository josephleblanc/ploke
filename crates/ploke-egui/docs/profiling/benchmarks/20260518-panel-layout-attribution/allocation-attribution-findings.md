Native interactive window allocation benchmarking was run for panel, inspector
chrome, and graph-widget attribution targets.

Command:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-panel-layout-attribution --benchmark-scenario startup_frames_300 --benchmark-scenario warm_idle_300 --benchmark-scenario inspector_run_records_phase_sequence_30 --benchmark-scenario inspector_run_records_phase_sequence_alternate_30 --benchmark-scenario inspector_llm_calls_phase_sequence_30 --benchmark-scenario inspector_llm_calls_phase_sequence_alternate_30
```

Generated files:

- `README.md`: native benchmark summary.
- `report.json`: typed benchmark report.
- `allocation-attribution-findings.md`: this findings note.

## Change Summary

This run adds attribution targets for:

- outer egui panel/frame layout around the top strip, run navigation, right
  inspector, timeline, and central panel `show_inside` calls.
- right-inspector `ScrollArea` and `CollapsingHeader` chrome.
- central graph widget prep, layout-state restore, diagnostics, and owned
  node/edge shape callbacks reached from `egui_graphs`.

The `egui_graphs` crate does not expose hooks around its internal interaction,
layout-state save/load, painter allocation, or node/edge draw loops. The new
node/edge scopes measure this crate's `DisplayNode` / `DisplayEdge` callbacks.
The residual `central_graph_widget_add` bucket is still the external dependency
boundary for the rest of `GraphView::ui`.

## Baseline Comparison

Nearest baselines:

- `../20260518-root-attribution-targets/report.json` for
  `startup_frames_300`, `warm_idle_300`, and Run Records scenarios.
- `../20260518-llm-calls-phase-heap-deltas/report.json` for LLM Calls
  scenarios.

This is an attribution run, not an optimization run. The high-frequency graph
callback spans add measurable frame-time overhead.

| Scenario | Baseline median frame | New median frame | Baseline allocs/frame | New allocs/frame | Baseline object bytes/frame | New object bytes/frame |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `startup_frames_300` | `1.720 ms` | `2.034 ms` | `1155` | `1168` | `826199` | `827838` |
| `warm_idle_300` | `1.718 ms` | `2.021 ms` | `1155` | `1165` | `826191` | `827447` |
| `inspector_run_records_phase_sequence_30` | `1.871 ms` | `2.219 ms` | `1229` | `1239` | `907353` | `908600` |
| `inspector_run_records_phase_sequence_alternate_30` | `1.869 ms` | `2.178 ms` | `1226` | `1236` | `906571` | `907825` |
| `inspector_llm_calls_phase_sequence_30` | `1.854 ms` | `2.173 ms` | `1227` | `1238` | `941093` | `908478` |
| `inspector_llm_calls_phase_sequence_alternate_30` | `1.853 ms` | `2.218 ms` | `1224` | `1235` | `940310` | `907705` |

The frame-time regression is real for this measurement build. Allocation
medians moved only slightly; the frame-time cost is mainly tracing/measurement
overhead from additional spans, especially the per-node/per-edge graph callback
spans.

## Attribution Findings

| Scenario | New target highlights |
| --- | --- |
| `startup_frames_300` | `inspector_scroll_area_layout` `2169928` object bytes, `inspector_collapsing_header_layout` `1363048`, `egui_graphs_node_shape_layout` `2214536`, `egui_graphs_edge_shape_layout` `1219668`, `central_graph_widget_add` `1897076`. |
| `warm_idle_300` | `inspector_scroll_area_layout` `1572900` object bytes, `inspector_collapsing_header_layout` `1189200`, `egui_graphs_node_shape_layout` `1473600`, `egui_graphs_edge_shape_layout` `1209600`, `central_graph_widget_add` `1827900`. |
| `inspector_run_records_phase_sequence_30` | `inspector_scroll_area_layout` `1487650` object bytes, `inspector_collapsing_header_layout` `1352082`, `egui_graphs_node_shape_layout` `1688928`, `egui_graphs_edge_shape_layout` `1088640`, `central_graph_widget_add` `1694070`. |
| `inspector_llm_calls_phase_sequence_30` | `inspector_collapsing_header_layout` `1676354` object bytes, `inspector_scroll_area_layout` `1375966`, `egui_graphs_node_shape_layout` `1326240`, `egui_graphs_edge_shape_layout` `1088640`, `central_graph_widget_add` `1694070`. |

The right inspector panel shell itself is small: `egui_panel_selection_inspector_layout`
is about `127440` object bytes across 270-frame phase scenarios, or roughly
`472` bytes/frame. The larger right-panel chrome buckets are inside the
`ScrollArea` and `CollapsingHeader` calls, not the outer `Panel::right` wrapper.

The graph widget split shows that this crate's node and edge shape callbacks
are visible contributors. In warm idle, `egui_graphs_node_shape_layout` is
about `1.47 MB` per 300-frame scenario and `egui_graphs_edge_shape_layout` is
about `1.21 MB`. The residual `central_graph_widget_add` remains about
`1.83 MB`, which still needs dependency-level hooks or sampled callsites to
separate interaction, state load/save, painter, and graph draw-loop overhead.

## Phase Findings

For Run Records, the selected-collapsed to idle-expanded delta remains dominated
by UI-thread fallback allocation:

| Scenario | Selected-collapsed delta window | Idle-expanded delta window | Main idle-expanded groups |
| --- | ---: | ---: | --- |
| `inspector_run_records_phase_sequence_30` | `37171` allocs, `27260158` object bytes | `44192`, `32652220` | `root_ui_thread` `31174780`, `central_graph_widget_add` `190950`, `run_navigation` `172500` |
| `inspector_run_records_phase_sequence_alternate_30` | `37081` allocs, `27234309` object bytes | `44164`, `32664497` | `root_ui_thread` `31190957`, `central_graph_widget_add` `190950`, `run_navigation` `172500` |

For LLM Calls, the expanded section body is visible through the existing run
record tool-step span plus new collapsing-header chrome:

| Scenario | Idle-expanded delta window | Main idle-expanded groups |
| --- | ---: | --- |
| `inspector_llm_calls_phase_sequence_30` | `64052` allocs, `34081151` object bytes | `root_ui_thread` `30892601`, `inspector_run_record_tool_step` `1715760`, `inspector_collapsing_header_layout` `315240` |
| `inspector_llm_calls_phase_sequence_alternate_30` | `77043` allocs, `35828810` object bytes | `root_ui_thread` `31568360`, `inspector_run_record_tool_step` `2787660`, `inspector_collapsing_header_layout` `315240` |

## Debt And Next Action

Every measured scenario remains above the allocation tripwires. Standard mode
still captured no callsite attribution.

Keep the panel and scroll/header scopes; they identify meaningful chrome cost
without dominating the run. Treat the per-node/per-edge graph callback scopes as
focused-attribution instrumentation, not a free steady benchmark default. If
these scopes stay enabled in the standard suite, frame-time comparisons against
older standard reports must be labeled as instrumentation-regressed. The next
cleaner graph split is either a sampled callsite mode around
`central_graph_widget_add` or an upstream/local `egui_graphs` hook around
`sync_layout`, interaction handling, and drawer node/edge passes.
