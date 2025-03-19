PLANNED_TASK:
1.  **Minimal `syn_parser` Rework:** Focus on adapting the existing `syn_parser` to output data directly compatible with CozoDB, *without* fundamentally altering its data flow or concurrency model. This means:
    *   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
    *   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.
    *   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.


CREATE_PLAN:
# Task Breakdown: [Task Name]

## Main Task Description:
[Brief description of the overall task]

## Feature Flag: `feature_[task_name]`
Apply this feature flag to any code changes that would break compilation if not fully implemented.

Example: `#[cfg(feature = "feature_[task_name]")]`

## Subtasks:
- [ ] 1. [Description of first subtask]
- [ ] 2. [Description of second subtask]
  - [ ] 2.1. [Sub-step if needed]
  - [ ] 2.2. [Sub-step if needed]
- [ ] 3. [Description of third subtask]

## Testing Strategy:
[Brief notes on how to test this change]
```
