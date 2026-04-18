# Bug: `nushell` `nu-cli` indexing fails with duplicate `crate::commands` module paths

- date: 2026-04-17
- status: active
- crate affected: `syn_parser` / eval indexing path
- severity: high

## Summary

Current eval runs for `nushell__nushell` fail while building the module tree for
`crates/nu-cli` with:

`Duplicate definition path 'crate::commands' found in module tree`

This failure currently appears in the `rust-baseline-grok4-xai` campaign on:

- `nushell__nushell-10395`
- `nushell__nushell-12901`
- `nushell__nushell-13357`
- `nushell__nushell-13870`

The current source shape in `nu-cli` does not obviously match the existing
documented duplicate-path limitations (`KL-003` cfg-disjoint inline duplicate
modules or `KL-004` nested `main.rs` logical-path collisions), so this should
be tracked as a separate active bug until a narrower root cause is proven.

## Evidence

- [nushell__nushell-10395 parse-failure.json](/home/brasides/.ploke-eval/runs/nushell__nushell-10395/parse-failure.json)
- [nushell__nushell-12901 parse-failure.json](/home/brasides/.ploke-eval/runs/nushell__nushell-12901/parse-failure.json)
- [nushell__nushell-13357 parse-failure.json](/home/brasides/.ploke-eval/runs/nushell__nushell-13357/parse-failure.json)
- [nushell__nushell-13870 parse-failure.json](/home/brasides/.ploke-eval/runs/nushell__nushell-13870/parse-failure.json)
- [syn_parser known limitations](/home/brasides/code/ploke/docs/design/syn_parser_known_limitations.md)
- [KL-004 nested-main collision note](/home/brasides/code/ploke/docs/design/known_limitations/KL-004-nested-main-rs-logical-path.md)

All four artifacts report the same failure family under
`Failed to build module tree`.

## Why This Matters

- The affected `nushell` runs never reach a usable graph, so the semantic tool
  surface is unavailable.
- This is currently one of the three restart-relevant blocker families for
  `nushell`.
- The failure does not yet have a dedicated known-limitation entry, so it would
  be easy to rediscover expensively.

## Suggested Follow-Up

- Isolate a minimal repro from `crates/nu-cli` that preserves the
  `crate::commands` collision.
- Inspect how module-path derivation produced two competing module ids for the
  same logical path.
- Decide whether this belongs under an existing duplicate-path limitation after
  RCA or warrants a new limitation entry.
