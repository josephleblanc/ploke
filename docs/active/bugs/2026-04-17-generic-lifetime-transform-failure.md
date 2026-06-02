# Bug: parsed-workspace transform fails on `generic_lifetime` relation writes

- date: 2026-04-17
- status: active
- crate affected: `syn_parser` / transform / DB write path
- severity: high

## Summary

Current eval runs are failing after parse but before usable indexing state with:

`Failed to transform parsed workspace: Database operation failed: when executing against relation 'generic_lifetime'`

This failure currently appears in the `rust-baseline-grok4-xai` campaign on:

- `nushell__nushell-11493`
- `nushell__nushell-11672`
- `nushell__nushell-11948`
- `nushell__nushell-12118`
- `serde-rs__serde-2709`
- `serde-rs__serde-2798`

The symptom is stable across both `nushell` and `serde`, but the current
artifacts do not prove whether the root cause is parser output, transform
logic, or relation/schema assumptions in the DB write layer.

## Evidence

- [nushell__nushell-11493 parse-failure.json](/home/brasides/.ploke-eval/runs/nushell__nushell-11493/parse-failure.json)
- [nushell__nushell-11672 parse-failure.json](/home/brasides/.ploke-eval/runs/nushell__nushell-11672/parse-failure.json)
- [nushell__nushell-11948 parse-failure.json](/home/brasides/.ploke-eval/runs/nushell__nushell-11948/parse-failure.json)
- [nushell__nushell-12118 parse-failure.json](/home/brasides/.ploke-eval/runs/nushell__nushell-12118/parse-failure.json)
- [serde-rs__serde-2709 parse-failure.json](/home/brasides/.ploke-eval/runs/serde-rs__serde-2709/parse-failure.json)
- [serde-rs__serde-2798 parse-failure.json](/home/brasides/.ploke-eval/runs/serde-rs__serde-2798/parse-failure.json)

All six artifacts report the same `generic_lifetime` relation failure during
parsed-workspace transform.

## Why This Matters

- The graph build aborts before evals can use semantic tools.
- The issue crosses repo families, so it is unlikely to be a single-target
  quirk.
- There was no dedicated active bug or limitation note for this failure family
  before this audit.

## Suggested Follow-Up

- Capture the minimal parsed-workspace input that triggers the failing
  `generic_lifetime` write.
- Determine whether the failure is caused by parser data shape, transform
  duplication, or DB schema/constraint assumptions.
- Once the real cause is known, decide whether it belongs in a parser known
  limitation, a transform bug, or both.
