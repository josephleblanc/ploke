Focused egui renderer tests, ploke-egui benchmark tests/check, and native benchmark scenarios were run; no native interactive window pointer session was run.

# Tool Step Render Cache Benchmark Note

## Change Summary

- Cached decoded tool arguments/results behind `InspectorRenderCache` with stable `Arc` entries so a stable selected tool record does not re-decode typed tool payloads every frame.
- Routed tool-step summaries, tool UI payload rows, details/error blocks, decoded argument/result rows, and size-summary labels through existing cached egui text render helpers.
- Kept source data borrowed from persisted typed records until the egui render boundary; no semantic row DTOs or owned mirror records were introduced.

## Verification

- `cargo fmt --all`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- `cargo test -p ploke-egui render_cache_tests 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" render_cache_tests 2>&1 | tail -n 120`
- Native benchmark command:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --benchmark-suite standard \
  --benchmark-scenario inspector_run_records_phase_sequence_30 \
  --benchmark-scenario inspector_run_records_phase_sequence_alternate_30 \
  --benchmark-scenario inspector_llm_calls_phase_sequence_30 \
  --benchmark-scenario inspector_llm_calls_phase_sequence_alternate_30
```

Generated report:

- `crates/ploke-egui/docs/profiling/benchmarks/20260518-f28b720e71b9-standard/report.json`
- `crates/ploke-egui/docs/profiling/benchmarks/20260518-f28b720e71b9-standard/README.md`

## Baseline Comparison

The direct fresh pre-edit baseline comparison is invalid. A fresh baseline was run from commit `f28b720e71b9`, but the benchmark runner names standard reports from the commit hash and overwrote that same directory when the post-edit dirty benchmark was run.

The nearest prior matching report that remains available for `inspector_llm_calls_phase_sequence_alternate_30` is `20260518-eframe-root-attribution-llm-alt-rerun/report.json`. Compared with that older report:

| Metric | Older report | Current report | Delta |
| --- | ---: | ---: | ---: |
| heap slope | growing | plateau | improved |
| median allocs/frame | 1246 | 1239 | -7 |
| median object bytes/frame | 943530 | 908185 | -35345 |
| median wrapped bytes/frame | 955576 | 920128 | -35448 |
| median live object bytes/frame | 2484841 | 608859 | -1875982 |
| final live object bytes | 2950151 | 1103171 | -1846980 |
| `inspector_run_record_tool_step` allocation count | 38177 | 38081 | -96 |
| `inspector_run_record_tool_step` allocated object bytes | 6290454 | 6086800 | -203654 |

This older-report comparison is useful directional evidence only. It is not a replacement for a same-checkout fresh baseline.

## Current Allocation Summary

Standard mode captured no callsite attribution for these scenarios: `top_callsites_by_allocated_bytes` length is `0` for every measured scenario.

| Scenario | Heap slope | Median allocs/frame | Median object bytes/frame | Median wrapped bytes/frame | Median live object bytes/frame | Final live object bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `inspector_run_records_phase_sequence_30` | growing | 1248 | 909666 | 921680 | 2404620 | 2895241 |
| `inspector_run_records_phase_sequence_alternate_30` | plateau | 1240 | 908306 | 920264 | 714872 | 1205001 |
| `inspector_llm_calls_phase_sequence_30` | growing | 1242 | 908968 | 920920 | 807670 | 1289490 |
| `inspector_llm_calls_phase_sequence_alternate_30` | plateau | 1239 | 908185 | 920128 | 608859 | 1103171 |

## Top Allocation Groups

Each tuple is `group: allocation_count / allocated_object_bytes / live_object_bytes`.

### `inspector_run_records_phase_sequence_30`

- By allocated bytes: `eframe_run_native: 166383 / 238243684 / 2026441`; `run_navigation: 23602 / 2640059 / 68387`; `egui_text_font_layout: 4849 / 2691504 / 300440`.
- By allocation count: `eframe_run_native: 166383 / 238243684 / 2026441`; `central_graph_widget_add: 57086 / 1763246 / 68176`; `run_navigation: 23602 / 2640059 / 68387`.
- By retained bytes: `eframe_run_native: 166383 / 238243684 / 2026441`; `egui_text_font_layout: 4849 / 2691504 / 300440`; `frame_update: 2160 / 125010 / 90450`.

### `inspector_run_records_phase_sequence_alternate_30`

- By allocated bytes: `eframe_run_native: 165114 / 235483477 / 891797`; `central_graph_widget_add: 57060 / 1694070 / 512`; `run_navigation: 21600 / 1552500 / 3306`.
- By allocation count: `eframe_run_native: 165114 / 235483477 / 891797`; `central_graph_widget_add: 57060 / 1694070 / 512`; `run_navigation: 21600 / 1552500 / 3306`.
- By retained bytes: `eframe_run_native: 165114 / 235483477 / 891797`; `egui_text_font_layout: 222 / 303076 / 191824`; `frame_update: 2160 / 125010 / 90450`.

### `inspector_llm_calls_phase_sequence_30`

- By allocated bytes: `eframe_run_native: 184537 / 234790011 / 895281`; `inspector_run_record_tool_step: 23459 / 3894110 / 171702`; `central_graph_widget_add: 57060 / 1694070 / 512`.
- By allocation count: `eframe_run_native: 184537 / 234790011 / 895281`; `central_graph_widget_add: 57060 / 1694070 / 512`; `inspector_run_record_tool_step: 23459 / 3894110 / 171702`.
- By retained bytes: `eframe_run_native: 184537 / 234790011 / 895281`; `inspector_run_record_tool_step: 23459 / 3894110 / 171702`; `frame_update: 2160 / 125010 / 90450`.

### `inspector_llm_calls_phase_sequence_alternate_30`

- By allocated bytes: `eframe_run_native: 198009 / 236452447 / 906484`; `inspector_run_record_tool_step: 38081 / 6086800 / 38930`; `central_graph_widget_add: 57060 / 1694070 / 512`.
- By allocation count: `eframe_run_native: 198009 / 236452447 / 906484`; `central_graph_widget_add: 57060 / 1694070 / 512`; `inspector_run_record_tool_step: 38081 / 6086800 / 38930`.
- By retained bytes: `eframe_run_native: 198009 / 236452447 / 906484`; `frame_update: 2160 / 125010 / 90450`; `inspector_run_record_tool_step: 38081 / 6086800 / 38930`.

## Result And Debt

- Measured directional improvement is present against the older matching `inspector_llm_calls_phase_sequence_alternate_30` report, including lower per-frame allocation medians, lower final live bytes, and lower `inspector_run_record_tool_step` allocated bytes.
- The current focused scenarios are still above the allocation debt tripwires: all measured medians are above `100` allocations/frame and above `64 KiB` object bytes/frame.
- `eframe_run_native` remains the dominant standard-mode attribution bucket and standard mode still has no callsites, so the root bucket remains localization debt rather than a fixed app-owned source.
- `inspector_run_record_tool_step` is still visible in the LLM call scenarios and should remain a target for focused callsite/backtrace attribution or narrower render-boundary accounting.
