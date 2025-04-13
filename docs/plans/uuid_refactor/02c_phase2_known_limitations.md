# Phase 2 (`uuid_ids`) Known Limitations

This document tracks known limitations, missing features, or areas where the Phase 2 "Parallel Parse Implementation" (`uuid_ids` feature) deviates from complete Rust syntax coverage or desired graph structure. These limitations were primarily discovered during testing and are documented here to inform future development and prevent regressions.

---

## 1. Associated Items in `impl` Blocks Not Parsed

*   **Limitation:** The parser currently does not create nodes or relations for associated constants (`const NAME: Type = ...;`) or associated types (`type Name = ...;`) defined within `impl` blocks. Only associated functions (methods) are processed.
*   **Discovery:** This was identified when the test `test_const_static_basic_smoke_test_full_parse` in [`crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/const_static.rs`](../../../../crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/const_static.rs) failed. The test expected to find a `ValueNode` for `IMPL_CONST`, which is defined as an associated constant in the `const_static.rs` fixture, but no such node was present in the `graph.values` collection.
*   **Root Cause:** The `visit_item_impl` method in [`crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`](../../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs) iterates through `item_impl.items` but only contains logic to handle the `syn::ImplItem::Fn` variant. It lacks handlers for `syn::ImplItem::Const` and `syn::ImplItem::Type`.
*   **Affected Code:**
    *   `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs` (primarily `visit_item_impl`)
    *   Potentially `crates/ingest/syn_parser/src/parser/nodes.rs` (if associated items require distinct node types or modifications to existing ones, e.g., `ValueNode`, `TypeAliasNode`).
    *   Potentially `crates/ingest/syn_parser/src/parser/relations.rs` (if new relation kinds are needed to link associated items to their `impl` block or the type being implemented).
*   **Future Ignored Tests (Examples):**
    *   `test_associated_const_found_in_impl`
    *   `test_associated_type_found_in_impl`
    *   `test_relation_impl_contains_associated_const`
    *   `test_relation_impl_contains_associated_type`

---

*(Add subsequent limitations below this line)*
