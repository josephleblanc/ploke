# Native Benchmarks

This directory holds small tracked benchmark summaries produced by the native
`ploke-egui` benchmark suite. Each benchmark run writes:

- `README.md`
- `report.json`

Short Markdown `*-benchmark-note.md` files are allowed for docs-only, test-only,
or small edits where the native benchmark is not run. Each note must name the
exact verification surface, baseline comparison status, performance increases,
regressions, and residual risk.

## Notes

- [`20260517-run-readiness-heuristic-benchmark-note.md`](20260517-run-readiness-heuristic-benchmark-note.md)
  Run-readiness heuristic benchmark note.
- [`20260518-nine-phase-selection-questions-benchmark-note.md`](20260518-nine-phase-selection-questions-benchmark-note.md)
  Nine-phase selection question benchmark note.
- [`20260518-run-records-focused-spans-benchmark-note.md`](20260518-run-records-focused-spans-benchmark-note.md)
  Run Records focused-span benchmark note.
- [`20260518-run-records-measurement-split-benchmark-note.md`](20260518-run-records-measurement-split-benchmark-note.md)
  Run Records measurement split benchmark note.
- [`20260518-run-records-phase-heap-deltas-benchmark-note.md`](20260518-run-records-phase-heap-deltas-benchmark-note.md)
  Run Records phase heap-delta benchmark note.
- [`20260518-selection-metric-witness-benchmark-note.md`](20260518-selection-metric-witness-benchmark-note.md)
  Selection metric witness benchmark note.
- [`20260518-test-triage-run-record-order-benchmark-note.md`](20260518-test-triage-run-record-order-benchmark-note.md)
  Run-record order test-triage benchmark note.
- [`20260518-tool-step-cache-benchmark-note.md`](20260518-tool-step-cache-benchmark-note.md)
  Tool-step cache benchmark note.
- [`20260519-score-child-prop-comparison-benchmark-note.md`](20260519-score-child-prop-comparison-benchmark-note.md)
  Score-child-prop comparison benchmark note.
- [`20260519-wasm-build-target-benchmark-note.md`](20260519-wasm-build-target-benchmark-note.md)
  Wasm build target benchmark note.
- [`20260520-agent-turn-playback-benchmark-note.md`](20260520-agent-turn-playback-benchmark-note.md)
  Agent-turn playback benchmark note.
- [`20260520-graph-snapshot-ui-benchmark-note.md`](20260520-graph-snapshot-ui-benchmark-note.md)
  Graph snapshot UI benchmark note.
- [`20260521-selected-graph-item-print-benchmark-note.md`](20260521-selected-graph-item-print-benchmark-note.md)
  Selected graph item JSON print benchmark note.
- [`20260525-eval-protocol-row-hover-samply-note.md`](20260525-eval-protocol-row-hover-samply-note.md)
  Eval & Protocol row-hover CPU sampling note and allocation follow-up.

Large Puffin captures stay local under
`crates/ploke-egui/data/profiling/puffin/benchmarks/` and are referenced from
`report.json` by path, byte size, and SHA-256 hash.

Standard v1 command:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --benchmark-suite standard
```

## WASM parity regression (Phase 0)

- **Pre-edit baseline:** [`20260602-wasm-parity-baseline/`](20260602-wasm-parity-baseline/) — full `standard()` suite including parity instrumentation scenarios.
- **Gated scenarios:** all names in `BenchmarkScenario::regression_gated()` (see [`20260602-wasm-parity-baseline/README.md`](20260602-wasm-parity-baseline/README.md) after baseline run).
- **Parity-only scenarios (added Phase 0):** `graph_snapshot_replace_cold`, `inspector_tool_decode_expanded_300`, `graph_catalog_idle_300`.
- **Regression tests:** `cargo test -p ploke-egui --features dev,native-benchmark benchmark_regression` (baseline + fixture + compare harness). Orchestrator numeric gate: `benchmark_regression_against_baseline` with `--ignored` after baseline refresh on the same machine (zero tolerance vs `report.json`).
- **Baseline refresh:** user-approved commit only (orchestrator).
