PLANNED_TASK:
1.  **Minimal `syn_parser` Rework:** Focus on adapting the existing `syn_parser` to output data directly compatible with CozoDB, *without* fundamentally altering its data flow or concurrency model. This means:
    *   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
    *   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.
    *   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.


Task Name: Type Alignment


CREATE_PLAN:
# Structured Implementation Plan: [Task Name]

## Task Overview
**Objective**: [Brief description of what needs to be accomplished]
**Impact Areas**: [Files/modules that will be affected]
**Estimated Complexity**: [Low/Medium/High]

## Feature Flag Strategy
- Feature name: `impl_[task_name]`
- When to use: Apply feature flag ONLY when changes would break the codebase without completing all related subtasks
- Usage example: 
  ```rust
  #[cfg(feature = "impl_[task_name]")]
  pub fn new_function() { /* implementation */ }
  ```

## Subtasks

### Phase 1: Analysis
- [ ] 1.1. [Review existing implementation]
- [ ] 1.2. [Identify affected components]

### Phase 2: Implementation
- [ ] 2.1. [First implementation step]
  - [ ] 2.1.1. [Implementation detail]
  - [ ] 2.1.2. [Implementation detail]
- [ ] 2.2. [Second implementation step]

### Phase 3: Testing & Validation
- [ ] 3.1. [Create tests for new functionality]
- [ ] 3.2. [Verify existing functionality works]

## Dependencies
- Subtask 2.1 depends on completing 1.2
- [Other dependency relationships]

## Implementation Notes
[Reasoning behind key design decisions]
```
