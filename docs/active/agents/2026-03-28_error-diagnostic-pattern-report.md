---
date: 2026-03-28
task title: Structured Error Diagnostic Pattern Report
task description: Formalize the error-handling and diagnostic pattern developed around `DiscoveryError` so it can be reused across parser, xtask, and adjacent crates.
related planning files:
  - /home/brasides/code/ploke/docs/active/agents/2026-03-28_error-diagnostic-rollout-plan.md
---

## Purpose
This report captures the error-handling pattern established while improving `xtask parse debug corpus` and `syn_parser` manifest/classification diagnostics.

The intent is to preserve the useful parts of the recent work as a reusable design:
1. rich errors should carry structured debugging facts
2. constructors/helpers should keep call sites ergonomic
3. CLI tooling should render structured facts instead of scraping error strings

## Problem We Were Solving
Before this work, broad corpus/debug runs surfaced failures, but the debugging loop was weak:
1. useful context was often flattened into `Display` strings
2. stage and classification boundaries did not consistently preserve structured source information
3. `xtask` had to rely on coarse summaries instead of real diagnostic fields
4. improving output risked duplicating parser knowledge inside `xtask`

The result was workable for humans reading logs, but weak as a durable debugging workflow.

## Desired Outcome
We want errors to support a debugging workflow with these properties:
1. the error type can tell us what failed
2. the error can point to the relevant input/source location when one exists
3. the error can point to the code location that emitted it
4. the error can preserve a backtrace for deeper investigation
5. the call site that creates the error should stay small and readable
6. downstream tools such as `xtask` should consume structured diagnostics directly

## Pattern Overview
The pattern has four layers.

### 1. Semantic Error Type
The domain error enum should remain the source of truth for failure semantics.

For the concrete slice we just implemented, that means `DiscoveryError` remains the semantic error type. The enum stores the domain-relevant data for the failure, not only a formatted message.

### 2. Shared Structured Diagnostic Interface
Errors that can participate in richer debugging should implement the shared diagnostic trait from `ploke-error`.

The trait is the contract that downstream tooling consumes. It should expose:
1. diagnostic kind
2. summary
3. optional detail
4. optional source path
5. optional source span
6. optional emission site
7. optional backtrace
8. structured context fields

This keeps rendering logic out of the parser crates while still making the data available.

### 3. Constructors/Helpers Own Metadata Capture
Rich metadata capture should happen inside constructors or helper adapters, not at every call site.

That includes:
1. `#[track_caller]` emission-site capture
2. backtrace capture
3. source-span derivation where possible
4. normalization of repeated context handling

The call site should express the failed operation, not the mechanics of diagnostic assembly.

### 4. Tooling Consumes Structured Facts
`xtask` and related tools should persist and render the structured diagnostic fields instead of reverse-engineering meaning from formatted strings.

This keeps the parser crates responsible for semantics and keeps `xtask` responsible for workflow and presentation.

## Concrete Reference Slice
The current reference implementation is the manifest/classification path around `DiscoveryError`.

It demonstrates:
1. structured diagnostics flowing from `syn_parser` into persisted corpus artifacts
2. `source_path` and `source_span` for TOML parse/manifest problems
3. emission-site capture through `#[track_caller]`
4. forced backtrace capture for debug-oriented failures
5. `xtask` rendering the structured fields in human output and JSON artifacts

This slice should be treated as the reference example when extending the pattern elsewhere.

## Design Principles

### Preserve Structure Across Boundaries
Do not flatten a rich lower-level error into a string if the caller still benefits from structured facts.

When a higher-level error wrapper is needed, it should preserve the lower-level structured payload where practical.

### Keep Problem Location Separate From Emission Site
There are two different kinds of location data:
1. problem location
   - the input/source file and optional span that caused the error
2. emission site
   - the place in our code where the diagnostic was constructed

These should remain distinct. They answer different debugging questions.

### Prefer Constructors Over Raw Variant Construction
Direct variant construction becomes brittle once errors carry:
1. source spans
2. caller provenance
3. backtraces
4. normalized context

Constructors and helper adapters keep this manageable and reduce code duplication.

### Keep Call Sites Operation-Oriented
The code emitting the error should stay close to:
1. what operation failed
2. which path/content was involved

It should not have to manually assemble every diagnostic field.

### Keep CLI Policy Out Of Library Errors
Library/shared errors should describe the failure. `xtask` should decide:
1. how to format it
2. which detail belongs in human output
3. what narrowed debug workflow to suggest next

This is why ideas like `debug_command` belong in `xtask`, not in the shared error crate.

## Ergonomic Construction Pattern
The current design direction favors:
1. a context type carrying repeated local facts
2. a small adapter API over `Result`
3. operation-specific terminal methods such as `.for_read()` / `.for_toml()`

This pattern keeps repeated `map_err(|err| ...)` closures from spreading across the codebase while preserving explicit operations at the call site.

The core ergonomics target is:
1. one or two small context values in scope
2. one helper chain to map foreign errors into the domain error
3. rich metadata captured automatically underneath

## Manifest Error Normalization
One key design choice from this work was to normalize manifest-related errors around the failed operation rather than around partially overlapping historical variants.

That means favoring:
1. `ManifestRead`
2. `ManifestParse`

with structured fields such as:
1. manifest kind
2. manifest path
3. crate path when relevant
4. source span for parse failures
5. emission site
6. backtrace
7. source error

This lets operation-specific helpers remain unambiguous and keeps the diagnostic shape more uniform.

## Backtrace And Caller Provenance
Two explicit choices were made here.

### `#[track_caller]`
Use `#[track_caller]` to capture the code location that emitted the error. This is especially useful when debugging parser internals.

Important constraint:
If helper/adapter layers sit between production code and the constructor, the tracked boundary must propagate through those helpers. Otherwise the captured site regresses to the helper layer instead of the real production caller.

### Forced Backtrace Capture
For this debugging-oriented path, backtraces are being treated as non-negotiable for now.

In practice that means:
1. capture them automatically
2. keep them available in structured diagnostic data
3. decide later how much should be shown by default in human output

The current stable toolchain does not support the `thiserror` `#[backtrace]` attribute in the way we would want, so backtrace capture is handled manually instead of relying on that attribute.

## Diagnostic Context
Diagnostic context should hold small structured fields that help explain the scenario without collapsing into prose.

Good context fields are:
1. manifest kind
2. manifest path
3. crate path when the failing manifest was discovered from a crate
4. other local facts needed to disambiguate the scenario

Context should support debugging, not replace the semantic error variant.

## What Belongs In `xtask`
`xtask` should own:
1. persistence of structured diagnostic payloads into run artifacts
2. human-facing summary formatting
3. compact vs verbose presentation choices
4. workflow-specific suggestions such as a narrowed debug command

`xtask` should not own:
1. parser-specific semantic interpretation that could live with the error type
2. parsing structured meaning back out of `Display` strings

## What To Avoid
These patterns are likely to rot or duplicate logic:
1. flattening rich errors into strings too early
2. manually attaching provenance fields at each call site
3. storing only formatted text when structured data is available
4. letting helper layers accidentally hide the real emission site
5. overusing free-form string metadata such as ad hoc operation/origin labels when structured fields or code provenance can do the job

## Recommended Adoption Checklist
When applying this pattern to another error family, use this checklist.

1. Confirm the error family benefits from structured debugging data.
2. Identify the core semantic error type that should remain the source of truth.
3. Preserve structured lower-level errors instead of flattening them if the data is still useful.
4. Implement the shared diagnostic trait where the information is available.
5. Add constructors/helpers so callers do not manually assemble metadata.
6. Capture caller provenance at the right boundary.
7. Capture backtraces automatically if this path is part of the debug workflow.
8. Persist and render the structured data in `xtask` without string scraping.
9. Add focused tests that verify at least:
   - source path
   - source span when available
   - emission site
   - structured persistence

## Recommended Next Targets
The next error family should be chosen by debugging value, not by abstract completeness.

Good candidates are:
1. parser-stage errors that still lose structure when lifted into `SynParserError`
2. resolve/module-tree errors with strong source-file context
3. other `xtask parse debug` failure paths that currently still collapse to strings

## Bottom Line
The reusable pattern is:
1. semantic error owns structured facts
2. shared diagnostic trait exposes those facts
3. constructors/helpers capture provenance and backtrace automatically
4. tooling consumes structured diagnostics directly

If we keep those four rules intact, we can expand error quality across the workspace without making call sites noisy or pushing parser semantics into `xtask`.
