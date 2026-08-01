Native interactive window allocation benchmarking was verified for the Run
Records app-vs-egui measurement split.

## Change Summary

- Split Run Records text/id cache measurement into lookup, hit, owned string/id
  prep, egui layout, and cache store spans.
- Kept the measurement local to the Run Records inspector path.
- Ran the primary and alternate `inspector_run_records_phase_sequence_*_30`
  scenarios into `20260518-run-records-measurement-split/report.json`.

## Verification

- `cargo test -p ploke-egui allocation 2>&1 | tail -n 120` (matched 0 tests;
  compile-only check)
- `cargo test -p ploke-egui --features "dev native-benchmark" run_record_measurement_scopes_are_registered 2>&1 | tail -n 120`
- `cargo test -p ploke-egui inspector_text_galley_cache_reuses_stable_labels 2>&1 | tail -n 120`
- `cargo test -p ploke-egui inspector_id_galley_cache_reuses_short_id_labels 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-measurement-split --benchmark-scenario inspector_run_records_phase_sequence_30 --benchmark-scenario inspector_run_records_phase_sequence_alternate_30`

The native run emitted EGL/Zink warnings but completed and wrote `report.json`
and `README.md`.

Baseline comparison uses
`20260518-run-records-focused-spans/report.json`. The comparison is valid for
attribution shape. It is not a clean performance regression comparison because
this run intentionally adds narrower tracing spans in the measured hot path.

## Allocation Summary

- `inspector_run_records_phase_sequence_30`: heap slope `growing`, median
  `1232` allocations/frame, `952865` object bytes/frame, `964712` wrapped
  bytes/frame, ending with `3108565` live object bytes. Standard mode captured
  no callsite attribution.
- `inspector_run_records_phase_sequence_alternate_30`: heap slope `plateau`,
  median `1214` allocations/frame, `939307` object bytes/frame, `950960`
  wrapped bytes/frame, ending with `1245353` live object bytes. Standard mode
  captured no callsite attribution.
- Primary Run Records expanded idle was `1462` allocations/frame and `1132067`
  object bytes/frame, compared with selected/collapsed idle at `1232`
  allocations/frame and `952878` object bytes/frame.
- Alternate Run Records expanded idle was `1447` allocations/frame and
  `1119283` object bytes/frame, compared with selected/collapsed idle at `1214`
  allocations/frame and `939308` object bytes/frame.

## Split Findings

- `inspector_run_records_text_egui_layout` is the dominant named Run Records
  text cost: primary `2099` allocations / `1245424` object bytes; alternate
  `87` allocations / `233376` object bytes.
- App-owned text allocation is small in this scenario. Primary text owned-string
  prep is `1051` object bytes and text cache store is `3611` object bytes.
  Alternate is `865` object bytes for each.
- `inspector_run_records_widget_row` remains material: primary `312000` object
  bytes, alternate `307200` object bytes.
- The registered `inspector_run_records_id_*` split scopes do not show up as
  meaningful allocation groups in this run.
- The process-wide `root`, `central_graph`, `selection_inspector`, and
  navigation groups still dominate total heap attribution, so the remaining
  whole-frame delta is not localized by this split.

## Debt And Next Action

Both scenarios remain above the current allocation tripwires. The next
optimization target should be Run Records egui text layout and row rendering,
not generic app-owned string/cache-store churn in the Run Records section. The
remaining root/layout delta still needs phase-scoped root/layout attribution or
sampled callsites before assigning it to a source-code line.
