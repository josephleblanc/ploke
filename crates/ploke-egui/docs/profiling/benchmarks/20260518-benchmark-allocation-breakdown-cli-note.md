# Benchmark Allocation Breakdown CLI Note

CLI snapshot/export verification was run for the benchmark allocation breakdown helper.

## Change Summary

- Added a typed `BenchmarkAllocationBreakdown` helper that loads a benchmark `report.json` from an explicit file or directory path, or finds the most recently modified tracked benchmark report by default.
- The helper reads full per-scenario heap artifacts when available, falling back to compact report groups when local heap artifacts are absent.
- Added `bench --breakdown [REPORT_OR_DIR]` and the shorter `bench --bd [REPORT_OR_DIR]` as thin CLI renderers over that helper.

## Verification

- `cargo test -p ploke-egui --features dev bench_breakdown 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features dev allocation_breakdown 2>&1 | tail -n 120`
- `cargo run -q -p ploke-egui --features dev -- bench --help`
- `cargo run -q -p ploke-egui --features dev -- bench --breakdown`
- `cargo run -q -p ploke-egui --features dev -- bench --bd`
- `git diff --check -- crates/ploke-egui`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`

The CLI default selected:

- `crates/ploke-egui/docs/profiling/benchmarks/20260518-panel-layout-attribution/report.json`

## Allocation Churn From Rendered Latest Report

No new native interactive window benchmark was run for this CLI-only reader/renderer change. The command rendered the latest existing native benchmark report and loaded full local heap artifacts for every scenario.

| scenario | slope | median allocs/frame | median object bytes/frame | median wrapped bytes/frame | top active span by object bytes |
| --- | ---: | ---: | ---: | ---: | --- |
| `startup_frames_300` | plateau | 1168 | 827838 | 839160 | `root_ui_thread` 238125823 object bytes, 93.07% |
| `warm_idle_300` | plateau | 1165 | 827447 | 838736 | `root_ui_thread` 234837170 object bytes, 94.89% |
| `inspector_run_records_phase_sequence_30` | growing | 1239 | 908600 | 920520 | `root_ui_thread` 235999081 object bytes, 93.84% |
| `inspector_run_records_phase_sequence_alternate_30` | plateau | 1236 | 907825 | 919728 | `root_ui_thread` 235494812 object bytes, 94.96% |
| `inspector_llm_calls_phase_sequence_30` | growing | 1238 | 908478 | 920384 | `root_ui_thread` 234791871 object bytes, 93.47% |
| `inspector_llm_calls_phase_sequence_alternate_30` | plateau | 1235 | 907705 | 919600 | `root_ui_thread` 236462724 object bytes, 92.78% |

Selected named-span rows from the rendered latest report:

- `warm_idle_300`: `central_graph_widget_add` 1827900 object bytes, 6093.0 object bytes/frame; `inspector_scroll_area_layout` 1572900 object bytes, 5243.0 object bytes/frame; `inspector_collapsing_header_layout` 1189200 object bytes, 3964.0 object bytes/frame.
- `inspector_run_records_phase_sequence_30`: `egui_text_font_layout` 2691504 object bytes, 9968.5 object bytes/frame; `central_graph_widget_add` 1694070 object bytes, 6274.3 object bytes/frame; `egui_graphs_node_shape_layout` 1688928 object bytes, 6255.3 object bytes/frame.
- `inspector_llm_calls_phase_sequence_alternate_30`: `inspector_run_record_tool_step` 6179722 object bytes, 22887.9 object bytes/frame; `inspector_collapsing_header_layout` 1669756 object bytes, 6184.3 object bytes/frame.

## Comparison And Risk

- Baseline comparison: not applicable for this code edit because it does not run the native UI loop or change measured render behavior.
- Measured improvements: none claimed.
- Measured regressions: none from this CLI reader/renderer verification.
- Remaining allocation debt: every rendered latest-report scenario remains above the current 100 allocations/frame and 64 KiB object-bytes/frame tripwires.
- Callsite attribution: not captured by the standard benchmark mode; the helper reports span/group attribution and full local heap artifacts when present.
