*   **Subtask ID: 1**
    *   Description: Analyze `syn_parser`'s existing types (e.g., `FunctionNode`, `StructNode`, `TypeId`) and map them to corresponding CozoDB types (Null, Bool, Number, String, Bytes, Uuid, Json, Vector). Document the mapping decisions, specifically focusing on how to best represent each `TypeKind` variant in CozoDB. Consider the implications of using `Bytes` for identifiers (function names, struct names, etc.) – potential performance impacts and the need for efficient string comparisons.
    *   Estimated Time: 6 hours
    *   Cfg Flag Required?: No
    *   Dependencies: None
    *   Potential Issues: Determining the best CozoDB type for complex Rust types (e.g., generics, enums). Handling potential data loss during type conversion. Performance implications of using `Bytes` for identifiers.
*   **Subtask ID: 2**
    *   Description: Modify `FunctionNode` to use `Bytes` for `name` instead of `String`.
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask 1
    *   Potential Issues: Impact on existing tests that rely on `String` for function names.
*   **Subtask ID: 3**
    *   Description: Modify `StructNode` and `EnumNode` to use `Bytes` for `name` instead of `String`.
    *   Estimated Time: 2 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask 1
    *   Potential Issues: Impact on existing tests that rely on `String` for struct/enum names.
*   **Subtask ID: 4**
    *   Description: Replace `TypeId` with a new enum, `CozoDbType`, that explicitly represents the CozoDB types (Null, Bool, Number, String, Bytes, Uuid, Json, Vector). Define the `CozoDbType` enum and update all relevant code to use it.
    *   Estimated Time: 8 hours
    *   Cfg Flag Required?: Yes - This change fundamentally alters how types are represented and could break existing code that relies on the previous `TypeId` definition. A `cfg` flag allows us to revert easily if necessary.
    *   Dependencies: Subtask 1
    *   Potential Issues: Complexity of representing nested types and generics within the `CozoDbType` enum. Ensuring the enum is expressive enough to capture all relevant type information.
*   **Subtask ID: 5**
    *   Description: Update `FieldNode` to use the new `CozoDbType` enum for its `type_id` field.
    *   Estimated Time: 3 hours
    *   Cfg Flag Required?: Yes - Dependent on Subtask 4, and carries the same risk of breaking existing code.
    *   Dependencies: Subtask 4
    *   Potential Issues: Ensuring consistency between field types and their corresponding CozoDB types.
*   **Subtask ID: 6**
    *   Description: Update `ParameterNode` to use the new `CozoDbType` enum for its `type_id` field.
    *   Estimated Time: 3 hours
    *   Cfg Flag Required?: Yes - Dependent on Subtask 4, and carries the same risk of breaking existing code.
    *   Dependencies: Subtask 4
    *   Potential Issues: Ensuring consistency between parameter types and their corresponding CozoDB types.
*   **Subtask ID: 7**
    *   Description: Modify `Attribute` to store its `value` as an `Option<Bytes>`. Consider whether all attribute values can reasonably be represented as `Bytes`. If not, explore a more flexible approach (e.g., an enum with different variants for different attribute value types).
    *   Estimated Time: 4 hours
    *   Cfg Flag Required?: No
    *   Dependencies: Subtask 1
    *   Potential Issues: Handling attributes with non-string values. Potential complexity of a more flexible attribute value type.
*   **Subtask ID: 8**
    *   Description: Review and update all relevant code, *specifically* the `visitor.rs` file, to correctly handle the new types and populate the `CodeGraph` accordingly. Ensure the `ron` serialization logic correctly handles the new `CozoDbType` enum.
    *   Estimated Time: 12 hours
    *   Cfg Flag Required?: Yes - This is a broad change that touches many parts of the codebase and could introduce regressions.
    *   Dependencies: Subtasks 2, 3, 4, 5, 6, 7
    *   Potential Issues: Missing updates in certain parts of the codebase. Introducing subtle bugs due to incorrect type handling. Serialization issues with the new `CozoDbType` enum.
