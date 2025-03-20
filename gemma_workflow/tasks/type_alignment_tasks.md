---

**Linked Subtask List:**

*   [Subtask ID: 1 - Analyze and Map Types](gemma_workflow/tasks/type_alignment_tasks_subtask1.md)
*   Subtask ID: 2
*   Subtask ID: 3
*   [Subtask ID: 4 - Replace TypeId with CozoDbType](gemma_workflow/tasks/type_alignment_tasks_subtask4.md)
*   Subtask ID: 5
*   Subtask ID: 6
*   Subtask ID: 7
*   Subtask ID: 8

---

*   **Subtask ID: 1**
    *   Description: Analyze `syn_parser`'s existing types (e.g., `FunctionNode`, `StructNode`, `TypeId`) and map them to corresponding CozoDB types (Null, Bool, Number, String, Bytes, Uuid, Json, Vector). Document the mapping decisions, specifically focusing on how to best represent each `TypeKind` variant in CozoDB. Consider the implications of using `Bytes` for identifiers (function names, struct names, etc.) – potential performance impacts and string comparison strategies.
    *   Estimated Time: 6 hours
    *   Cfg Flag Required?: No
    *   Dependencies: None
    *   Potential Issues: Determining the best CozoDB type for complex Rust types (e.g., generics, enums). Handling potential data loss during type conversion. Performance implications of using `Bytes` for identifiers.
    *   Context: This task lays the foundation for all subsequent type alignment work. A clear and accurate mapping is crucial for ensuring data integrity and compatibility with CozoDB.
    *   Files to Modify: None
*   **Subtask ID: 2**
    *   Description: Modify `FunctionNode` to use `Bytes` for `name` instead of `String`. Deprecate the `name: String` field and add a new `name: Bytes` field (behind the `cozo_type_refactor` feature flag).
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask 1
    *   Potential Issues: Impact on existing tests that rely on `String` for function names.
    *   Context:  Aligning function names with the CozoDB `Bytes` type for efficient storage and retrieval.
    *   Files to Modify: crates/syn_parser/src/parser/nodes.rs
*   **Subtask ID: 3**
    *   Description: Modify `StructNode` and `EnumNode` to use `Bytes` for `name` instead of `String`. Deprecate the `name: String` field and add a new `name: Bytes` field (behind the `cozo_type_refactor` feature flag).
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask 1
    *   Potential Issues: Impact on existing tests that rely on `String` for struct/enum names.
    *   Context: Aligning struct and enum names with the CozoDB `Bytes` type.
    *   Files to Modify: crates/syn_parser/src/parser/nodes.rs
*   **Subtask ID: 4**
    *   Description: Replace `TypeId` with a new enum, `CozoDbType`, that explicitly represents the CozoDB types (Null, Bool, Number, String, Bytes, Uuid, Json, Vector). Define the `CozoDbType` enum and update all relevant code to use it.
    *   Estimated Time: 12 hours
    *   Cfg Flag Required?: Yes - This change fundamentally alters how types are represented and could break existing code that relies on the previous `TypeId` definition. A `cfg` flag allows us to revert easily if necessary.
    *   Dependencies: Subtask 1
    *   Potential Issues: Complexity of representing nested types and generics within the `CozoDbType` enum. Ensuring the enum is expressive enough to capture all relevant type information.
    *   Context: This is a core change that will enable us to store type information in a way that is compatible with CozoDB.
    *   Rollback Plan: Disable the `cozo_type_refactor` feature flag to revert to the original `TypeId` implementation.
    *   Files to Modify: crates/syn_parser/src/parser/types.rs, crates/syn_parser/src/parser/nodes.rs, crates/syn_parser/src/parser/visitor.rs
        *   **Subtask ID: 4.1:** Define the `CozoDbType` enum in `syn_parser/src/parser/types.rs`.
        *   **Subtask ID: 4.2:** Update all instances of `TypeId` in `syn_parser/src/parser/nodes.rs` to use `CozoDbType`.
        *   **Subtask ID: 4.3:** Modify the `VisitorState` in `syn_parser/src/parser/visitor.rs` to use `CozoDbType` when creating new nodes.
        *   **Subtask ID: 4.4:** Update the `type_map` in `VisitorState` to store `CozoDbType` instead of `TypeId`.
        *   **Subtask ID: 4.5:** Update `process_type` function.
        *   **Subtask ID: 4.6:** Update `get_or_create_type` function.
        *   **Subtask ID: 4.7:** Handle Generic Types.
        *   **Subtask ID: 4.8:** Update Type Graph.
*   **Subtask ID: 5**
    *   Description: Update `FieldNode` to use the new `CozoDbType` enum for its `type_id` field. Deprecate the `type_id: TypeId` field and add a new `type_id: CozoDbType` field (behind the `cozo_type_refactor` feature flag).
    *   Estimated Time: 3 hours
    *   Cfg Flag Required?: Yes - Dependent on Subtask 4, and carries the same risk of breaking existing code.
    *   Dependencies: Subtask 4
    *   Potential Issues: Ensuring consistency between field types and their corresponding CozoDB types.
    *   Context: Aligning field types with the CozoDB type system.
    *   Files to Modify: crates/syn_parser/src/parser/nodes.rs, crates/syn_parser/src/parser/visitor.rs
*   **Subtask ID: 6**
    *   Description: Update `ParameterNode` to use the new `CozoDbType` enum for its `type_id` field. Deprecate the `type_id: TypeId` field and add a new `type_id: CozoDbType` field (behind the `cozo_type_refactor` feature flag).
    *   Estimated Time: 3 hours
    *   Cfg Flag Required?: Yes - Dependent on Subtask 4, and carries the same risk of breaking existing code.
    *   Dependencies: Subtask 4
    *   Potential Issues: Ensuring consistency between parameter types and their corresponding CozoDB types.
    *   Context: Aligning parameter types with the CozoDB type system.
    *   Files to Modify: crates/syn_parser/src/parser/nodes.rs, crates/syn_parser/src/parser/visitor.rs
*   **Subtask ID: 7**
    *   Description: Modify `Attribute` to store its `value` as an `Option<Bytes>`. For now, we will store all attribute values as `String`.
    *   Estimated Time: 4 hours
    *   Cfg Flag Required?: No
    *   Dependencies: None
    *   Potential Issues: None
    *   Files to Modify: crates/syn_parser/src/parser/nodes.rs
*   **Subtask ID: 8**
    *   Description: Add unit tests to verify the changes made in the previous tasks.
    *   Estimated Time: 4 hours
    *   Cfg Flag Required?: No
    *   Dependencies: All previous tasks
    *   Potential Issues: Ensuring comprehensive test coverage.
    *   Files to Modify: crates/syn_parser/tests/parser/mod.rs
