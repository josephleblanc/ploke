Native interactive window allocation benchmarking was verified for the new
`inspector_sections_sequence_30` scenario.

## Change Summary

- Added a filterable native benchmark scenario,
  `inspector_sections_sequence_30`.
- The scenario renders 100 warmup frames, selects the standard benchmark graph
  artifact `A1`, waits 30 frames, then forces exactly one top-level right-panel
  collapsible section open for 30 frames each.
- Added allocation scope names for right-panel collapsible render bodies and
  nested drilldowns so `tracking-allocator` group totals can distinguish the
  right-panel sections.
- Increased benchmark heap-summary group retention from 10 to 32 groups so the
  short sequence report keeps all right-panel section groups visible.

## Verification

- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui patch_diff_cache 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-scenario inspector_sections_sequence_30`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-inspector-patch-debug-focused --benchmark-scenario patch_debug_cold_300 --benchmark-scenario patch_debug_warm_300`

Current report:

- `crates/ploke-egui/docs/profiling/benchmarks/20260518-19561dec186b-standard/report.json`

Heap artifact with full group totals:

- `crates/ploke-egui/data/profiling/heap/benchmarks/20260518-19561dec186b-standard/inspector_sections_sequence_30.heap.json`

No same-scenario baseline exists yet; this run is the baseline for the new
sequence scenario.

## Collapsible Section Render Map

- `Run Records`: body rendered by `render_run_records_for_inspector`, allocation
  group `inspector_run_records`.
- `Graph edges`: body rendered by `render_graph_edges_for_inspector`, allocation
  group `inspector_graph_edges`.
- `Artifact edges`: body rendered by `render_artifact_edges_for_inspector`,
  allocation group `inspector_artifact_edges`.
- `Patch Debug`: body rendered by `render_patches_for_inspector`, allocation
  group `inspector_patch_debug`; nested `Details` bodies render through
  `render_patch_details`, allocation group `inspector_patch_details`.
  Follow-up focused spans split this section into patch item/header, diff cache,
  diff text build, diff syntax highlighting, diff galley layout, diff cache
  store, diff widget rendering, details header, and touch rendering groups.
- `Source refs`: body rendered by `render_source_refs_for_inspector`, allocation
  group `inspector_source_refs`. Follow-up focused spans split iteration from
  per-row rendering as `inspector_source_refs_iter` and
  `inspector_source_refs_row`.
- `Artifact Ids`: body rendered by `render_artifact_ids_for_inspector`,
  allocation group `inspector_artifact_ids`.
- `LLM calls`: body rendered by `render_parent_create_llm_calls`, allocation
  group `inspector_parent_create_llm_calls`.
- `Source status`: body rendered by `render_parent_create_source_status`,
  allocation group `inspector_parent_create_source_status`.
- Run-record arm sections: body rendered by `render_run_record_arm_turns`,
  allocation group `inspector_run_record_arm`.
- Run-record tool step sections: body rendered by `render_run_record_tool_step`,
  allocation group `inspector_run_record_tool_step`.
- Tool argument sections: body rendered by `render_tool_arguments_section`,
  allocation group `inspector_tool_arguments`; nested raw arguments use
  `render_tool_raw_arguments_section`, allocation group
  `inspector_tool_raw_arguments`.
- Tool result sections: body rendered by `render_tool_result_section`,
  allocation group `inspector_tool_result`; nested raw content/error use
  `render_tool_raw_result_section`, allocation group `inspector_tool_raw_result`.
- Tool UI payload sections: body rendered by `render_tool_ui_payload`,
  allocation group `inspector_tool_ui_payload`; nested details use
  `render_tool_ui_payload_details`, allocation group
  `inspector_tool_ui_details`; nested typed errors use
  `render_tool_error_wire`, allocation group `inspector_tool_error`.
- Tool argument edit/patch sections are scoped as
  `inspector_tool_argument_edit` and `inspector_tool_argument_patch`.
- List-dir result details and entry sections are scoped as
  `inspector_tool_result_list_dir_details` and
  `inspector_tool_result_list_dir_entry`.
- Context sections use `render_concise_context`, allocation group
  `inspector_context`.
- Agent-turn fallback sections use `render_agent_turns`, allocation group
  `inspector_agent_turn`.

## Allocation Results

Scenario summary:

- frames: `310`
- median frame time: `1,970,678 ns`
- p95 frame time: `2,495,892 ns`
- p99 frame time: `23,807,329 ns`
- max frame time: `88,130,162 ns`
- heap slope: `growing`
- median allocations/frame: `1258`
- median object bytes/frame: `945,580`
- median wrapped bytes/frame: `957,576`
- median live object bytes/frame: `2,530,129`
- live object bytes at scenario end: `4,315,687`
- callsite attribution: not captured; `top_callsites_by_allocated_bytes` was
  empty.

First-50 vs last-50 allocation window:

- first 50: median `1144` allocations/frame, `859,078` object bytes/frame,
  `870,120` wrapped bytes/frame, `1,715,031` live object bytes,
  `1,732,504` live wrapped bytes.
- last 50: median `1258` allocations/frame, `945,576` object bytes/frame,
  `957,576` wrapped bytes/frame, `4,224,456` live object bytes,
  `4,314,920` live wrapped bytes.

Right-panel section groups from the initial sequence run, normalized by the
30-frame observation window. After the follow-up split, the Patch Debug row is
represented by the sum of the focused `inspector_patch_debug_*` subspans:

- `inspector_patch_debug`: about `527` allocations/frame, `193,285` object
  bytes/frame, `198,125` wrapped bytes/frame, ending with `1,083,400` live
  object bytes.
- `inspector_run_records`: about `127` allocations/frame, `47,697` object
  bytes/frame, `48,828` wrapped bytes/frame, ending with `252,734` live object
  bytes.
- `inspector_graph_edges`: about `56` allocations/frame, `12,102` object
  bytes/frame, `12,581` wrapped bytes/frame, ending with `113,044` live object
  bytes.
- `inspector_artifact_edges`: about `37` allocations/frame, `2,827` object
  bytes/frame, `3,124` wrapped bytes/frame, ending with no retained object
  bytes.
- `inspector_artifact_ids`: about `11` allocations/frame, `1,822` object
  bytes/frame, `1,916` wrapped bytes/frame, ending with `8,750` live object
  bytes.
- `inspector_source_refs`: about `7` allocations/frame, `836` object
  bytes/frame, `900` wrapped bytes/frame, ending with `7,734` live object
  bytes.

## Findings

- The new staged scenario is useful: it isolates all six top-level right-panel
  sections in one short native run and keeps separate allocation groups in the
  heap artifact.
- `Patch Debug` is the largest measured right-panel section in this run. It is
  the only top-level section group that exceeds the 64 KiB/frame object-byte
  tripwire by itself.
- `Run Records` is the next right-panel section to inspect. It is below the
  64 KiB/frame byte tripwire, but still above the 100 allocations/frame tripwire
  by the 30-frame normalized estimate.
- The full scenario remains allocation debt regardless of section attribution:
  median `1258` allocations/frame and `945,580` object bytes/frame are far above
  both tripwires.
- The `growing` heap slope is a real warning for this scenario. The benchmark
  opens progressively heavier sections over time, so some growth is expected,
  but the last-50 live byte window is much higher than the first-50 window and
  should not be treated as steady-state acceptable.
- Standard mode still has no callsite backtraces, so the section groups identify
  the next code areas to inspect, not exact allocation callsites.

## Follow-up Focused Span Results

Native interactive window allocation benchmarking was verified after adding the
focused Patch Debug and Source refs spans.

Focused sequence report:

- `crates/ploke-egui/docs/profiling/benchmarks/20260518-19561dec186b-standard/report.json`

Cold/warm Patch Debug report:

- `crates/ploke-egui/docs/profiling/benchmarks/20260518-inspector-patch-debug-focused/report.json`

The first sequence finding still holds, but the new spans localize the Patch
Debug source. In the 30-frame sequence window, Patch Debug allocations are
mostly cold diff construction:

- `inspector_patch_debug_diff_layout`: `6,422` allocations,
  `3,561,847` object bytes.
- `inspector_patch_debug_diff_highlight`: `8,318` allocations,
  `1,824,234` object bytes.
- `inspector_patch_debug_diff_build_text`: `80` allocations,
  `271,804` object bytes.
- `inspector_patch_debug_diff_widget`: `456` allocations,
  `40,296` object bytes.
- `inspector_patch_debug_header` plus `inspector_patch_debug_details_header`:
  `532` allocations, `99,709` object bytes.

That accounts for the previous `inspector_patch_debug` total. The expensive
work is the first cached diff build/layout/highlight for the selected artifact's
patches, not the steady cached diff widget body.

The focused cold/warm benchmark confirms that interpretation:

- `patch_debug_cold_300`: median `1283` allocations/frame,
  `1,100,202` object bytes/frame, heap slope `plateau`.
- `patch_debug_warm_300`: median `1283` allocations/frame,
  `1,100,201` object bytes/frame, heap slope `plateau`.
- Cold-only diff build spans total about `5,896,717` object bytes across the
  300-frame cold scenario.
- Warm Patch Debug no longer reports diff build/highlight/layout groups; the
  remaining Patch Debug-specific groups are `inspector_patch_debug_diff_widget`
  at `338,400` object bytes, `inspector_patch_debug_details_header` at
  `191,100` object bytes, and `inspector_patch_debug_header` at `57,600` object
  bytes across 300 frames.

Source refs stayed small after the focused split:

- `inspector_source_refs_row`: `237` allocations, `25,094` object bytes in the
  sequence run.

## Next Action

Use this sequence as the fast right-panel allocation smoke test. For root cause,
the next code change should target Patch Debug diff construction and caching:
prewarm the selected artifact's patch diff galleys before measuring the
section, or move diff highlighting/layout behind a cache boundary keyed by the
selected artifact and theme. After that, `Run Records` is the next right-panel
section to inspect; Source refs is not a current allocation priority.

## Follow-up Nine-Phase Section Runs

Native interactive window allocation benchmarking was verified for focused
nine-phase section sequences with 30 frames per phase.

Code/report changes:

- Added filterable focused scenarios named
  `inspector_<section>_phase_sequence_30` and
  `inspector_<section>_phase_sequence_alternate_30`.
- Each scenario records typed `phase_windows` in `report.json` for:
  `idle_before_selection`, `select_node`, `idle_selected_collapsed`,
  `expand_section`, `idle_expanded`, `collapse_section`, `idle_collapsed`,
  `unselect_node`, and `idle_after_unselect`.
- Added a benchmark-only selection clear path so phases 1, 8, and 9 actually
  measure unselected inspector state.
- Added focused unit coverage that all collapsible sections have primary and
  alternate-node scenarios and that the nine 30-frame windows are generated.

Focused report:

- `crates/ploke-egui/docs/profiling/benchmarks/20260518-inspector-section-phase-sequences/report.json`
- heap artifacts under
  `crates/ploke-egui/data/profiling/heap/benchmarks/20260518-inspector-section-phase-sequences/`

Verification:

- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" inspector_section_phase_sequence 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- Native benchmark command recorded in the focused report's `command` field;
  it used `--benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-inspector-section-phase-sequences`
  and all primary plus alternate `inspector_<section>_phase_sequence_*_30`
  scenario filters.

Phase findings, using median per-frame deltas from the previous relevant idle
phase:

| Phase or section | Primary delta | Alternate delta | Finding |
| --- | ---: | ---: | --- |
| Select node | `+74` allocations/frame, `+90.7 KiB` object bytes/frame | `+71` allocations/frame, `+89.9 KiB` object bytes/frame | Nearly fixed selection cost over unselected idle. |
| Unselect node | `-74` allocations/frame, `-90.7 KiB` object bytes/frame | `-71` allocations/frame, `-89.9 KiB` object bytes/frame | Reverses the steady selected-node cost. |
| Run Records expand | `+230` allocations/frame, `+179,194` object bytes/frame | `+233` allocations/frame, `+179,974` object bytes/frame | Largest non-Patch-debug steady section cost. |
| Patch Debug expand | `+66` allocations/frame, `+160,090` object bytes/frame | `+69` allocations/frame, `+46,481` object bytes/frame | Median count is smaller, but bytes and cold retained diff work remain high. |
| Graph edges expand | `+188` allocations/frame, about `+136 KiB` object bytes/frame | `+88` allocations/frame, about `+11.3 KiB` object bytes/frame | Selection-sensitive expansion cost. |
| Artifact edges expand | `+188` allocations/frame, about `+136 KiB` object bytes/frame | `+88` allocations/frame, about `+11.3 KiB` object bytes/frame | Selection-sensitive expansion cost. |
| Source refs expand | `+32` allocations/frame, `+4,429` object bytes/frame | `+22` allocations/frame, `+3,341` object bytes/frame | Comparatively small. |
| Artifact Ids expand | `+41` allocations/frame, `+5,502` object bytes/frame | `+53` allocations/frame, `+6,617` object bytes/frame | Comparatively small. |

The primary Patch Debug run still shows cold diff layout/highlight groups:
`inspector_patch_debug_diff_layout` at `3,561,847` object bytes and
`inspector_patch_debug_diff_highlight` at `1,824,234`.

Idle-phase comparison across primary vs alternate node:

- The different selected node does not materially change baseline idle churn.
  Collapsed selected idle is consistently around `1213-1216`
  allocations/frame and `905 KiB` object bytes/frame.
- Unselected idle is consistently around `1142` allocations/frame and
  `815 KiB` object bytes/frame.
- The first primary `Run Records` sequence started with higher live bytes than
  later sequences, so live-byte comparisons should be read as scenario-local.
  The per-frame allocation deltas are more stable than absolute live-byte
  levels across scenario order.

Debt status:

- Every focused phase still exceeds both current allocation tripwires:
  `>100` allocations/frame and `>64 KiB` object bytes/frame.
- Standard mode still captured no callsite backtraces; these phase windows rank
  transitions and sections, but they do not prove exact allocation callsites.
- Heap slope is `growing` for `Run Records` and `Patch Debug`; other focused
  section runs were `plateau`.
