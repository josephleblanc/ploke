## Subtasks:

1.  **Subtask:** Analyze existing `syn_parser` types and map them to CozoDB types. Focus on `String`, `Bytes`, `Uuid`, `Number` (Int/Float), and `List`. Document the mapping decisions.
    **Effort:** 4 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** `feat: Analyze and document type mapping to CozoDB`

2.  **Subtask:** Replace `String` with `Bytes` for content hashes and any other fields currently storing binary data as strings. This will likely involve changes in `Attribute` struct.
    **Effort:** 6 hours
    **Breaking Change?** Maybe (if consumers rely on string manipulation of hashes)
    **Feature Flag (if applicable):** `feature_cozo_bytes_for_hashes`
    **Commit Message:** `feat: Use Bytes for content hashes`

3.  **Subtask:**  Adjust numeric types (Int, Float) to align with CozoDB's Number representation.  This might involve using a specific integer size (i64) or consistently using floats.  Focus on `FieldNode` and potentially `ValueNode`.
    **Effort:** 4 hours
    **Breaking Change?** Maybe (if existing code relies on specific integer sizes)
    **Feature Flag (if applicable):** `feature_cozo_numeric_types`
    **Commit Message:** `feat: Align numeric types with CozoDB`

4.  **Subtask:**  Modify `List` representation to match CozoDB's List type. This may involve changing the underlying data structure or how lists are serialized/deserialized.  Impacts `TypeDefNode` and potentially `ValueNode`.
    **Effort:** 8 hours
    **Breaking Change?** Maybe (if list serialization/deserialization is exposed)
    **Feature Flag (if applicable):** `feature_cozo_list_type`
    **Commit Message:** `feat: Align list representation with CozoDB`

5.  **Subtask:** Update serialization logic in `ron.rs` to handle the new types correctly. Ensure that `Bytes`, adjusted numerics, and lists are serialized and deserialized as expected by CozoDB.
    **Effort:** 6 hours
    **Breaking Change?** Yes (serialization format changes)
    **Feature Flag (if applicable):** `feature_cozo_type_alignment`
    **Commit Message:** `feat: Update RON serialization for CozoDB types`

6.  **Subtask:**  Update tests (in `tests/mod.rs` and submodules) to reflect the type changes and ensure compatibility with the new schema.
    **Effort:** 6 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** `test: Update tests for CozoDB type alignment`

7.  **Subtask:** Review and document all changes, paying close attention to potential compatibility issues and migration strategies.
    **Effort:** 2 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** `docs: Document CozoDB type alignment changes`
