# Protocol Sanity-Check Note

- date: 2026-04-15
- task title: protocol sanity-check note
- task description: capture the first sampled sanity-check results comparing `ploke-eval inspect` trace reads against persisted protocol artifacts
- related planning files: `docs/active/agents/2026-04-15_protocol-artifact-coverage-note.md`, `docs/active/agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md`

## Summary

Across the first sampled runs, the persisted
`tool_call_intent_segmentation` artifacts are broadly aligned with the coarse
shape seen through existing `ploke-eval inspect` commands.

Observed recurring pattern:

- the artifacts usually get the high-level phase structure right
- the main concern is granularity rather than outright mismatch
- retries, failed reads, and fine-grained search refinements are often
  compressed into one larger labeled phase

## Sampled Runs

### `BurntSushi__ripgrep-1367`

- Broadly aligned.
- Qualitative read:
  - locate target
  - refine search
  - validate hypothesis

### `tokio-rs__tokio-4789`

- Broadly aligned.
- Qualitative read:
  - refine search
  - inspect candidate
  - edit attempt

### `tokio-rs__tokio-5781`

- Broadly aligned.
- Qualitative read:
  - locate target
  - inspect candidate
  - edit attempt

### `BurntSushi__ripgrep-454`

- Broadly aligned.
- Qualitative read:
  - locate target
  - inspect candidate
  - validate hypothesis

### `BurntSushi__ripgrep-1642`

- Broadly aligned.
- Qualitative read:
  - broad context search
  - file inspection
  - terminal patch attempt

### `BurntSushi__ripgrep-2295`

- Broadly aligned.
- Qualitative read:
  - file read / locate target
  - inspection through candidate files
  - focused search progression

## Current Take

The present evidence does **not** support claiming strong semantic correctness
for the protocol outputs yet.

It **does** support a narrower claim:

- the persisted segmentation artifacts are not obviously detached from the trace
- they appear to be plausible coarse summaries of observed tool-call structure

## Main Risks

- coarse segmentation can hide meaningful distinctions inside one phase
- failures and retries can be compressed into otherwise “clean” labels
- single-artifact checks do not validate later protocol layers or downstream
  aggregation quality

## Recommended Next Validation Move

- keep sampling across more runs as coverage reaches full completion
- compare additional protocol artifact types when available, not only
  `tool_call_intent_segmentation`
- look specifically for cases where:
  - retries dominate a phase
  - patch attempts are weak but still grouped as coherent progress
  - multi-turn runs stress the procedure more than the current single-turn-heavy
    sample
