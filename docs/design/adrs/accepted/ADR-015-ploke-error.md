# ADR-015: Create common `ploke-error` crate

## Status
ACCEPTED 2025-05-11

## Context
Error types across crates are disorganized, and do not share a common organizational structure. Using error types across crates is becoming more important as testing requires using functions from other crates in the workspace, and using crate-local error types is messy. A common error type for the workspace will normalize errors and provide a clear indication of the severity of the error.

## Decision
1.  Added new crate, `ploke-error`, to hold common error types.
2.  Used `thiserror` for ease of organization and conversion across error types.
3.  Started designing error categories:
  - `Fatal`: Abort processing (e.g. to prevent insertion of invalid state to graph)
  - `Warning`: Recoverable errors (e.g. upon processing files for graph not found in module tree)
  - `Internal`: Invalid state errors, should panic (e.g. bugs, invariants violated)

## Consequences
- **Positive:**
    *   Unifies organization of errors across crates, provides a shared resource that dictates error structures in the crates.
    *   Clearly defined categories of errors allow better design of error handling. Less boilerplate for handling response to various error types.
    *   Allows for unified strategy of reporting errors in the future.
- **Negative:**
    *   All crates now must use the `ploke-error` dependency. This should remain minimal but still represents and additional dependency.
    *   May constrain the options available to error handling, though this should be limited as far as possible.
- **Neutral:**
    *   Final module path/ID resolution remains a Phase 3 responsibility.
    *   Respects Phase 2 parallel constraints (no cross-worker communication).

## Compliance
- Aligns with planned structure for error handling ([`PROPOSED_ARCH_V3.md`](PROPOSED_ARCH_V3.md)).
- Improves error handling (C-GOOD-ERR) ([`IDIOMATIC_RUST.md`](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md)).

Accepted git tag: 
