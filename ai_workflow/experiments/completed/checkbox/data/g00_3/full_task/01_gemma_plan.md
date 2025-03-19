Okay, here's a detailed plan for implementing the `syn_parser` rework, formatted as requested.  The reasoning behind this approach is to prioritize incremental changes, focusing on type safety and reducing complexity while minimizing the risk of breaking existing functionality.  We'll start with foundational work (type analysis, `Send + Sync`) and then progressively move towards removing the `CodeGraph` and implementing direct streaming.

```
1. **Analyze CozoDB Types & `syn_parser` Mismatch**
   *   **Description:** Thoroughly examine the CozoDB schema and identify all type discrepancies with the current `syn_parser` types. Document these mismatches (e.g., `String` vs. `Bytes`, `usize` vs. `Int`).
   *   **Effort:** 4 hours
   *   **Breaking Change?** No
   *   **Commit Message:** `feat: Analyze CozoDB type compatibility`
   *   **Dependencies:** None
   *   **Reasoning:** This is the essential first step.  Understanding the type differences is crucial for making informed decisions about conversions and adaptations.

2. **Implement `Send + Sync` for Core Data Structures (Phase 1)**
   *   **Description:** Add `Send + Sync` bounds to publicly exposed structs in `parser/nodes.rs` (e.g., `FunctionNode`, `StructNode`, `EnumNode`, `TraitNode`).  Initially, focus on adding the bounds; address any compilation errors related to non-`Send + Sync` types in subsequent subtasks.
   *   **Effort:** 6 hours
   *   **Breaking Change?** Potentially (if public API changes)
   *   **Feature Flag:** `feature_send_sync_fixes`
   *   **Commit Message:** `feat: Implement Send + Sync for core data structures (Phase 1)`
   *   **Dependencies:** 1
   *   **Reasoning:**  Ensuring `Send + Sync` compliance is a good practice for thread safety and prepares the codebase for potential future concurrency.  Doing this early minimizes the risk of larger refactorings later.

3. **Refactor `Attribute` struct to use `String` for arguments and values**
   *   **Description:** Modify the `Attribute` struct in `parser/nodes.rs` to use `String` instead of `Vec<String>` for `args` and `Option<String>` for `value`.
   *   **Effort:** 2 hours
   *   **Breaking Change?** No
   *   **Commit Message:** `refactor: Standardize Attribute struct`
   *   **Dependencies:** None
   *   **Reasoning:** Simplifies the `Attribute` struct and aligns it with CozoDB's string type.

4. **Initial `CodeGraph` Reduction - Remove `defined_types` and `type_graph`**
   *   **Description:** Remove the `defined_types` and `type_graph` fields from the `CodeGraph` struct in `parser/graph.rs`.
   *   **Effort:** 3 hours
   *   **Breaking Change?** No
   *   **Commit Message:** `refactor: Remove redundant type information from CodeGraph`
   *   **Dependencies:** None
   *   **Reasoning:** These fields appear redundant given the goal of direct streaming to CozoDB, simplifying the `CodeGraph`.

5. **Implement `Send + Sync` for Core Data Structures (Phase 2)**
   *   **Description:** Address any compilation errors arising from adding `Send + Sync` bounds in Subtask 2. This may involve wrapping types in `Arc` or `Mutex` where necessary. Prioritize avoiding unnecessary copying.
   *   **Effort:** 4 hours
   *   **Breaking Change?** Potentially (if public API changes)
   *   **Feature Flag:** `feature_send_sync_fixes`
   *   **Commit Message:** `feat: Implement Send + Sync for core data structures (Phase 2)`
   *   **Dependencies:** 2
   *   **Reasoning:** Completes the `Send + Sync` implementation, resolving any issues identified in the initial phase.

6. **Begin Direct Streaming - Function Node to CozoDB Representation**
   *   **Description:** Modify the `VisitorState` in `parser/visitor.rs` to, instead of creating a `FunctionNode`, directly construct a representation suitable for insertion into CozoDB. This involves mapping `syn` elements to CozoDB types.
   *   **Effort:** 10 hours
   *   **Breaking Change?** Potentially (if the structure of `FunctionNode` is exposed publicly)
   *   **Feature Flag:** `feature_direct_streaming`
   *   **Commit Message:** `feat: Begin direct streaming - FunctionNode to CozoDB`
   *   **Dependencies:** 1, 3, 4
   *   **Reasoning:** This is the first step towards eliminating the `CodeGraph` as an intermediary.  Focusing on `FunctionNode` first provides a manageable starting point.

7. **Adapt `String` to `Bytes` for Function Data**
   *   **Description:** Within the direct streaming implementation for `FunctionNode` (from Subtask 6), convert any `String` fields (e.g., docstrings, function body) to `Bytes` before storing them in the CozoDB representation.
   *   **Effort:** 3 hours
   *   **Breaking Change?** No
   *   **Commit Message:** `feat: Convert String to Bytes for function data`
   *   **Dependencies:** 6
   *   **Reasoning:** Aligns the data with CozoDB's `Bytes` type for content.

8. **Continue Direct Streaming - Struct/Enum/Trait Nodes**
   *   **Description:** Extend the direct streaming approach to `StructNode`, `EnumNode`, and `TraitNode` in `parser/visitor.rs`.
   *   **Effort:** 14 hours
   *   **Breaking Change?** Potentially (if the structure of these nodes is exposed publicly)
   *   **Feature Flag:** `feature_direct_streaming`
   *   **Commit Message:** `feat: Continue direct streaming - Struct/Enum/Trait Nodes`
   *   **Dependencies:** 1, 3, 4, 6
   *   **Reasoning:** Completes the direct streaming implementation for the core code structure elements.

9. **Adapt `String` to `Bytes` for Struct/Enum/Trait Data**
   *   **Description:** Within the direct streaming implementation for `StructNode`, `EnumNode`, and `TraitNode` (from Subtask 8), convert any `String` fields (e.g., docstrings, field names) to `Bytes` before storing them in the CozoDB representation.
   *   **Effort:** 4 hours
   *   **Breaking Change?** No
   *   **Commit Message:** `feat: Convert String to Bytes for struct/enum/trait data`
   *   **Dependencies:** 8
   *   **Reasoning:** Aligns the data with CozoDB's `Bytes` type for content.

10. **Final `CodeGraph` Reduction - Remove Remaining Fields**
    *   **Description:** Remove the remaining fields (e.g., `functions`, `impls`, `traits`) from the `CodeGraph` struct in `parser/graph.rs`.
    *   **Effort:** 2 hours
    *   **Breaking Change?** No
    *   **Commit Message:** `refactor: Final CodeGraph reduction`
    *   **Dependencies:** 8, 9
    *   **Reasoning:** Completes the elimination of the `CodeGraph` as an intermediary.
```

Total estimated effort: 56 hours.

Let me know if you'd like me to refine this plan further or if you're ready to proceed with the first subtask.
