# TUI Adapter Implementation Wave

This directory records the 2026-05-12 implementation wave for the broad harness
headless `ploke-tui` adapter.

## Reports

- `reports/retainer.md` - source map for the existing `ploke-tui` headless app
  harness and event surfaces.
- `reports/review-run-readiness.md` - run-readiness review after the first
  implementation pass.
- `reports/review-structural-invariants.md` - structural carrier and request
  binding review.
- `reports/review-rust-style.md` - Rust correctness and async boundary review.

## Outcome

The implementation wires the broad request path through the vanilla `ploke-tui`
test harness, keeps `ploke-eval` responsible for bounded-surface admission, and
materializes admitted edits into request-bound child plans. The live
`prototype1-state` run itself remains an operator action.
