# Clap Baseline Eval Orchestration

- date: 2026-04-15
- workstream: `A2`
- status: active
- purpose: coordinate the remaining Multi-SWE-bench Rust baseline eval coverage for `clap-rs__clap`

## Current Inventory

- completed run families on disk:
  - `BurntSushi__ripgrep` `14/14`
  - `tokio-rs__tokio` `25/25`
- incomplete family:
  - `clap-rs__clap` `0/132` completed artifacts so far

## Model / Provider

- active model: `x-ai/grok-4-fast`
- active provider: `xai` (auto-selected)

## Immediate Plan

1. run `clap-rs-all` through `ploke-eval` using the active `x-ai/grok-4-fast` / `xai` lane
2. keep the run recoverable by checking per-run completion artifacts instead of assuming the whole batch finishes cleanly
3. record any missing, mismatched, or failed runs as explicit bug candidates rather than retrying indefinitely
4. after the baseline batch, use the new protocol aggregate CLI to spot-check representative runs and confirm the artifact surface remains usable

