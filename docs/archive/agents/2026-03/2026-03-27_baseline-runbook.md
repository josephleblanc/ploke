---
date: 2026-03-27
task title: Baseline Ingest Runbook
task description: Draft a small recurring runbook using `cargo xtask profile-ingest` to generate comparable ingestion performance reports across stable fixture targets.
related planning files:
  - /home/brasides/code/ploke/.cursor/plans/ingestion-pipeline-comparison-overview_88f15a9b.plan.md
---

## Purpose
Generate a repeatable baseline set of ingestion timings (parse/transform and optionally embed) using `cargo xtask profile-ingest`.

The runbook is designed so that reruns produce directly comparable JSON reports written to `xtask/profiling_output/`.

## Recurrence
Run this baseline after:
1. Any change in ingestion pipeline codepaths (parser/transform/embed).
2. Any change in compilation-unit structural mask logic.
3. Periodically (e.g. daily on main, or per-PR if you are actively tuning performance).

## Comparability Rules (must be kept stable)
1. Stages: keep the stage list identical across runs (`--stages parse,transform` for the baseline; embed is optional).
2. Loops: keep `--loops` identical (baseline uses `--loops 3`).
3. Compilation-unit dimensions (union mode only): keep compilation-unit dimension env vars stable:
   - `PLOKE_CU_TARGET_TRIPLES`
   - `PLOKE_CU_PROFILES`
   - `PLOKE_CU_FEATURE_SETS` (or `PLOKE_CU_FEATURES`)

   If you do not intentionally change CU dimensions, use the defaults by ensuring these are unset.
4. Network: do not include `embed` in baseline runs unless you explicitly want embedding timings (embed is sensitive to network and OpenRouter latency).

## Pre-flight (once per shell session)
These exports/unsets make CU-dimension behavior stable for union mode:

```bash
# Ensure the default target triple is stable if your environment might vary.
export TARGET="${TARGET:-x86_64-unknown-linux-gnu}"

# Use default compilation-unit dimensions unless you explicitly want otherwise.
unset PLOKE_CU_TARGET_TRIPLES
unset PLOKE_CU_PROFILES
unset PLOKE_CU_FEATURE_SETS
unset PLOKE_CU_FEATURES
```

Optional (debugging only):
```bash
export PLOKE_PROFILE_LOG=1
```

## Baseline Matrix (recommended)
All commands below:
- run from the ploke workspace root
- write JSON to `xtask/profiling_output/`
- use identical stages/verbosity/loops

### 1) Classic crate ingest (fixture_nodes)
```bash
cargo xtask profile-ingest \
  --target tests/fixture_crates/fixture_nodes \
  --stages parse,transform \
  --verbosity 2 \
  --loops 3
```

### 2) Classic crate ingest (fixture_unusual_lib)
```bash
cargo xtask profile-ingest \
  --target tests/fixture_crates/fixture_unusual_lib \
  --stages parse,transform \
  --verbosity 2 \
  --loops 3
```

### 3) Union+CU crate ingest (fixture_cfg_cu)
This exercises the compilation-unit union path via `--compilation-unions`:
```bash
cargo xtask profile-ingest \
  --target tests/fixture_crates/fixture_cfg_cu \
  --stages parse,transform \
  --verbosity 2 \
  --loops 3 \
  --compilation-unions
```

### 4) Workspace ingest (ws_fixture_01)
```bash
cargo xtask profile-ingest \
  --target tests/fixture_workspace/ws_fixture_01 \
  --stages parse,transform \
  --verbosity 2 \
  --loops 3
```

## Output Collection
`profile-ingest` always writes a JSON file under:
- `xtask/profiling_output/`

After each command completes, capture the printed `Wrote <path>` line.
You can also locate the newest JSON for a target by:

```bash
ls -t xtask/profiling_output/fixture_nodes_*.json | head -n 1
```

## Optional: Embed Timing Probe (not recommended for “baseline” comparability)
Only run this if you are explicitly tracking embed performance, because it depends on OpenRouter latency.

```bash
# Requires a valid OPENROUTER_API_KEY.
export OPENROUTER_API_KEY="..."

cargo xtask profile-ingest \
  --target tests/fixture_crates/fixture_nodes \
  --stages parse,transform,embed \
  --verbosity 1 \
  --loops 1
```

## What to Compare Across Runs (JSON fields)
In each report JSON, compare:
1. `statistics.global.avg_ms` / `min_ms` / `max_ms`
2. `statistics.stages.parse.avg_ms`
3. `statistics.stages.transform.avg_ms`
4. For union+CU runs, optionally also compare that the run is using the same CU dimension env state (see comparability rules).

