# eframe Root Attribution Findings

native interactive window benchmark verified the new allocation attribution spans against the standard Prototype 1 run root.

## Change Summary

Added benchmark-only allocation span coverage for:

- `eframe_run_native` around the native runner boundary.
- `benchmark_frame_begin`, `benchmark_apply_action`, `frame_prepare_selection`,
  `emit_diagnostics`, `puffin_capture`, `benchmark_finish_frame`,
  `benchmark_frame_end`, and `benchmark_end_frame` inside the app frame path.

This was an attribution change only. It was not intended to reduce allocation
counts or bytes.

## Commands

- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-eframe-root-attribution --benchmark-scenario startup_frames_300 --benchmark-scenario warm_idle_300 --benchmark-scenario inspector_run_records_phase_sequence_30 --benchmark-scenario inspector_run_records_phase_sequence_alternate_30 --benchmark-scenario inspector_llm_calls_phase_sequence_30 --benchmark-scenario inspector_llm_calls_phase_sequence_alternate_30`
- `target/debug/ploke-egui bench --bd crates/ploke-egui/docs/profiling/benchmarks/20260518-eframe-root-attribution`
- Focused timing check: `target/debug/ploke-egui --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-eframe-root-attribution-llm-alt-rerun --benchmark-scenario inspector_llm_calls_phase_sequence_alternate_30`

## Baseline

Compared against `crates/ploke-egui/docs/profiling/benchmarks/20260518-panel-layout-attribution/report.json`.

The baseline and new runs were both dirty-relevant. The new run's benchmark-relevant dirty paths were the instrumentation files:

- `crates/ploke-egui/src/allocation.rs`
- `crates/ploke-egui/src/native.rs`
- `crates/ploke-egui/src/ui/app/mod.rs`

The benchmark also observed unrelated dirty paths in `ploke-eval` and
`ploke-tui`; those were not edited for this pass.

## Allocation Summary

Standard mode did not capture callsite backtraces. `top callsites` is therefore
zero for every scenario.

| Scenario | Slope | Median allocs/frame | Median object bytes/frame | Median wrapped bytes/frame | Live object bytes | Live wrapped bytes | Top bytes group | Top count group | Top retained group |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
| `startup_frames_300` | plateau | 1171 | 828189 | 839552 | 2653777 | 2701096 | `eframe_run_native` | `eframe_run_native` | `eframe_run_native` |
| `warm_idle_300` | plateau | 1167 | 827686 | 839008 | 1090055 | 1123024 | `eframe_run_native` | `eframe_run_native` | `eframe_run_native` |
| `inspector_run_records_phase_sequence_30` | growing | 1243 | 909082 | 921048 | 1430119 | 1467352 | `eframe_run_native` | `eframe_run_native` | `eframe_run_native` |
| `inspector_run_records_phase_sequence_alternate_30` | plateau | 1240 | 908293 | 920248 | 1220649 | 1252632 | `eframe_run_native` | `eframe_run_native` | `eframe_run_native` |
| `inspector_llm_calls_phase_sequence_30` | growing | 1242 | 908962 | 920920 | 1288904 | 1323840 | `eframe_run_native` | `eframe_run_native` | `eframe_run_native` |
| `inspector_llm_calls_phase_sequence_alternate_30` | plateau | 1239 | 908185 | 920128 | 1094166 | 1127008 | `eframe_run_native` | `eframe_run_native` | `eframe_run_native` |

## Findings

The old `root_ui_thread` bucket moved almost entirely into
`eframe_run_native`. In the new full run, `eframe_run_native` accounts for
about 92.77%-94.91% of tracked object bytes, depending on scenario.

The app callback itself is not the main allocation source. In the same run,
`frame_update` accounts for only about 125010-138900 total object bytes per
scenario, or about 463 object bytes/frame. The new app-frame subspans are also
small: `benchmark_end_frame` is about 2719-3472 object bytes/frame, and
`frame_prepare_selection` is about 122-129 object bytes/frame in selected
scenarios.

This means the remaining large bucket is allocation after or around the app
callback inside the native `eframe::run_native` loop: egui/eframe platform
processing, tessellation, font/paint preparation, or wgpu/native backend work.
The current standard mode can now prove that the allocation is outside the
ploke-egui `OperatorApp::ui` body, but it still cannot split the dependency
internals without callsite sampling or upstream/library spans.

## Improvements

- Attribution improved: the previous ambiguous `root_ui_thread` row is now
  separated as `eframe_run_native`.
- App-frame gaps are measured and are not large enough to explain the root
  allocation debt.

## Regressions

No confirmed allocation regression was introduced by the attribution spans. The
median allocation counts and object bytes are effectively flat versus
`20260518-panel-layout-attribution`.

The full run showed one timing outlier:
`inspector_llm_calls_phase_sequence_alternate_30` moved from 2218091 ns median
to 3716439 ns median. A focused rerun of that scenario measured 2283234 ns
median and 3676373 ns p95, so I treat the full-run median spike as
inconclusive rather than a confirmed persistent regression. The focused rerun
started from a different scenario order, so its allocation totals are not a
strict apples-to-apples allocation baseline.

## Remaining Debt

Every measured scenario remains far above the current allocation tripwires of
100 allocations/frame and 64 KiB object bytes/frame. The steady idle scenario
still measures 1167 allocations/frame and 827686 object bytes/frame.

The next useful measurement is a focused sampled-callsite mode for a small
scenario set around `eframe_run_native`, or targeted dependency-level
instrumentation if egui/eframe exposes a usable hook. Standard span mode has
localized the debt to the native runner boundary but cannot name the exact
allocation callsites.

## Unmeasured Risk

The benchmark ran in the local native window environment. The focused rerun
emitted Mesa/EGL warnings but still completed. GPU/driver memory remains
outside the tracking-allocator object/wrapped-byte surface.
