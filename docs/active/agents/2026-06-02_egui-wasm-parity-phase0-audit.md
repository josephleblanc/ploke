# 2026-06-02 — ploke-egui WASM parity Phase 0 audit

**Date:** 2026-06-02  
**Scope:** Phase 0a–0c + Lane P (instrumentation, regression harness, pre-edit baseline). No feature lanes A–F.  
**Plan:** `.cursor/plans/ploke-egui_wasm_parity_3d6f53b6.plan.md` (do not edit).

## cfg-gate audit (`crates/ploke-egui/src/ui/**`, `lib.rs`)

| Location | Gate | Hot-path risk | Lane |
|----------|------|---------------|------|
| `lib.rs` | `allocation`, `benchmark`, `cli`, `diagnostics`, `native`, `perf`, `run_picker` → `not(wasm32)`; `web` → `wasm32` | Module split OK (drivers only) | E, F |
| `lib.rs` | `global_allocator` + `native-benchmark` | Native harness only | P |
| `ui/app/mod.rs` | `run_picker`, snapshot fields, `emit_diagnostics`, puffin → `not(wasm32)` | **Panel-level:** `run_navigation` + `timeline` render on all targets; native-only *content* inside `render_run_navigation_panel` | C |
| `ui/app/mod.rs` | `current_run_name` wasm returns `None` | Warm (labels) | C |
| `ui/app/mod.rs` | Benchmark hooks → `all(not(wasm32), dev, native-benchmark)` | Driver only | P |
| `ui/app/shell.rs` | Tool decode caches + `render_decoded_*` → `not(wasm32)` | **Hot:** per-frame inspector when tools visible | D, A |
| `ui/app/shell.rs` | `native_only` stubs in tool args/results → `wasm32` | **Hot:** must remove in Lane D | D |
| `ui/app/shell.rs` | Puffin/text-size helpers → `not(wasm32)` | Warm/cold | D |
| `ui/dashboard/tiles.rs` | Benchmark inspector forcing → `not(wasm32)+dev+native-benchmark` | Driver only | P |

**Hot-path cfg to remove in parity lanes:** tool decode split in `shell.rs` (~3934–4004); wasm-empty `current_run_name`; native-only blocks inside `render_run_navigation_panel` (Lane C unifies panels, keeps I/O behind `cfg` inside drivers).

## Coverage matrix (edit surface → benchmark)

| Edit surface | Coverage | Pre-edit action (Lane P) |
|--------------|----------|---------------------------|
| `OperatorApp::ui` panels (top/left/timeline/central) | **COVERED** (`top_strip`, `run_navigation`, `timeline`, `central_graph`; `startup_frames_300`, `warm_idle_300`, …) | Re-baseline standard suite |
| Dashboard / inspector render | **COVERED** (`select_artifact_inspector_300`, `inspector_*_expanded_300`, `inspector_sections_sequence_30`) | Re-baseline before C/D |
| Tool decode + decoded sections (A+D) | **GAP → INSTRUMENTED** | `inspector_tool_decode_expanded_300` + component `inspector_tool_decode` |
| `graph_from_snapshot_bytes` / `replace_graph` (B) | **GAP → INSTRUMENTED** | `graph_snapshot_replace_cold` |
| `GraphCatalog` sidebar (B+C) | **GAP → INSTRUMENTED** | `graph_catalog_idle_300` + component `graph_catalog` (benchmark placeholder UI) |
| `RunCatalog` trait (C) | **Partial** (folded into `run_navigation` today) | Baseline after C |
| `ploke-records` tool_contracts (A) | Not frame-scoped until D | Gate at D via decode scenario |
| Diagnostics / `default_view` (F) | **COVERED** (`diagnostics` under `run_navigation` when rendered) | Baseline if hot path moves |
| `web.rs` / Trunk (E) | No separate metric | wasm check; perf via shared `OperatorApp` after C |

## Regression gate (Lane P)

- **Baseline dir:** `crates/ploke-egui/docs/profiling/benchmarks/20260602-wasm-parity-baseline/`
- **Gated scenarios:** `BenchmarkScenario::regression_gated()` (= `standard()` including new parity scenarios)
- **Test:** `benchmark_regression` validates committed baseline + fixture + compare harness; full numeric gate is `benchmark_regression_against_baseline` (`#[ignore]`, run after baseline refresh on same machine)
- **Tolerance:** zero on `frame_stats` median/p95, `allocation_frames.per_frame` alloc stats median/p95, heap slope, touched component median/p95 (+ alloc share)

## Commands (orchestrator gates)

```sh
cargo check -p ploke-egui --target wasm32-unknown-unknown
cargo check -p ploke-egui
cargo test -p ploke-egui
cargo test -p ploke-egui --features "dev native-benchmark" benchmark_regression -- --nocapture
```

Baseline capture:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --benchmark-suite standard \
  --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260602-wasm-parity-baseline
```

Fixture for cold load: `crates/ploke-egui/benchmark-fixtures/protocol-graph.json` (default WASM dogfood; symlink/copy from `.temp/` export). Alternate: `standard-prototype1-graph-snapshot.json` (tracked benchmark snapshot).
