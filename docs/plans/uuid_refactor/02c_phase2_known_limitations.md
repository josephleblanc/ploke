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

## 2. Associated Items in `trait` Definitions Not Parsed

*   **Limitation:** The parser currently does not create nodes or relations for associated constants (`const NAME: Type = ...;`) or associated types (`type Name = ...;`) defined within `trait` blocks. Only associated functions (methods) are processed.
*   **Discovery:** This was noted via comments (`NOTE: Associated types are not stored directly on TraitNode yet`) in the tests within [`crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/traits.rs`](../../../../crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/traits.rs). Examination of the visitor confirmed the lack of implementation.
*   **Root Cause:** Similar to the limitation with `impl` blocks, the `visit_item_trait` method in [`crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`](../../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs) iterates through `item_trait.items` but only contains logic to handle the `syn::TraitItem::Fn` variant. It lacks handlers for `syn::TraitItem::Const` and `syn::TraitItem::Type`.
*   **Affected Code:**
    *   `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs` (primarily `visit_item_trait`)
    *   Potentially `crates/ingest/syn_parser/src/parser/nodes.rs` (e.g., `TraitNode` might need fields to store associated item IDs, or new node types might be needed).
    *   Potentially `crates/ingest/syn_parser/src/parser/relations.rs` (if new relation kinds are needed to link associated items to their `TraitNode`).
*   **Future Ignored Tests (Examples):**
    *   `test_associated_const_found_in_trait`
    *   `test_associated_type_found_in_trait`
    *   `test_relation_trait_contains_associated_const`
    *   `test_relation_trait_contains_associated_type`

---

## 3. `pub(crate)` Visibility Parsed as `Restricted(["crate"])`

*   **Limitation:** The `convert_visibility` function in the visitor currently parses `pub(crate)` visibility (`syn::Visibility::Restricted` with path "crate") into `VisibilityKind::Restricted(vec!["crate".to_string()])` instead of the dedicated `VisibilityKind::Crate` variant.
*   **Discovery:** This was identified when the test `test_value_node_field_visibility_crate` in [`crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/const_static.rs`](../../../../crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/const_static.rs) failed. The test initially expected `VisibilityKind::Crate` but the actual parsed value was `VisibilityKind::Restricted(["crate"])`.
*   **Root Cause:** The `convert_visibility` method in [`crates/ingest/syn_parser/src/parser/visitor/state.rs`](../../../../crates/ingest/syn_parser/src/parser/visitor/state.rs) handles `syn::Visibility::Restricted` generically by collecting path segments. It doesn't have a specific check to map the case where the path is exactly `["crate"]` to the `VisibilityKind::Crate` enum variant. This is deferred pending decisions on how visibility resolution will work in Phase 3.
*   **Affected Code:**
    *   `crates/ingest/syn_parser/src/parser/visitor/state.rs` (`convert_visibility` function)
*   **Future Ignored Tests (Examples):**
    *   Tests specifically asserting `VisibilityKind::Crate` for `pub(crate)` items (currently adjusted to expect `Restricted(["crate"])`).

---

*(Add subsequent limitations below this line)*
