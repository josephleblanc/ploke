Okay, here's a structured implementation plan for the "Minimal `syn_parser` Rework" task, following your requested format.

```
# Structured Implementation Plan: syn_parser_to_cozo

## Task Overview
**Objective**: Adapt the `syn_parser` crate to directly output data compatible with CozoDB, minimizing changes to the existing data flow and concurrency model. This involves aligning types with the CozoDB schema, ensuring `Send + Sync` compliance for public types, and reducing the scope/complexity of the `CodeGraph`.
**Impact Areas**: `crates/syn_parser/src/lib.rs`, `crates/syn_parser/src/parser/mod.rs`, `crates/syn_parser/src/parser/nodes.rs`, `crates/syn_parser/src/parser/types.rs`, `crates/syn_parser/src/serialization/mod.rs`, potentially `crates/syn_parser/src/parser/visitor.rs`.
**Estimated Complexity**: Medium.  The type changes and `CodeGraph` refactoring are the most significant challenges.

## Feature Flag Strategy
- Feature name: `impl_syn_parser_to_cozo`
- When to use: Apply this feature flag if refactoring the `CodeGraph` or changing core data structures breaks existing tests or functionality. It will allow us to incrementally implement the changes.
- Usage example:
  ```rust
  #[cfg(feature = "impl_syn_parser_to_cozo")]
  pub fn new_function() { /* implementation */ }
  ```

## Subtasks

### Phase 1: Analysis (Estimated: 1 day)
- [ ] 1.1. [Review existing implementation] - Thoroughly understand the current data flow within `syn_parser`, focusing on how the AST is traversed and the `CodeGraph` is populated.
- [ ] 1.2. [Identify affected components] - Pinpoint all types and functions that will need modification to align with CozoDB's types and `Send + Sync` requirements.  Specifically, identify where the `CodeGraph` is used and whether its removal is feasible without significant disruption.

### Phase 2: Implementation (Estimated: 3-5 days)
- [ ] 2.1. [Type Alignment] - Modify the types in `nodes.rs` and `types.rs` to match CozoDB's types.
  - [ ] 2.1.1. [Replace `String` with `Bytes` where appropriate] -  For content hashes, code snippets, or other binary data currently stored as `String`, switch to `Bytes`.
  - [ ] 2.1.2. [Adjust numeric types] - Ensure numeric types (e.g., for line numbers, column numbers) are appropriate for CozoDB (likely `i64` for integers, `f64` for floats).
  - [ ] 2.1.3. [Handle UUIDs] - Use the `Uuid` type from a suitable crate (e.g., `uuid`) for UUIDs.
- [ ] 2.2. [Send + Sync Compliance] - Ensure all publicly exposed types (those in `lib.rs` and potentially others) implement `Send + Sync`. This may involve adding these traits as bounds to generic parameters or using appropriate synchronization primitives if necessary.
- [ ] 2.3. [CodeGraph Refactoring] - Reduce the scope of the `CodeGraph` or remove it entirely.
  - [ ] 2.3.1. [Evaluate streaming approach] - Investigate whether data can be streamed directly from the `syn` AST visitor to CozoDB without the need for an intermediary graph structure.
  - [ ] 2.3.2. [If retaining CodeGraph, simplify] - If the `CodeGraph` is retained, identify and remove unnecessary fields or complexity.  Focus on only storing essential information needed for CozoDB integration.
- [ ] 2.4. [Serialization Adjustment] - Update the `serialization` module to output data in a format directly consumable by CozoDB (e.g., a stream of records).

### Phase 3: Testing & Validation (Estimated: 2 days)
- [ ] 3.1. [Create tests for new functionality] - Write unit tests to verify that the type conversions and data output are correct.  Focus on testing edge cases and potential data loss scenarios.
- [ ] 3.2. [Verify existing functionality works] - Run the existing test suite to ensure that the changes haven't introduced any regressions.

## Dependencies
- Subtask 2.1 depends on completing 1.2.
- Subtask 2.3 depends on completing 1.2.
- Subtask 2.4 depends on completing 2.1 and 2.3.

## Implementation Notes
- We will prioritize minimizing changes to the core parsing logic. The goal is to adapt the *output* of the parser, not to rewrite the parser itself.
- The `CodeGraph` is a potential bottleneck.  We should aggressively explore the possibility of removing it in favor of a streaming approach.
- Thorough testing is crucial to ensure data integrity and prevent regressions.
- We will use the feature flag `impl_syn_parser_to_cozo` to isolate the changes and allow for incremental development.
```

Does this plan look reasonable?  Would you like me to elaborate on any specific aspect of it?  Also, would you like me to start with Phase 1 (Analysis) and ask for the contents of specific files, or do you have a different starting point in mind?
