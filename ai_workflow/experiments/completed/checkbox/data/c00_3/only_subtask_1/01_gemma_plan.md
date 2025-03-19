## Type Alignment

**Main Task Description:**
Modify the data types used within the `syn_parser` crate to be compatible with the CozoDB schema. This involves replacing existing Rust types with their CozoDB equivalents where appropriate, ensuring data consistency and enabling seamless integration with CozoDB.

## Feature Flag: `feature_cozo_type_alignment`
Apply this feature flag to any code changes that would break compilation if not fully implemented.

## Subtasks:
- [ ] 1. **Identify Type Mismatches:** Analyze the `syn_parser` code (specifically `src/parser/nodes.rs`) and identify all instances where Rust types are used to represent data that will eventually be stored in CozoDB.  Document these mismatches.
- [ ] 2. **Map Rust Types to CozoDB Types:** Create a mapping table detailing the conversion from each identified Rust type to its corresponding CozoDB type.  Consider the following:
    - `String` -> `String` (likely compatible)
    - `usize` -> `Int` or `Float` (depending on expected range and precision)
    - `bool` -> `Bool` (compatible)
    - `Vec<u8>` -> `Bytes` (for content hashes, etc.)
    - `f32`, `f64` -> `Float`
    - `i32`, `i64` -> `Int`
    - `Uuid` -> `Uuid` (compatible)
- [ ] 3. **Implement Type Conversions:** Modify the `syn_parser` code to perform the necessary type conversions when creating data structures that will be stored in CozoDB. This will likely involve changes in `src/parser/visitor.rs` where the `CodeGraph` is populated.
    - [ ] 3.1. Update `FunctionNode`, `StructNode`, `EnumNode`, `FieldNode`, `VariantNode`, `TypeAliasNode`, `UnionNode`, `ImplNode`, `TraitNode`, `ModuleNode`, `ValueNode`, `MacroNode`, `MacroRuleNode`, `Attribute`, `ParameterNode` to use the CozoDB-compatible types.
- [ ] 4. **Update Serialization:**  Ensure the `ron` serialization in `src/serialization/ron.rs` handles the new types correctly.  This might require adjustments to the `to_string_pretty` function or the use of custom serialization logic.

## Testing Strategy:
- **Unit Tests:** Add unit tests to `src/parser/mod.rs` and `src/serialization/mod.rs` to verify that the type conversions are performed correctly and that the serialized data matches the expected CozoDB schema.
- **Integration Tests:**  Extend the existing integration tests in `tests/mod.rs` to include checks that the data written to CozoDB (via the serialization process) is valid and can be read back correctly.
- **Manual Inspection:**  Manually inspect the serialized data to confirm that the types are as expected.
