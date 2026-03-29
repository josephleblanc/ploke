---
date: 2026-03-28
task title: Error Diagnostic Rollout Plan
task description: Stage the rollout of the structured diagnostic and rich-error pattern established for `DiscoveryError` so other crates can adopt it incrementally without losing ergonomics.
related planning files:
  - /home/brasides/code/ploke/docs/active/agents/2026-03-28_error-diagnostic-pattern-report.md
---

## Goal
Apply the new diagnostic pattern across the codebase in a way that keeps error construction small at call sites, preserves structured debugging context, and avoids premature whole-workspace churn.

## Rollout Order
1. Stabilize the current `DiscoveryError` implementation as the reference slice.
2. Extend the pattern to the next highest-value parser-stage error family.
3. Standardize `xtask` rendering/persistence over the shared diagnostic trait, not ad hoc error-string parsing.
4. Expand the pattern to other crates only after the parser-facing ergonomics feel settled.

## Stage 1: Reference Slice Cleanup
1. Confirm `DiscoveryError` is the canonical reference for:
   - structured diagnostic trait usage
   - source path and source span
   - `#[track_caller]` emission site capture
   - forced backtrace capture
   - constructor/adaptor ergonomics
2. Document any remaining rough edges in:
   - trait naming
   - diagnostic kind naming
   - context payload shape
   - `xtask` default rendering choices

## Stage 2: Next Error Family
Prioritize one family that meets all of these:
1. It frequently blocks debugging.
2. It currently loses useful context through string flattening.
3. It has repeated construction patterns that would benefit from constructors or adapters.

Likely candidates:
1. `SynParserError` stage failures that still collapse to strings.
2. `resolve`/module-tree errors with source file context.
3. workspace/classification-adjacent errors beyond manifest read/parse.

## Stage 3: Shared Conventions
When another error family adopts the pattern, keep these conventions aligned:
1. Error type owns structured facts.
2. Constructors/helpers own metadata capture.
3. Call sites should not manually attach emission site or backtrace.
4. `xtask` derives follow-up workflow output from structured data rather than ad hoc strings.
5. Problem location and code emission site stay separate concepts.

## Stage 4: Consumer Integration
Consumers should use the shared diagnostic interface for:
1. persisted artifact payloads
2. human summary rendering
3. optional verbose/debug views

Consumers should not:
1. infer structure from formatted `Display` strings
2. duplicate parser-specific diagnostic logic locally

## Open Decisions To Revisit Later
1. Whether `debug_command` should be rendered by `xtask` from structured facts.
2. Whether backtrace and emission-site details belong in default human output or a verbose mode.
3. Whether manifest-kind naming should stay `Crate` / `WorkspaceRoot` / `AncestorWorkspace` or evolve further.
4. Whether additional provenance should come from semantic context, backtrace interpretation, or both.

## Exit Criteria For “Pattern Is Ready”
The pattern is ready for broader adoption when:
1. at least two distinct error families implement it
2. `xtask` can render both without string parsing
3. developers can construct the rich errors without large `map_err` closures
4. the persisted JSON shape feels stable enough to rely on during debugging
