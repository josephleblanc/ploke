# 2026-06-02 — ploke-egui WASM parity Phase N dogfood

**Date:** 2026-06-02  
**Scope:** Manual dogfood after Trunk/WASM wiring (not a plan edit).  
**Fixture (reference):** `crates/ploke-egui/benchmark-fixtures/standard-prototype1-graph-snapshot.json`

## Results (pass / fail)

| Step | Command / check | Result | Notes |
|------|-----------------|--------|-------|
| 1 | `cargo test -p ploke-egui --features dev,native-benchmark benchmark_regression` | **PASS** | `benchmark_regression` + `benchmark_regression_skips_when_standard_run_root_missing` ok; `benchmark_regression_against_baseline` ignored (orchestrator gate). |
| 2 | `env -u NO_COLOR trunk build --config crates/ploke-egui/Trunk.toml` | **PASS** | Trunk 0.21.14; distribution applied successfully (Rust warnings only). |
| 3 | `env -u NO_COLOR trunk serve … --address 127.0.0.1 --port 8080` | **PASS** | Background serve started from repo root. |
| 4 | `ss` + `curl http://127.0.0.1:8080/` | **PASS** | `127.0.0.1:8080` LISTEN (trunk); HTTP 200; HTML contains `<title>Ploke Operator UI</title>` and `<canvas id="ploke-operator-canvas">`. |
| 5 | Browser MCP (navigate, console, canvas/title) | **SKIP** | Browser MCP tools not available in the dogfood subagent session; static HTML verification via curl only (no JS console / WASM runtime check). |
| 6 | Stop trunk server | **PASS** | Server stopped; port 8080 no longer listening. |

## Follow-ups

- Re-run step 5 from an environment with **cursor-ide-browser** (or manual browser) to confirm WASM init, egui canvas paint, and console errors after `TrunkApplicationStarted`.
- Graph load (fixed 2026-06-02): Trunk `copy-dir` ships `benchmark-fixtures/`; startup auto-fetches `/benchmark-fixtures/standard-prototype1-graph-snapshot.json` when no `?graph=` is set. Working URL: `http://127.0.0.1:8080/?graph=/benchmark-fixtures/standard-prototype1-graph-snapshot.json`. File picker: left nav → **Load graph snapshot (.json)…**

## Related

- Phase 0 audit: `docs/active/agents/2026-06-02_egui-wasm-parity-phase0-audit.md`
- Baseline: `crates/ploke-egui/docs/profiling/benchmarks/20260602-wasm-parity-baseline/`
