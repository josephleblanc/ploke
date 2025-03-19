## Subtasks:

1.  **Subtask:** Analyze `syn_parser` types and map them to CozoDB equivalents. Document the mapping decisions.
    **Effort:** 4 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** `feat: Document CozoDB type mapping for syn_parser`

2.  **Subtask:** Replace `String` with `Bytes` for content hashes and similar data within `nodes.rs`.
    **Effort:** 6 hours
    **Breaking Change?** Maybe (if consumers rely on `String` for hash comparisons)
    **Feature Flag (if applicable):** `feature_cozo_bytes_for_hashes`
    **Commit Message:** `feat: Use Bytes for content hashes in nodes`

3.  **Subtask:** Adjust numeric types (Int, Float) to match CozoDB's specifications (Int64, Float64) in `nodes.rs` and `types.rs`.
    **Effort:** 4 hours
    **Breaking Change?** Maybe (if consumers rely on specific integer sizes)
    **Feature Flag (if applicable):** `feature_cozo_numeric_types`
    **Commit Message:** `feat: Align numeric types with CozoDB specifications`

4.  **Subtask:** Implement `Send + Sync` traits for all publicly exposed types in `nodes.rs`, `parser/graph.rs`, and `parser/types.rs`.
    **Effort:** 8 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** `feat: Implement Send + Sync for public types`

5.  **Subtask:** Begin refactoring to remove the `CodeGraph`. Start by modifying the `visitor.rs` to directly process `syn` AST nodes and prepare data for CozoDB insertion. Focus on functions first.
    **Effort:** 12 hours
    **Breaking Change?** Yes (major refactor)
    **Feature Flag (if applicable):** `feature_cozo_direct_insertion`
    **Commit Message:** `feat: Begin refactoring for direct CozoDB insertion - functions`

6.  **Subtask:** Continue refactoring to remove the `CodeGraph`.  Handle structs, enums, and other AST elements in `visitor.rs`, preparing data for CozoDB.
    **Effort:** 16 hours
    **Breaking Change?** Yes (major refactor - continuation of #5)
    **Feature Flag (if applicable):** `feature_cozo_direct_insertion`
    **Commit Message:** `feat: Continue refactoring for direct CozoDB insertion - structs & enums`

7.  **Subtask:**  Finalize `CodeGraph` removal.  Clean up any remaining references and ensure all data is streamed directly to CozoDB.
    **Effort:** 4 hours
    **Breaking Change?** Yes (finalization of #5 & #6)
    **Feature Flag (if applicable):** `feature_cozo_direct_insertion`
    **Commit Message:** `feat: Complete removal of CodeGraph and direct CozoDB insertion`

8.  **Subtask:** Add comprehensive tests to verify the new data flow and type mappings.
    **Effort:** 8 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** `test: Add tests for CozoDB type mapping and direct insertion`
