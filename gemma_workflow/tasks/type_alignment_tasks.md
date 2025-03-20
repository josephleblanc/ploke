*   **Subtask ID: 1**
    *   Description: Analyze `syn_parser`'s existing types (e.g., `FunctionNode`, `StructNode`, `TypeId`) and map them to corresponding CozoDB types (Null, Bool, Number, String, Bytes, Uuid, Json, Vector).  Document the mapping decisions.
    *   Estimated Time: 4 hours
    *   Cfg Flag Required?: No
    *   Dependencies: None
    *   Potential Issues: Determining the best CozoDB type for complex Rust types (e.g., generics, enums).  Handling potential data loss during type conversion.
*   **Subtask ID: 2**
    *   Description: Modify `FunctionNode` to use `Bytes` for `name` instead of `String`.
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask 1
    *   Potential Issues:  Impact on existing tests that rely on `String` for function names.
*   **Subtask ID: 3**
    *   Description: Modify `StructNode` and `EnumNode` to use `Bytes` for `name` instead of `String`.
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask 1
    *   Potential Issues: Impact on existing tests that rely on `String` for struct/enum names.
*   **Subtask ID: 4**
    *   Description: Replace `TypeId` with a more specific type representing a CozoDB type.  Consider using an enum to represent the allowed CozoDB types.
    *   Estimated Time: 6 hours
    *   Cfg Flag Required?: Yes - This change fundamentally alters how types are represented and could break existing code that relies on the previous `TypeId` definition.  A `cfg` flag allows us to revert easily if necessary.
    *   Dependencies: Subtask 1
    *   Potential Issues:  Complexity of representing nested types and generics.  Ensuring the new type system is expressive enough to capture all relevant type information.
*   **Subtask ID: 5**
    *   Description: Update `FieldNode` to use the new `TypeId` type (from Subtask 4) for its `type_id` field.
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: Yes - Dependent on Subtask 4, and carries the same risk of breaking existing code.
    *   Dependencies: Subtask 4
    *   Potential Issues:  Ensuring consistency between field types and their corresponding CozoDB types.
*   **Subtask ID: 6**
    *   Description: Update `ParameterNode` to use the new `TypeId` type (from Subtask 4) for its `type_id` field.
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: Yes - Dependent on Subtask 4, and carries the same risk of breaking existing code.
    *   Dependencies: Subtask 4
    *   Potential Issues: Ensuring consistency between parameter types and their corresponding CozoDB types.
*   **Subtask ID: 7**
    *   Description:  Modify `Attribute` to store its `value` as an `Option<Bytes>` instead of `Option<String>`.
    *   Estimated Time: 3 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask 1
    *   Potential Issues:  Handling attributes with non-string values.
*   **Subtask ID: 8**
    *   Description: Review and update all relevant code (e.g., visitor functions, serialization logic) to reflect the new type mappings.
    *   Estimated Time: 8 hours
    *   Cfg Flag Required?: Yes - This is a broad change that touches many parts of the codebase and could introduce regressions.
    *   Dependencies: Subtasks 2, 3, 4, 5, 6, 7
    *   Potential Issues:  Missing updates in certain parts of the codebase.  Introducing subtle bugs due to incorrect type conversions.

I've included `cfg` flags for Subtasks 4, 5, 6, and 8 because they represent significant changes to the core type system and could potentially break existing functionality.  The reasoning for each `cfg` flag is explained in the "Explanation" field.  I've also identified potential issues for each subtask.
