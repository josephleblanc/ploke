Native interactive window allocation benchmarking was verified for phase-scoped
Run Records heap attribution.

## Change Summary

- Added `HeapProfileTotals::delta_since` and `HeapProfileSnapshot::delta_since`
  so benchmark reports can compare heap groups between two snapshots.
- Added a benchmark-only `heap_profile_snapshot` path and stored per-frame heap
  snapshots for scenario capture.
- Added `phase_windows[].heap_profile_delta` to the native benchmark report.
- Preallocated scenario frame/snapshot storage under `allocation::untracked` so
  phase attribution storage does not add avoidable tracked allocation churn.

## Verification

- `cargo test -p ploke-egui --features "dev native-benchmark" heap_profile_snapshot_delta_preserves_group_attribution 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" inspector_section_phase_windows_include_heap_profile_delta 2>&1 | tail -n 120`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" run_record_measurement_scopes_are_registered 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-phase-heap-deltas --benchmark-scenario inspector_run_records_phase_sequence_30 --benchmark-scenario inspector_run_records_phase_sequence_alternate_30`

The native run emitted EGL/Zink warnings but completed and wrote:

- `20260518-run-records-phase-heap-deltas/report.json`
- `20260518-run-records-phase-heap-deltas/README.md`

## Baseline Comparison

Baseline:
`20260518-run-records-measurement-split/report.json`.

This is an attribution comparison, not an optimization comparison. The new run
adds phase-window heap snapshots and report fields.

| Scenario | Baseline median | New median | Heap slope |
| --- | ---: | ---: | --- |
| primary | `1232` allocs/frame, `952865` object bytes/frame | `1230`, `941600` | `growing` to `growing` |
| alternate | `1214` allocs/frame, `939307` object bytes/frame | `1225`, `940662` | `plateau` to `plateau` |

## Allocation Summary

| Scenario | Overall median | End live object bytes | Root share of scenario object bytes | Callsites |
| --- | ---: | ---: | ---: | ---: |
| primary | `1230` allocs/frame, `941600` object bytes/frame | `2916493` | `92.5%` | `0` |
| alternate | `1225` allocs/frame, `940662` object bytes/frame | `1195510` | `95.3%` | `0` |

Both scenarios remain far above the allocation tripwires.

## Phase Findings

The phase delta localizes the steady Run Records open-section cost to `root`,
not to the named Run Records text spans.

| Scenario | Steady expanded delta vs selected-collapsed | Root share of that byte delta | Named row-widget delta |
| --- | ---: | ---: | ---: |
| primary | `+236` allocs/frame, `+179982` object bytes/frame | `+180.1` allocs/frame, `+173935` object bytes/frame (`96.6%`) | `+48` allocs/frame, `+4800` object bytes/frame |
| alternate | `+237` allocs/frame, `+180506` object bytes/frame | `+180.0` allocs/frame, `+174510` object bytes/frame (`96.7%`) | `+48` allocs/frame, `+4800` object bytes/frame |

The cold expansion phase still shows Run Records text layout:

| Scenario | Cold expand delta vs selected-collapsed | `inspector_run_records_text_egui_layout` in expand phase |
| --- | ---: | ---: |
| primary | `+236` allocs/frame, `+179981` object bytes/frame | `70.0` allocs/frame, `41514` object bytes/frame |
| alternate | `+237` allocs/frame, `+180509` object bytes/frame | `2.9` allocs/frame, `7779` object bytes/frame |

Interpretation:

- Text layout is a real cold/open cost and retained cache source.
- The warmed steady expanded body is not mainly the named text-layout span.
- The remaining steady delta is mostly `root`, meaning allocations happened
  outside registered benchmark scopes.
- The report still has no callsite attribution, so `root` is an attribution
  boundary, not a source-code diagnosis.

## Answer To The Gap

The previous "big attribution gap" is now measured more sharply:

- Scenario-wide, `root` still owns `92.5%` to `95.3%` of allocated object bytes.
- In the specific steady expanded-vs-collapsed phase delta, `root` explains
  about `174 KB/frame`, or `96.6%` to `96.7%` of the extra object bytes.
- The named row-widget span explains another `4.8 KB/frame`.
- Named Run Records text layout explains the cold expansion work, not the
  warmed steady expanded delta.

## Next Action

The next measurement should focus on `root` during the `idle_expanded` phase:

- add sampled callsite attribution for root allocations in the two focused Run
  Records phase scenarios, or
- split the outer egui/eframe panel/layout boundary around the right inspector,
  the panel body, and frame finalization before sampling callsites.

Do not spend the next optimization slice on generic app-owned text prep/cache
store for Run Records; this report again shows that is not the large steady
allocation source.
