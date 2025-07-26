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

---

## 4. ✅ `TypeId` Conflation for Generics and `Self` Types

*   **Status:** **ADDRESSED**
*   **Original Limitation:** The initial implementation of `TypeId::generate_synthetic` did not incorporate sufficient contextual information (like the defining scope). This led to identical `TypeId`s being generated for generic parameters (e.g., `<T>`) or `Self` types defined in different scopes (e.g., different functions, structs, or impl blocks), even though they represented distinct types semantically.
*   **Resolution:** The `TypeId::generate_synthetic` function in [`ploke-core/src/lib.rs`](../../../../crates/ploke-core/src/lib.rs) was updated to include the `parent_scope_id` (the `NodeId` of the containing definition like a function, struct, impl, etc.) as part of its hash input. This ensures that types like `T` or `Self` defined within different scopes now generate distinct `TypeId`s.
*   **Validation:** Tests in [`crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/type_conflation.rs`](../../../../crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/type_conflation.rs) now pass, specifically:
    *   `test_generic_param_conflation_in_functions`: Verifies distinct `TypeId`s for `<T>` in different functions.
    *   `test_self_return_type_conflation_in_impls`: Verifies distinct `TypeId`s for `Self` return types in different impl blocks.
    *   `test_generic_field_conflation_in_structs`: Verifies distinct `TypeId`s for `<T>` field types in different struct definitions.
*   **Additional Validation:** Some tests in 
    *   `test_impl_node_self_type_conflation_phase2`: Verifies distinct `TypeId`s for certain node fields in test directory `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/`
        * `impls.rs`: Verified different `Self` type (capitalized due to syn parsing) across two self-impl blocks for `SimpleStruct` vs `GenericStruct` for methods `SimpleStruct::new` (`Self` in body and return type) and `GenericStruct::print_value` (`&self` parameter)

---

## 5. Item-Level `#[cfg]` Attribute Handling (`NodeId` Conflation)

*   **Status:** **KNOWN LIMITATION (Deferred)**
*   **Limitation:** The `NodeId::generate_synthetic` function currently **does not** incorporate item-level `#[cfg(...)]` attributes into its hash input. As a result, identically named items within the same scope that differ only by mutually exclusive `cfg` attributes (e.g., `#[cfg(feature = "a")] struct Foo;` and `#[cfg(not(feature = "a"))] struct Foo;`) are assigned the **same `NodeId`**. The `CodeVisitor` creates duplicate node instances in the graph for each `cfg` branch, but these instances share the same ID.
*   **Discovery:** Identified during testing with the `fixture_conflation` crate.
*   **Root Cause:** Lack of `cfg` attribute processing within `NodeId::generate_synthetic` in [`ploke-core/src/lib.rs`](../../../../crates/ploke-core/src/lib.rs).
*   **Decision:** Handling item-level `cfg` attributes during Phase 2 ID generation has been explicitly **deferred** due to complexity. See [ADR-009: Defer Handling of Item-Level `cfg` Attributes in Phase 2 ID Generation](../adrs/proposed/ADR-009-Defer-Item-Level-Cfg-Handling.md).
*   **Validation:** Tests in [`crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/type_conflation.rs`](../../../../crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/type_conflation.rs) verify this *expected* conflation:
    *   `test_cfg_struct_node_id_conflation`
    *   `test_cfg_function_node_id_conflation`
    These tests assert that *two* node instances are created but share the *same* `NodeId`.

---

## 6. File-Level `#![cfg]` Attribute Handling (Attribute Propagation)

*   **Status:** **KNOWN LIMITATION (Proposal Pending)**
*   **Limitation:** File-level attributes (`#![cfg(...)]`) are correctly captured and stored on the corresponding `ModuleNode` (specifically in `ModuleDef::FileBased::file_attrs`). Items defined within these files receive distinct `NodeId`s due to the file path being part of the ID generation. However, the `cfg` context from the file-level attribute is **not** currently propagated or directly associated with the item nodes (e.g., `StructNode`, `FunctionNode`) defined within that file. Consumers need to traverse back to the containing `ModuleNode` to determine the file-level `cfg` context.
*   **Discovery:** Identified during testing with the `fixture_conflation` crate.
*   **Root Cause:** The `CodeVisitor` currently stores file-level attributes on the `ModuleNode` but doesn't pass this context down when visiting items within the file. See [`crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`](../../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs).
*   **Decision:** A proposal exists to associate file-level `cfg` attributes directly with contained items, but the specific mechanism (Phase 2 visitor change vs. later enhancement phase) is undecided. See [ADR-010: Apply File-Level `cfg` Attributes to Contained Items](../adrs/proposed/ADR-010-Apply-File-Level-Cfg-Attributes.md).
*   **Validation:** The test `test_file_level_cfg_struct_node_id_disambiguation` in [`crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/type_conflation.rs`](../../../../crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/type_conflation.rs) verifies:
    *   That `NodeId`s *are* distinct for items in different `cfg`-gated files (due to file path).
    *   That the file-level `cfg` attributes *are* correctly stored on the respective `ModuleNode`s.

---

## 7. `#[cfg(...)]` Attribute Evaluation – Unsupported Atoms & Fallback Bias

*   **Limitation:** The evaluator in [`crates/ingest/syn_parser/src/parser/visitor/cfg_evaluator.rs`](../../../../crates/ingest/syn_parser/src/parser/visitor/cfg_evaluator.rs) recognizes only the atoms `feature`, `target_os`, `target_arch`, and `target_family`.  
    All other widely-used atoms (e.g. `target_pointer_width`, `target_endian`, `windows`, `unix`, `test`, `debug_assertions`, `panic`, etc.) are silently treated as *false*, causing any item guarded by them to be **dropped from the graph**.

*   **Fallback Target Triple:** When the `TARGET` environment variable is absent, the code defaults to `"x86_64-unknown-linux-gnu"`.  
    This biases the corpus toward Linux/x86-64 code paths and breaks determinism across machines.

*   **Impact:**  
    • Valid conditional code is omitted without warning.  
    • Cross-platform crates appear to contain far less code than they actually do.  
    • Results are non-repeatable unless `TARGET` is explicitly set.

*   **Future Work:**  
    • Extend the atom set to match `rustc --print cfg`.  
    • Replace the fallback with an explicit CLI flag or error.

---

*(Add subsequent limitations below this line)*
