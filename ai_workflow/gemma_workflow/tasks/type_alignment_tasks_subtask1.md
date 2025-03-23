# Subtask 1: Analyze and Map Types

---

**Parent Task:** [Type Alignment Tasks](gemma_workflow/tasks/type_alignment_tasks.md)

*   **Subtask ID: 1.1**
    *   Description: Review the `TypeKind` enum in `syn_parser/src/parser/types.rs`. Document each variant and its potential mapping to CozoDB types (Null, Bool, Number, String, Bytes, Uuid, Json, Vector). **Ensure all code adheres to the conventions outlined in `CONVENTIONS` and `IDIOMATIC_RUST`.**
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: None
    *   Potential Issues: Ambiguity in mapping complex Rust types to CozoDB types.
*   **Subtask ID: 1.2**
    *   Description: Analyze how `FunctionNode`, `StructNode`, `EnumNode`, and other relevant AST nodes use the `TypeId` and `TypeKind` types. **Ensure all code adheres to the conventions outlined in `CONVENTIONS` and `IDIOMATIC_RUST`.**
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask ID: 1.1
    *   Potential Issues: Identifying all locations where type information is used.
*   **Subtask ID: 1.3**
    *   Description: Investigate the implications of using `Bytes` for identifiers (function names, struct names, etc.). Research potential performance impacts and string comparison strategies. **Ensure all code adheres to the conventions outlined in `CONVENTIONS` and `IDIOMATIC_RUST`.**
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask ID: 1.1
    *   Potential Issues: Performance bottlenecks related to `Bytes` comparisons.
*   **Subtask ID: 1.4**
    *   Description: Create a detailed mapping table documenting the recommended CozoDB type for each `TypeKind` variant, along with any necessary conversion logic. **Ensure all code adheres to the conventions outlined in `CONVENTIONS` and `IDIOMATIC_RUST`.** Deliverable: A Markdown table in a new file `gemma_workflow/type_mappings.md`.
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask ID: 1.1, 1.2, 1.3
    *   Potential Issues: Ensuring the mapping table is comprehensive and accurate.
*   **Subtask ID: 1.5**
    *   Description: Document any potential data loss or precision issues that may arise during type conversion. **Ensure all code adheres to the conventions outlined in `CONVENTIONS` and `IDIOMATIC_RUST`.**
    *   Estimated Time: 1 hour
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask ID: 1.4
    *   Potential Issues: Identifying subtle type conversion issues.
*   **Subtask ID: 1.6**
    *   Description: Review the documentation and examples in `PROPOSED_ARCH_V2.md` and `cozodb_docs_types.txt` to ensure the proposed type mappings align with the overall architecture and CozoDB best practices. **Ensure all code adheres to the conventions outlined in `CONVENTIONS` and `IDIOMATIC_RUST`.**
    *   Estimated Time: 1 hour
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask ID: 1.4
    *   Potential Issues: Discovering inconsistencies between the proposed mappings and the existing documentation.
*   **Subtask ID: 1.7**
    *   Description: Document CozoDB Type Limitations: Explicitly document any limitations or potential data loss when mapping Rust types to CozoDB types. **Ensure all code adheres to the conventions outlined in `CONVENTIONS` and `IDIOMATIC_RUST`.**
    *   Estimated Time: 1 hour
    *   Cfg Flag Required?: No
    *