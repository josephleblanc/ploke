# Subtask 4: Replace TypeId with CozoDbType

---

**Parent Task:** [Type Alignment Tasks](gemma_workflow/tasks/type_alignment_tasks.md)

*   **Subtask ID: 4.1**
    *   Description: Define the `CozoDbType` enum in `syn_parser/src/parser/types.rs`. Include variants for all CozoDB types (Null, Bool, Number, String, Bytes, Uuid, Json, Vector).
    *   Estimated Time: 3 hours
    *   Cfg Flag Required?: Yes - This is a fundamental change to the type system.
    *   Dependencies: Subtask ID: 1.4 (mapping table)
    *   Potential Issues: Ensuring the enum is comprehensive and accurately represents CozoDB types.
*   **Subtask ID: 4.2**
    *   Description: Update all instances of `TypeId` in `syn_parser/src/parser/nodes.rs` to use `CozoDbType`. This includes fields in `FunctionNode`, `StructNode`, `EnumNode`, `FieldNode`, `ParameterNode`, etc.
    *   Estimated Time: 4 hours
    *   Cfg Flag Required?: Yes - This is a widespread change that could introduce regressions.
    *   Dependencies: Subtask ID: 4.1
    *   Potential Issues: Missing updates in certain parts of the codebase.
*   **Subtask ID: 4.3**
    *   Description: Modify the `VisitorState` in `syn_parser/src/parser/visitor.rs` to use `CozoDbType` when creating new nodes. Update the `get_or_create_type` and `process_type` methods accordingly.
    *   Estimated Time: 3 hours
    *   Cfg Flag Required?: Yes - This change affects the core parsing logic.
    *   Dependencies: Subtask ID: 4.1, 4.2
    *   Potential Issues: Incorrect type mapping during parsing.
*   **Subtask ID: 4.4**
    *   Description: Update the `type_map` in `VisitorState` to store `CozoDbType` instead of `TypeId`.
    *   Estimated Time: 1 hour
    *   Cfg Flag Required?: Yes - Impacts the core parsing logic.
    *   Dependencies: Subtask ID: 4.3
    *   Potential Issues: Inconsistent type mapping.
*   **Subtask ID: 4.5**
    *   Description: Add unit tests to verify that the `CozoDbType` enum is correctly populated and that the type mapping is accurate.
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask ID: 4.1, 4.2, 4.3, 4.4
    *   Potential Issues: Insufficient test coverage.

---
