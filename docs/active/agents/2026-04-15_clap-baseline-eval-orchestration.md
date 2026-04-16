# Clap Baseline Eval Orchestration

- date: 2026-04-15
- workstream: `A2`
- status: reference
- purpose: coordinate the remaining Multi-SWE-bench Rust baseline eval coverage for `clap-rs__clap`

> Superseded as the main control plane by [2026-04-16_eval-closure-formal-sketch.md](2026-04-16_eval-closure-formal-sketch.md) once the full local Rust slice and explicit target registry `T` landed. Keep this note for `clap`-specific baseline and failure details, not for current overall orchestration.

## Current Inventory

- completed run families on disk:
  - `BurntSushi__ripgrep` `14/14`
  - `tokio-rs__tokio` `25/25`
- `clap-rs__clap` batch state:
  - `132` attempted
  - `130` succeeded
  - `2` failed
  - known failed instances:
    - `clap-rs__clap-1624`
    - `clap-rs__clap-941`

## Current Reality

- the baseline eval batch is effectively complete
- per-run directories are the trustworthy source of truth
- the batch summary's top-level `run_arm` metadata is stale-looking and should not be used to infer that the runs were setup-only
- real `clap` agentic artifacts exist on disk, including:
  - `record.json.gz`
  - `agent-turn-summary.json`
  - `agent-turn-trace.json`
  - `llm-full-responses.jsonl`
- protocol follow-up started on `clap-rs__clap-3521`, then stopped

## Model / Provider

- active model: `x-ai/grok-4-fast`
- active provider: `xai` (explicitly pinned during the batch)

## Protocol Follow-Through State

- closed legacy protocol gaps:
  - `BurntSushi__ripgrep-727`
  - `tokio-rs__tokio-5520`
- first partial `clap` protocol run:
  - `clap-rs__clap-3521`
  - currently has:
    - `1` intent segmentation artifact
    - `2` tool-call review artifacts
    - `0` segment-review artifacts
- protocol-artifact run directories currently total `40`

## Immediate Plan

1. treat the `clap` baseline as complete except for the two explicit parse/indexing failures
2. resume protocol coverage from `clap-rs__clap-3521`
3. continue across the remaining completed `clap` runs in:
   - `tool-call-intent-segments`
   - `tool-call-review`
   - `tool-call-segment-review`
4. keep the two failed `clap` instances as explicit bug/follow-up cases rather than silently treating the baseline as `132/132`
