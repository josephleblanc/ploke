Okay, I understand. You want a more concise plan format. Here's a plan adhering to that structure, focusing on the initial phase of adapting `syn_parser` for CozoDB compatibility:

```markdown
# Task Breakdown: CozoDB Type Alignment & Send+Sync

## Main Task Description:
Adapt `syn_parser`'s data structures to align with CozoDB's data types and ensure public types are `Send + Sync`, preparing for direct data streaming.

## Feature Flag: `cozo_integration`
Apply this feature flag to any code changes that would break compilation if not fully implemented.

Example: `#[cfg(feature = "cozo_integration")]`

## Subtasks:
- [ ] 1. **CozoDB Type Review:**  Review `cozodb_docs_types.txt` to understand CozoDB's type system and identify necessary mappings from `syn` types.
- [ ] 2. **Type Alignment in `nodes.rs`:** Modify types in `src/parser/nodes.rs` to match CozoDB types (e.g., `String` -> `Bytes`, `i64` for integers).
  - [ ] 2.1. Replace `String` fields with `Bytes` where appropriate (names, content).
  - [ ] 2.2. Ensure numeric types are compatible (use `i64` for integers).
- [ ] 3. **Ensure `Send + Sync`:** Add `Send + Sync` bounds to all public structs in `src/parser/nodes.rs` and `src/parser/graph.rs`. Address any non-`Send + Sync` dependencies.
- [ ] 4. **Update Tests:** Modify existing unit and integration tests to reflect the type changes and verify `Send + Sync` compliance.

## Testing Strategy:
*   **Unit Tests:** Verify type conversions and `Send + Sync` bounds.
*   **Compile-Time Checks:** Ensure code compiles with and without the `cozo_integration` feature flag.
*   **Integration Tests:**  Confirm existing tests pass after type changes.
```

This plan is more streamlined, focusing on the core tasks and providing a clear testing strategy.  It's designed to be easily tracked and executed.  Let me know if you'd like me to adjust it further or move on to planning the next phase (reducing the scope of `CodeGraph`).
