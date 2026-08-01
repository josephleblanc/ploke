# 2026-05-18 Allocation Callsite Summary

Verification surface: native interactive window.

## Reports

- Baseline standard report: `20260518-571576ed4472-standard/report.json`
- Post-change standard report: `20260518-allocation-callsite-post-standard/report.json`
- Focused sampled callsite report: `20260518-allocation-callsite-sampled/report.json`

The standard before/after reports cover these four rendered inspector scenarios:

- `inspector_run_records_phase_sequence_30`
- `inspector_run_records_phase_sequence_alternate_30`
- `inspector_llm_calls_phase_sequence_30`
- `inspector_llm_calls_phase_sequence_alternate_30`

## Verdict

The standard before/after run did not show a material app-owned allocation reduction, but it also did not show a meaningful allocation regression. Median allocations/frame stayed flat in all four scenarios, object/wrapped bytes moved only within small measurement noise, and median frame time stayed within the 5% acceptance bound.

The new sampled callsite mode is intentionally diagnostic and adds backtrace overhead. It should not be used as the frame-time acceptance surface. Its purpose is to split the previous broad `eframe_run_native` bucket into scoped callsite estimates for `eframe_run_native`, `selection_inspector`, `central_graph_widget_add`, and `inspector_run_record_tool_step`.

## Remaining Debt

The sampled callsite evidence shows the largest remaining native-window allocation debt under dependency/internal rendering work, especially:

- `eframe_run_native -> tessellate_text` in `epaint`
- `eframe_run_native -> reserve_vertices` / `reserve_triangles` in `epaint`
- `egui::layers::GraphicLayers::drain`
- `puffin::data::Stream::extend` in some scenarios

For the named app span `inspector_run_record_tool_step`, the sampled callsites are mostly egui style/layout/widget machinery (`Ui::style_mut`, `Style::clone`, `UiStack`, `RichText`, and `LayoutJob`) rather than newly identified app-owned semantic DTO or string rebuilding.

## Code Changes Covered

- Added benchmark-only focused sampled callsite accounting to allocator tracking.
- Added CLI/config/report support for `--benchmark-callsite-sample-every`.
- Added a `GraphViewDiagnostics` cache keyed by graph, viewport, style, label, layout, and readability inputs.

The visible inspector content and ordering were not intentionally changed.
