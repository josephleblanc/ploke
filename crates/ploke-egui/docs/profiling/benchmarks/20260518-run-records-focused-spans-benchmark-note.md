Native interactive window allocation benchmarking was verified for focused Run
Records span attribution.

## Change Summary

- Added Run Records-only allocation scopes for slot resolution, row wrappers,
  row widgets, text galley/cache work, label widgets, id galley/cache work, and
  id widgets.
- Added a native-benchmark allocator test that verifies the new scope names are
  registered.
- Ran the primary and alternate `inspector_run_records_phase_sequence_*_30`
  scenarios into `20260518-run-records-focused-spans/report.json`.

## Verification

- `cargo test -p ploke-egui allocation 2>&1 | tail -n 120` (matched 0 tests;
  compile-only check)
- `cargo test -p ploke-egui --features "dev native-benchmark" inspector_section_phase_sequence 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" run_record_measurement_scopes_are_registered 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-run-records-focused-spans --benchmark-scenario inspector_run_records_phase_sequence_30 --benchmark-scenario inspector_run_records_phase_sequence_alternate_30`

Baseline comparison uses
`20260518-inspector-section-phase-sequences/report.json`. The comparison is
valid for phase-window medians and broad Run Records attribution shape, but the
new run intentionally moves the old `inspector_run_records` group into narrower
subspans.

## Allocation Summary

- `inspector_run_records_phase_sequence_30`: heap slope `growing`, median
  `1216` allocations/frame, `905990` object bytes/frame, `917648` wrapped
  bytes/frame, ending with `2985732` live object bytes and `3034440` live
  wrapped bytes. Standard mode captured no callsite attribution.
- `inspector_run_records_phase_sequence_alternate_30`: heap slope `growing`,
  median `1213` allocations/frame, `905210` object bytes/frame, `916856`
  wrapped bytes/frame, ending with `1254588` live object bytes and `1286296`
  live wrapped bytes. Standard mode captured no callsite attribution.
- Primary Run Records expanded idle remains `1446` allocations/frame and
  `1085188` object bytes/frame, compared with selected/collapsed idle at `1216`
  allocations/frame and `906001` object bytes/frame.
- Alternate Run Records expanded idle remains `1446` allocations/frame and
  `1085180` object bytes/frame, compared with selected/collapsed idle at `1213`
  allocations/frame and `905217` object bytes/frame.

Top focused Run Records body groups:

- primary `inspector_run_records_text_galley`: `2154` allocations, `1250086`
  object bytes, `239902` live object bytes.
- primary `inspector_run_records_widget_row`: `3120` allocations, `312000`
  object bytes, no retained object bytes.
- alternate `inspector_run_records_text_galley`: `101` allocations, `235106`
  object bytes, `166282` live object bytes.
- alternate `inspector_run_records_widget_row`: `3120` allocations, `312000`
  object bytes, no retained object bytes.

## Findings

- The top-level Run Records body is not spending this scenario on nested
  tool-payload decoding; no `inspector_tool_*` groups appear in the focused heap
  artifacts.
- Slot resolution and row wrappers are negligible in the measured body spans.
- The old broad `inspector_run_records` attribution is now split mostly between
  text galley/cache work and row widget construction.
- The focused body spans still do not explain the full process-wide
  expanded-vs-collapsed phase delta. A large share remains in root/egui/layout
  or other frame work outside the current Run Records body spans.

## Debt And Next Action

Both scenarios remain above the current allocation tripwires. The next
measurement should either add phase-scoped group summaries or split egui/root
work around the expanded Run Records phase so the remaining delta can be
assigned without guessing.

## Follow-Up Measurement Split

`20260518-run-records-measurement-split/report.json` split the Run Records text
and id cache paths into cache lookup, cache hit, owned string/id prep, egui text
layout, and cache store spans. The split confirms that the broad
`inspector_run_records_text_galley` bucket was mostly egui text layout, not
app-owned string/cache-store churn.

- primary `inspector_run_records_text_egui_layout`: `2099` allocations,
  `1245424` object bytes, `235240` live object bytes.
- primary app-owned text prep/store combined:
  `inspector_run_records_text_layout_owned_string` `1051` object bytes and
  `inspector_run_records_text_cache_store` `3611` object bytes.
- alternate `inspector_run_records_text_egui_layout`: `87` allocations,
  `233376` object bytes, `164552` live object bytes.
- alternate app-owned text prep/store combined:
  `inspector_run_records_text_layout_owned_string` `865` object bytes and
  `inspector_run_records_text_cache_store` `865` object bytes.
- `inspector_run_records_widget_row` remains material: primary `312000` object
  bytes, alternate `307200` object bytes.
- Standard mode still captured no callsite attribution. Root/layout work outside
  the Run Records body spans remains unattributed.

The split run adds measurement overhead versus this report: primary median frame
allocation moved from `1216` to `1232` allocations/frame and from `905990` to
`952865` object bytes/frame. Treat it as an attribution run, not an optimization
result.
