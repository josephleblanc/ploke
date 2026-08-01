# 2026-05-17 Run Readiness Heuristic Benchmark Note

not tested on the native interactive window; verification was focused benchmark
unit tests and `cargo check`.

## Change Summary

- Added a standard-suite preflight that reads `scheduler.json`, `nodes/*/node.json`,
  and sealed History blocks through typed Rust records.
- The preflight accepts the run root when sealed History reaches
  `policy.max_generations`, or when spawned child node records reach the minimum
  implied by `policy.child_budget.min * policy.max_generations`.
- Added the heuristic to v3 benchmark reports and generated READMEs.

## Verification

- `cargo test -p ploke-egui benchmark_standard_run_readiness_matches_persisted_records 2>&1 | tail -n 120`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark 2>&1 | tail -n 120`
- `cargo check -p ploke-egui --features "dev native-benchmark" 2>&1 | tail -n 120`

## Baseline

No native benchmark baseline comparison is valid for this note. The prior full
native v3 run was intentionally stopped because per-allocation callsite
attribution made the standard suite too slow to use as a verification surface.

## Measured Changes

- Standard run-root readiness test passed against the real
  `/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1`
  records: 6 sealed History blocks with max block height 5 for
  `max_generations = 5`.
- No frame timing or heap attribution improvement was measured.

## Residual Risk

This is a heuristic, not loop authority. It only gates the native benchmark
against obviously incomplete persisted records while a more precise run-control
completion model is handled elsewhere.
