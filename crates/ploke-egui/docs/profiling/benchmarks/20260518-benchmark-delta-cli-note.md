# Benchmark Delta CLI Note

CLI snapshot/export verified the benchmark breakdown filters and delta renderer.

## Change Summary

- Added `bench --bd --short --fast-only` filtering over the typed allocation breakdown projection.
- Added `bench delta` with `diff` as an alias; both accept the same `--bd`, `--short`, and `--fast-only` report-rendering flags.
- Delta output compares the selected report with the previous matching report for the same benchmark suite and run root, and prints `missing` when a rendered scenario or span has no baseline match.
- Updated the local `ploke-egui-benchmarking` skill to default to 30-frame quick-pulse runs followed by `./target/debug/ploke-egui bench diff --bd --short --fast-only`.

## Verification

- `cargo test -p ploke-egui bench 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" bench 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`
- `cargo run -p ploke-egui --features "dev native-benchmark" -- bench --bd --short --fast-only 2>&1 | tail -n 100`
- `./target/debug/ploke-egui bench diff --bd --short --fast-only 2>&1 | tail -n 100`
- `./target/debug/ploke-egui bench delta --bd --short --fast-only 2>&1 | tail -n 20`

The focused tests passed. The CLI smoke commands rendered the latest report and
its previous matching baseline without running the native window benchmark.

## Rendered Existing Report

Current selected report:

- `crates/ploke-egui/docs/profiling/benchmarks/20260518-eframe-root-attribution-llm-alt-rerun/report.json`

Previous matching baseline selected by `bench diff`:

- `crates/ploke-egui/docs/profiling/benchmarks/20260518-eframe-root-attribution/report.json`

The filtered breakdown rendered only the fast scenario present in the latest
report, with five rows:

| scenario | median allocs/frame | median object bytes/frame | median wrapped bytes/frame | top span |
| --- | ---: | ---: | ---: | --- |
| `inspector_llm_calls_phase_sequence_alternate_30` | 1246 | 943530 | 955576 | `eframe_run_native` 248666799 object bytes |

The filtered diff for that existing report showed:

| scenario | allocs delta | object bytes delta | wrapped bytes delta | median object bytes/frame delta |
| --- | ---: | ---: | ---: | ---: |
| `inspector_llm_calls_phase_sequence_alternate_30` | +18746 | +20808641 | +20984192 | +35345 |

These values compare two existing native benchmark artifacts. They are useful
for proving the new renderer shape, but they are not evidence that this CLI
change affected native frame behavior.

## Allocation And Risk

- Baseline comparison: valid only for the existing rendered artifacts selected by the CLI; not valid as a fresh performance comparison for this code edit.
- Measured improvements: none claimed.
- Measured regressions: none claimed from this reporting-only change.
- Remaining allocation debt: the rendered latest report remains above the allocation tripwires in the fast scenario.
- Callsite attribution: not captured by the standard benchmark mode; this renderer compares span/group attribution from report and heap artifacts.
- Unmeasured risk: no new native interactive window benchmark was run for this CLI/reporting edit.
