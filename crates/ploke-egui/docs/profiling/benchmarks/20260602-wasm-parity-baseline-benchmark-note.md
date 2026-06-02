# WASM parity pre-edit baseline (2026-06-02)

## Verification surface

- Native harness: `cargo run -p ploke-egui --features "dev native-benchmark"` with `--benchmark-suite standard` and `--run-root` `STANDARD_RUN_ROOT`.
- Output: [`20260602-wasm-parity-baseline/`](20260602-wasm-parity-baseline/) (`report.json`, `README.md`).
- Regression: `cargo test -p ploke-egui --features "dev native-benchmark" benchmark_regression`.

## Gated scenarios (`BenchmarkScenario::regression_gated()`)

Standard suite plus Phase 0 instrumentation:

| Scenario | Components under gate |
|----------|----------------------|
| `graph_snapshot_replace_cold` | (cold action notes: `graph_snapshot_replace_cold_load_ns`; frame + alloc stats for 300 idle frames after replace) |
| `inspector_tool_decode_expanded_300` | `inspector_tool_decode`, `central_graph`, layout components |
| `graph_catalog_idle_300` | `graph_catalog` (nested under `run_navigation`), layout components |
| All prior `standard()` scenarios | Per-scenario `component_timings` in baseline README |

## Median / p95 snapshot (see README for full tables)

Authoritative numbers live in [`README.md`](20260602-wasm-parity-baseline/README.md). Key parity scenarios:

- `graph_snapshot_replace_cold` — cold snapshot bytes load + 300-frame window
- `inspector_tool_decode_expanded_300` — Run Records exclusive, tool args/results decoded every frame
- `graph_catalog_idle_300` — benchmark catalog placeholder in left nav

## Baseline comparison status

- **Pre-edit reference** for lanes A–F; zero-tolerance regression vs this `report.json`.
- **WASM browser timing:** not CI-gated (native harness only).

## Residual risk

- `graph_catalog` UI is a benchmark placeholder until Lane B ships real `GraphCatalog`.
- Tool decode gate measures native path; wasm `native_only` stubs remain until Lane D.
