# Comprehensive Test Plan: Phase 2 - Parallel Parse & Provisional Graph Generation

## 1. Overview

**Goal:** To rigorously verify the implementation of Phase 2 ("Parallel Parse & Provisional Graph Generation") of the UUID refactoring plan for `syn_parser`, ensuring correctness, robustness, and adherence to the design outlined in [00_overview_batch_processing_model.md](00_overview_batch_processing_model.md) and [02_phase2_parallel_parse_implementation.md](02_phase2_parallel_parse_implementation.md).

**Scope:** Testing the `analyze_files_parallel` function (and its worker function `analyze_file_phase2`) under the `uuid_ids` feature flag. This includes verifying:
    - Correct consumption of Phase 1 `DiscoveryOutput`.
    - Parallel file processing using `rayon`.
    - Generation of `NodeId::Synthetic`, `TypeId::Synthetic`, and `TrackingHash` UUIDs using correct context.
    - Creation of partial `CodeGraph` structures containing provisional data (synthetic IDs, tracking hashes, unresolved type info).
    - Correct formation of relations using `GraphId` wrappers and synthetic IDs.
    - Robustness against various Rust language constructs and potential errors.
    - Adherence to specific implementation details and handling of known deviations (e.g., `FieldNode` ID generation).

**Testing Philosophy:** Aim for extremely high coverage ("adamantium solid") due to the foundational nature of the parser. Tests should cover happy paths, edge cases, error conditions, and specific implementation choices. We will identify limitations and distinguish between implementation bugs and intentional design constraints.

**Prerequisites:**
    - Phase 1 (`run_discovery_phase`) is assumed to function correctly, providing valid `DiscoveryOutput`.
    - `ploke-core` provides the necessary ID types and generation logic.

## 2. Test Setup & Environment

*   **Feature Flag:** All tests targeting Phase 2 functionality **MUST** be run with the `uuid_ids` feature enabled (`cargo test -p syn_parser --features uuid_ids`).
*   **Fixtures:** Utilize a combination of dedicated fixture crates:
    *   `simple_crate`: For basic, minimal validation of core constructs.
    *   `example_crate`: For testing interactions between modules and basic dependencies.
    *   `file_dir_detection`: For testing complex module structures, visibility, and file organization scenarios.
    *   Potentially create new, targeted micro-fixtures for specific edge cases identified during test development.
*   **Test Location:** New tests should reside primarily within `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/`. Unit tests for ID generation might reside closer to the implementation (e.g., in `ploke-core` or `syn_parser/src/parser/visitor/` test modules).
*   **Helpers:** Leverage existing test helpers (`fixtures_crates_dir`, etc.) and potentially create new ones specific to Phase 2 validation (e.g., helpers to find nodes/types/relations with specific synthetic ID properties or structures).

## 3. Unit Tests (ID & Hash Generation Logic)

**Goal:** Verify the correctness, consistency, and uniqueness of the core ID and hash generation functions in isolation. These tests likely belong in `ploke-core` or near the `VisitorState` implementation.

*   **[x] Test `NodeId::generate_synthetic`:** (Covered indirectly by integration tests showing ID differences based on context)
    *   **Consistency:** Same inputs (`crate_namespace`, `file_path`, `relative_path`, `item_name`, `span`) produce the same `NodeId::Synthetic(Uuid)`. (Verified by `determinism::determinism_tests::test_phase2_determinism`)
    *   **Uniqueness (Sensitivity):**
        *   Different `crate_namespace` -> different ID. (Verified by `ids::phase2_id_tests::test_synthetic_node_ids_differ_across_crates`)
        *   Different `file_path` -> different ID. (Verified by `ids::phase2_id_tests::test_synthetic_ids_differ_across_files_same_crate_name`)
        *   Different `relative_path` -> different ID. (Implicitly tested by file path differences)
        *   Different `item_name` -> different ID. (Implicitly tested by file path differences)
        *   Different `span` -> different ID. (Verified by `ids::phase2_id_tests::test_synthetic_ids_differ_across_files_same_crate_name` - fixture 2 has different span)
    *   **Edge Cases:** Test with empty `relative_path`, empty `item_name` (if possible), zero `span`.
*   **[x] Test `TypeId::generate_synthetic`:** (Covered indirectly by integration tests showing ID differences based on context)
    *   **Consistency:** Same inputs (`crate_namespace`, `file_path`, `type_string_repr`) produce the same `TypeId::Synthetic(Uuid)`. (Verified by `determinism::determinism_tests::test_phase2_determinism`)
    *   **Uniqueness (Sensitivity):**
        *   Different `crate_namespace` -> different ID. (Implicitly tested by node ID tests across crates)
        *   Different `file_path` -> different ID. (Verified by `ids::phase2_id_tests::test_synthetic_ids_differ_across_files_same_crate_name` - param type ID differs)
        *   Different `type_string_repr` -> different ID. (Needs specific test)
    *   **Edge Cases:** Test with complex `type_string_repr` (generics, lifetimes, paths), empty string (if possible).
*   **[x] Test `TrackingHash::generate`:** (Covered indirectly by integration tests showing hash presence and determinism)
    *   **Consistency:** Same inputs (`crate_namespace`, `file_path`, `item_tokens`) produce the same `TrackingHash(Uuid)`. (Verified by `determinism::determinism_tests::test_phase2_determinism`)
    *   **Uniqueness (Sensitivity):**
        *   Different `crate_namespace` -> different Hash. (Needs specific test)
        *   Different `file_path` -> different Hash. (Needs specific test)
        *   Different `item_tokens` (content change) -> different Hash. (Needs specific test)
    *   **[ ] Insensitivity (Current Limitation):** Verify that changes *only* in whitespace or comments *do* currently change the hash (due to `to_string()`). Document this limitation. (Needs specific test)
    *   **Robustness:** Test with various token streams (empty, simple, complex). (Needs specific test)
        * NOTE: We will likely soon improve `TrackingHash` to be less sensitive to whitespace. When that refactor occurs, we may revisit the whitespace-only `TrackingHash` sensitive, and invert these tests to verify that whitespace does not cause the `TrackingHash` to change.

## 4. Integration Tests (`analyze_files_parallel`)

**Goal:** Verify the end-to-end functionality of Phase 2, ensuring correct graph structures with provisional data are generated for various inputs. Tests will primarily run `run_phase1_phase2` helper and assert on the resulting `Vec<Result<CodeGraph, syn::Error>>`.

### 4.1 Core Functionality & Output Structure

*   **[/] Test Basic Execution:** (Partially covered)
    *   Run on `simple_crate`. Verify output `Vec` has length 1. Verify the `Result` is `Ok`. (Covered by `basic::phase2_tests::test_simple_crate_phase2_output`)
    *   Run on `example_crate`. Verify output `Vec` has the correct length (number of `.rs` files). Verify all `Result`s are `Ok`. (Covered by `determinism::determinism_tests::test_phase2_determinism` setup)
    *   **[ ] Run on `file_dir_detection`. Verify output `Vec` has the correct length. Verify all `Result`s are `Ok`.**
*   **[x] Test Context Propagation (Indirect):** (Covered by `ids::*` tests)
    *   Run Phase 2 on two *different* fixture crates within the same test.
    *   Verify that items with the *same name* and *relative path* but in *different crates* result in different `NodeId::Synthetic` UUIDs (due to different `crate_namespace`). Requires careful fixture design or UUID inspection. (Covered by `ids::phase2_id_tests::test_synthetic_node_ids_differ_across_crates`)
    *   Verify that items with the *same name* and *relative path* but in *different files* (same crate) result in different `NodeId::Synthetic` UUIDs (due to different file path). (Covered by `ids::phase2_id_tests::test_synthetic_ids_differ_across_files_same_crate_name`)
*   **[x] Test Determinism:** (Covered by `determinism::determinism_tests::test_phase2_determinism`)
    *   Run `run_phase1_phase2` multiple times on the *same* fixture crate.
    *   Assert that the resulting `CodeGraph` structures are identical (using `assert_eq!` if `CodeGraph` derives `PartialEq`, otherwise compare field by field, potentially skipping UUIDs if comparison is too complex).
    *   **(Advanced):** If possible, capture and compare the actual generated UUIDs within a single run's output graph to ensure internal consistency (e.g., a specific function parameter always links to the same synthetic `TypeId`).

### 4.2 Graph Node Verification

(Partially covered by `basic::phase2_tests::test_simple_crate_phase2_output` and `ids::phase2_id_tests::test_synthetic_ids_and_hashes_present_simple_crate`. Needs systematic checks for all node types and fields.)

*   **[/] Functions (`ItemFn`):** (Basic ID/Hash/Param/Return presence checked)
    *   Verify `FunctionNode` exists in `graph.functions`.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))`.
    *   Assert `parameters` contains correct `ParamData` with `TypeId::Synthetic(_)`.
    *   Assert `return_type` (if present) is `Some(TypeId::Synthetic(_))`.
    *   **[ ] Verify other fields (name, visibility, generics, attributes, docstring, body string).**
*   **[ ] Structs (`ItemStruct`):**
    *   Verify `TypeDefNode::Struct` exists in `graph.defined_types`.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))`.
    *   Verify `fields` contains `FieldNode`s.
        *   Assert `FieldNode.id` is `NodeId::Synthetic(_)`.
        *   Assert `FieldNode.type_id` is `TypeId::Synthetic(_)`.
    *   Verify other fields (name, visibility, generics, attributes, docstring).
*   **[ ] Enums (`ItemEnum`):**
    *   Verify `TypeDefNode::Enum` exists.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))`.
    *   Verify `variants` contains `VariantNode`s.
        *   Assert `VariantNode.id` is `NodeId::Synthetic(_)`.
        *   Verify `VariantNode.fields` contains `FieldNode`s with `NodeId::Synthetic` and `TypeId::Synthetic`.
    *   Verify other fields (name, visibility, generics, attributes, docstring).
*   **[ ] Type Aliases (`ItemType`):**
    *   Verify `TypeDefNode::TypeAlias` exists.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))`.
    *   Assert `type_id` (the aliased type) is `TypeId::Synthetic(_)`.
    *   Verify other fields (name, visibility, generics, attributes, docstring).
*   **[ ] Unions (`ItemUnion`):**
    *   Verify `TypeDefNode::Union` exists.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))`.
    *   Verify `fields` contains `FieldNode`s with `NodeId::Synthetic` and `TypeId::Synthetic`.
    *   Verify other fields (name, visibility, generics, attributes, docstring).
*   **[ ] Traits (`ItemTrait`):**
    *   Verify `TraitNode` exists in `graph.traits` (or `private_traits`).
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))`.
    *   Verify `methods` contains `FunctionNode`s (check their IDs/hashes).
    *   Assert `super_traits` contains `TypeId::Synthetic(_)`.
    *   Verify other fields (name, visibility, generics, attributes, docstring).
*   **[ ] Impls (`ItemImpl`):**
    *   Verify `ImplNode` exists in `graph.impls`.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `self_type` is `TypeId::Synthetic(_)`.
    *   Assert `trait_type` (if present) is `Some(TypeId::Synthetic(_))`.
    *   Verify `methods` contains `FunctionNode`s (check their IDs/hashes).
    *   Verify generics.
*   **[ ] Modules (`ItemMod`):**
    *   Verify `ModuleNode` exists in `graph.modules`.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))` (except maybe root).
    *   Verify `path` is correct relative to crate root.
    *   Verify `submodules` and `items` contain `NodeId::Synthetic(_)`.
    *   Verify `imports` contains `ImportNode`s.
    *   Verify other fields (name, visibility, attributes, docstring).
*   **[ ] Constants/Statics (`ItemConst`, `ItemStatic`):**
    *   Verify `ValueNode` exists in `graph.values`.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))`.
    *   Assert `type_id` is `TypeId::Synthetic(_)`.
    *   Verify other fields (name, visibility, kind, value string, attributes, docstring).
*   **[ ] Macros (`ItemMacro`, `ItemFn` proc macros):**
    *   Verify `MacroNode` exists in `graph.macros`.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Assert `tracking_hash` is `Some(TrackingHash(_))`.
    *   Verify kind (`DeclarativeMacro`, `ProcedureMacro`).
    *   Verify other fields (name, visibility, attributes, docstring, body string).
*   **[ ] Use Statements (`ItemUse`, `ItemExternCrate`):**
    *   Verify `ImportNode` exists in `graph.use_statements` and relevant `ModuleNode.imports`.
    *   Assert `id` is `NodeId::Synthetic(_)`.
    *   Verify fields (`path`, `kind`, `visible_name`, `original_name`, `is_glob`).

### 4.3 Graph Relation Verification

**(No specific tests implemented yet)**
*   **[ ] `Contains`:**
    *   Verify relation exists between module `NodeId::Synthetic` and contained item `NodeId::Synthetic`.
    *   Check `source` is `GraphId::Node(module_id)`.
    *   Check `target` is `GraphId::Node(item_id)`.
*   **[ ] `StructField` / `EnumVariant` Fields:**
    *   Verify relation exists between struct/enum/variant `NodeId::Synthetic` and field `NodeId::Synthetic`.
    *   Check `source` is `GraphId::Node(parent_id)`.
    *   Check `target` is `GraphId::Node(field_id)`.
    *   **Crucially:** Test the case where `FieldNode.id` was generated via `generate_synthetic_node_id` directly, ensuring this relation is still created correctly.
*   **[ ] `FunctionParameter` / `FunctionReturn`:**
    *   Verify relation exists between function `NodeId::Synthetic` and parameter/return `TypeId::Synthetic`.
    *   Check `source` is `GraphId::Node(function_id)`.
    *   Check `target` is `GraphId::Type(type_id)`.
*   **[ ] `ImplementsFor` / `ImplementsTrait`:**
    *   Verify relation exists between impl `NodeId::Synthetic` and self `TypeId::Synthetic`.
    *   If trait impl, verify relation exists between impl `NodeId::Synthetic` and trait `TypeId::Synthetic`.
    *   Check `source` is `GraphId::Node(impl_id)`.
    *   Check `target` is `GraphId::Type(type_id)`.
*   **[ ] `Uses` (for `extern crate` and potentially `use`):**
    *   Verify relation exists between `ImportNode` `NodeId::Synthetic` and the corresponding external crate/item `TypeId::Synthetic`.
    *   Check `source` is `GraphId::Node(import_id)`.
    *   Check `target` is `GraphId::Type(type_id)`.
*   **[ ] `ValueType`:**
    *   Verify relation exists between const/static `NodeId::Synthetic` and its `TypeId::Synthetic`.
    *   Check `source` is `GraphId::Node(value_id)`.
    *   Check `target` is `GraphId::Type(type_id)`.
*   **[ ] `ModuleImports`:**
    *   Verify relation exists between module `NodeId::Synthetic` and `ImportNode` `NodeId::Synthetic`.
    *   Check `source` is `GraphId::Node(module_id)`.
    *   Check `target` is `GraphId::Node(import_id)`.
*   **(Others):** Add checks for `Method`, `EnumVariant`, `Inherits`, `MacroUse` as applicable.

### 4.4 Type System Verification

**(No specific tests implemented yet)**
*   **[ ] `TypeNode` Creation:**
    *   For various type constructs (paths, references, slices, tuples, generics, function pointers, etc.), verify that corresponding `TypeNode` entries are created in `graph.type_graph`.
    *   Assert `TypeNode.id` is `TypeId::Synthetic(_)`.
    *   Assert `TypeNode.kind` accurately reflects the `syn::Type` structure.
    *   Assert `TypeNode.related_types` contains the correct `TypeId::Synthetic` IDs for nested types.
*   **[ ] Type Caching (`VisitorState.type_map`):**
    *   Use a fixture where the same complex type (e.g., `Vec<Option<String>>`) appears multiple times.
    *   Verify that only *one* `TypeNode` is created for this type in `graph.type_graph`.
    *   Verify that all usages correctly reference the *same* `TypeId::Synthetic` ID. (Requires inspecting multiple nodes/relations).
*   **[ ] Cyclic Types:**
    *   Use a fixture with a self-referential struct (e.g., `struct Node { next: Option<Box<Node>> }`).
    *   Verify that parsing completes successfully without infinite recursion.
    *   Verify the `TypeNode` for `Node` is created and its `related_types` correctly references its own `TypeId::Synthetic`.

### 4.5 Tracking Hash Verification

(Partially covered by `ids::phase2_id_tests::test_synthetic_ids_and_hashes_present_simple_crate` checking for presence)
*   **[/] Hash Generation:** Verify `tracking_hash` is `Some` for all expected node types. (Presence checked)
*   **[ ] Hash Sensitivity (Basic):**
    *   Parse a fixture file.
    *   Create a modified version with a meaningful code change (e.g., change function body logic, add a field). Parse it.
    *   Verify the `TrackingHash` value differs for the modified node. (Requires capturing/comparing hash values).
*   **[ ] Hash Insensitivity (Current Limitation):**
    *   Create a modified version with only whitespace/comment changes. Parse it.
    *   Verify the `TrackingHash` *also* differs (confirming the limitation of `to_string()`).

### 4.6 Error Handling Verification

**(No specific tests implemented yet)**
*   **[ ] Syntax Errors:**
    *   Create a fixture file with invalid Rust syntax.
    *   Run `run_phase1_phase2`.
    *   Assert that the `Vec` contains a `Result::Err(syn::Error)` for that specific file.
    *   Assert that results for other valid files (if any in the batch) are `Ok`.
*   **[ ] File I/O Errors:**
    *   Simulate a file read error during Phase 2 (e.g., by manipulating permissions temporarily, though this is hard in standard tests).
    *   If possible, verify that `analyze_file_phase2` returns `Err(syn::Error)` wrapping the I/O error.

### 4.7 Feature Flag Interaction

**(Assumed working based on test setup, no dedicated tests)**
*   **[ ] Run Phase 2 Tests:** Execute all tests developed above using `cargo test -p syn_parser --features uuid_ids`. Ensure they pass.
*   **[ ] Run Non-UUID Tests:** Execute the existing test suite using `cargo test -p syn_parser --no-default-features`. Ensure they still pass (verifying the non-UUID path isn't broken).
*   **[ ] Compile Checks:** Ensure `cargo check -p syn_parser --features uuid_ids` and `cargo check -p syn_parser --no-default-features` both succeed.

## 5. Fixture Requirements

*   **`simple_crate`:** Minimal valid Rust code (e.g., one function, one struct). Used for basic smoke tests.
*   **`example_crate`:** Multiple modules (`mod.rs` and `file.rs`), basic `use` statements, structs/enums/functions defined across modules. Used for testing module path handling and basic item interactions.
*   **`file_dir_detection`:** Complex nested module structure (`a/b/mod.rs`, `a/b/c.rs`), `pub use` re-exports, various visibility modifiers (`pub`, `pub(crate)`, `pub(in path)`), items defined at different levels. Used for testing complex module resolution, visibility handling (as input to Phase 3), and path generation.
*   **New Fixtures (Examples):**
    *   `fixture_generics`: Structs, functions, traits, impls with various generic parameters (types, lifetimes, consts), bounds, where clauses.
    *   `fixture_types`: Examples of all `syn::Type` variants (tuples, slices, arrays, references, pointers, function pointers, trait objects, impl trait).
    *   `fixture_macros`: `macro_rules!` definitions (exported and local), procedural macros (derive, attribute, function-like).
    *   `fixture_attributes`: Items with various standard and custom attributes.
    *   `fixture_cyclic_types`: Self-referential structs or type aliases.
    *   `fixture_errors`: Files containing specific syntax errors.
    *   `fixture_tracking_hash`: Files designed for testing `TrackingHash` sensitivity/insensitivity.

## 6. Test Implementation Notes

*   Focus assertions on the *structure* and *presence* of synthetic IDs/hashes initially. Asserting specific UUID values is difficult and brittle.
*   Develop helper functions to navigate the `CodeGraph` and find specific nodes/types/relations based on names or paths to simplify assertions.
*   Clearly document the purpose of each test and the specific aspect of Phase 2 it verifies.
*   Mark tests that expose known limitations (like `TrackingHash` sensitivity) appropriately.

This plan provides a comprehensive roadmap for testing Phase 2. We can refine and add more specific test cases as we proceed with implementation.
