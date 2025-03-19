Okay, I understand. Here's a detailed plan for aligning the types within `syn_parser` with the CozoDB schema. This is a significant undertaking, as it touches core data representation. I'll prioritize minimizing breaking changes where possible, and suggest feature flags where necessary.

```
USER TASK DESCRIPTION:
**Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).

---

**Implementation Plan:**

1.  **Analyze CozoDB Type Mapping Requirements** (Effort: 4 hours, Breaking Change: No, Dependencies: None)
    *   **Description:** Thoroughly review the `cozodb_docs_types.txt` file to understand the exact CozoDB types and their Rust equivalents.  Specifically, map `String` to `Bytes` where appropriate (e.g., content hashes), determine the best `Number` representation (Float/Int/Double), and understand how lists/vectors are handled.  Document this mapping clearly.
    *   **Commit Message:** `feat: Document CozoDB type mapping`
    *   **Reasoning:** This is the foundational step.  Without a clear mapping, the rest of the work will be inaccurate and potentially introduce data inconsistencies.

2.  **Update `Attribute` struct to use `Bytes` for attribute names and values** (Effort: 6 hours, Breaking Change: Yes - `feature_cozo_attribute_types`, Dependencies: 1)
    *   **Description:** The `Attribute` struct currently uses `String` for `name`, `args`, and `value`.  Change these to `Bytes`. This will likely require changes in how attributes are parsed and serialized. Introduce a feature flag to control this change.
    *   **Commit Message:** `feat(cozo): Use Bytes for Attribute fields (feature: cozo_attribute_types)`
    *   **Reasoning:** Attributes often represent metadata or identifiers, which are better represented as `Bytes` in CozoDB. This is a breaking change because it alters the structure of the `Attribute` type.

3.  **Update `MacroNode` and `MacroRuleNode` to use `Bytes` for pattern and expansion** (Effort: 8 hours, Breaking Change: Yes - `feature_cozo_macro_types`, Dependencies: 1)
    *   **Description:** Similar to attributes, macro patterns and expansions are often string-like data that can benefit from being stored as `Bytes`. Update the `pattern` and `expansion` fields in `MacroRuleNode` and the `rules` field in `MacroNode` to use `Bytes`. Introduce a feature flag.
    *   **Commit Message:** `feat(cozo): Use Bytes for Macro pattern and expansion (feature: cozo_macro_types)`
    *   **Reasoning:**  Macros are often used for code generation and manipulation, and their textual representation is well-suited for `Bytes`. This is a breaking change due to the type alteration.

4.  **Update `FunctionNode` and `TypeDefNode` to use appropriate types for identifiers** (Effort: 10 hours, Breaking Change: Yes - `feature_cozo_identifier_types`, Dependencies: 1, 2, 3)
    *   **Description:**  Examine the `name` fields in `FunctionNode`, `StructNode`, `EnumNode`, `TypeAliasNode`, and `UnionNode`, and `VariantNode`.  Determine if they should be `Bytes` or remain `String` based on their intended use in CozoDB.  Update accordingly. Introduce a feature flag.
    *   **Commit Message:** `feat(cozo): Update identifier types in FunctionNode and TypeDefNode (feature: cozo_identifier_types)`
    *   **Reasoning:**  Identifiers are crucial for data lookup and relationships.  Choosing the correct type here is vital for performance and consistency. This is a breaking change as it alters core data structures.

5.  **Update `TypeNode` to handle CozoDB type representations** (Effort: 8 hours, Breaking Change: Yes - `feature_cozo_type_node`, Dependencies: 1)
    *   **Description:** The `TypeNode`'s `kind` enum needs to be updated to accurately represent CozoDB types. This might involve adding new variants for `Bytes`, `Uuid`, `Json`, etc.  The `related_types` field should also be reviewed to ensure it uses the correct type IDs. Introduce a feature flag.
    *   **Commit Message:** `feat(cozo): Update TypeNode to support CozoDB types (feature: cozo_type_node)`
    *   **Reasoning:** This is a central component for representing types within the code graph, so it must accurately reflect the CozoDB schema.

6.  **Update `ParameterNode` to use appropriate types** (Effort: 6 hours, Breaking Change: Yes - `feature_cozo_parameter_types`, Dependencies: 4)
    *   **Description:**  The `type_id` field in `ParameterNode` needs to be updated to reflect the new `TypeNode` representation.  Ensure that the parameter names are handled correctly with the new type system. Introduce a feature flag.
    *   **Commit Message:** `feat(cozo): Update ParameterNode types (feature: cozo_parameter_types)`
    *   **Reasoning:** Parameters are essential for function definitions, and their types must align with the overall schema.

7.  **Serialization/Deserialization Updates** (Effort: 12 hours, Breaking Change: Yes - `feature_cozo_serialization`, Dependencies: 2, 3, 4, 5, 6)
    *   **Description:**  Modify the `ron` serialization code (and potentially other serialization formats if added) to correctly handle the new types. This includes converting `String` to `Bytes` where necessary and vice versa. Introduce a feature flag.
    *   **Commit Message:** `feat(cozo): Update serialization to handle new types (feature: cozo_serialization)`
    *   **Reasoning:** Serialization is the bridge between the in-memory representation and the CozoDB storage format.  It must be updated to ensure data integrity.

8.  **Comprehensive Testing** (Effort: 16 hours, Breaking Change: No, Dependencies: 7)
    *   **Description:**  Write extensive unit and integration tests to verify that the type changes haven't introduced any regressions.  Focus on testing the parsing, serialization, and deserialization of code with different types.  Pay special attention to edge cases and potential data loss scenarios.
    *   **Commit Message:** `test: Add comprehensive tests for CozoDB type alignment`
    *   **Reasoning:** Thorough testing is crucial to ensure the correctness and stability of the changes.

**Total Estimated Effort:** 70 hours

**General Notes:**

*   The feature flags allow for a gradual rollout of the changes and minimize the risk of breaking existing functionality.
*   Each subtask is designed to be relatively self-contained, making it easier to review and test.
*   The dependencies are based on the order in which the types need to be updated.
*   I've assumed that the existing parsing logic can be adapted to handle the new types with reasonable effort. If this proves to be incorrect, the effort estimates may need to be adjusted.
```

Let me know if you'd like me to elaborate on any of these subtasks or adjust the plan based on your specific requirements.  Also, please let me know when you're ready for me to examine the full contents of the files.
