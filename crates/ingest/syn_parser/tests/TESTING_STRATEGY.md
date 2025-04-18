# syn_parser Testing Strategy

**Version:** 1.0
**Date:** 2025-04-18
**Status:** Adopted

## 1. Condensed Strategy

Maintain high confidence in `syn_parser` correctness while improving test maintainability through a tiered approach:

1.  **Core Function Unit Tests:** Exhaustively test critical functions (e.g., `NodeId::generate_synthetic`) in isolation.
2.  **Visitor Context Gathering Tests:** Verify the `CodeVisitor` correctly extracts and prepares the necessary context (paths, parent IDs, CFGs) *before* calling core generation functions.
3.  **Paranoid Helper "Canary" Tests:** Use a *reduced set* of existing paranoid helper tests (refactored with Context Objects) for key complex scenarios, ensuring end-to-end ID generation integrity.
4.  **Structural/Relational Tests:** Verify the overall `CodeGraph` structure and relationships (`Contains`, `Implements`, etc.), implicitly relying on correct IDs.
5.  **Refactor Helpers:** Centralize ID regeneration logic and use Context Objects to reduce brittleness and improve maintainability.
6.  **Introduce Error Handling:** Gradually replace panics with `Result<T, SynParserError>` for better diagnostics and robustness.

This strategy balances the need for rigorous validation (especially for ID generation) with the practical need to reduce test maintenance overhead during refactoring.

## 2. Detailed Elaboration

### 2.1 Rationale

The `syn_parser` crate is foundational for Ploke. Its output, the `CodeGraph`, must be accurate and internally consistent. Particularly crucial is the generation of unique and deterministic `NodeId`s and `TypeId`s, which depend on complex contextual information (file paths, module paths, parent scopes, CFG attributes, item kinds, names).

Historically, a highly "paranoid" testing approach, involving regenerating expected IDs within test helpers based on reconstructed context, was necessary. This approach successfully caught critical bugs in state management logic (e.g., scope stack handling, CFG context propagation). However, this paranoia led to brittle tests: logically sound refactors to the ID generation signature or context handling required tedious updates across numerous tests, hindering development velocity and refactoring efforts.

This tiered strategy aims to retain the high confidence provided by rigorous checks while making the test suite more resilient to change.

### 2.2 Test Tiers Explained

*   **Tier 1: Core Function Unit Tests**
    *   **Focus:** Exhaustive testing of the core ID generation functions (`ploke_core::NodeId::generate_synthetic`, `ploke_core::TypeId::generate_synthetic`, `ploke_core::TrackingHash::generate`) and key helper functions (`calculate_cfg_hash_bytes`).
    *   **Method:** Standard Rust unit tests (`#[test]`) located near the function definitions (likely in `ploke-core` or `syn_parser::parser::visitor`).
    *   **Examples:**
        *   Test `NodeId::generate_synthetic` with various combinations of inputs: different file paths, module paths, item kinds, parent scope IDs (None, Some), and `cfg_bytes` (None, Some with different content/ordering). Assert the output UUID changes appropriately.
        *   Test `calculate_cfg_hash_bytes` with empty lists, single items, multiple items (sorted vs. unsorted input), ensuring deterministic output.
    *   **Goal:** Highest rigor on the fundamental algorithms. Changes to ID generation logic primarily impact these tests.

*   **Tier 2: Visitor Context Gathering Tests**
    *   **Focus:** Verifying that the `CodeVisitor` methods (`visit_item_*`) correctly identify and assemble the necessary context *before* they call `generate_synthetic_node_id` or `get_or_create_type`.
    *   **Method:** Integration tests (`tests/uuid_phase2_partial_graphs/visitor_context.rs` - *to be created*). These tests might involve running the visitor on small code snippets and asserting the state (`current_module_path`, `current_definition_scope.last()`, extracted `item_cfgs`, `current_scope_cfgs`) is correct at specific points, or asserting the arguments passed to mocked/instrumented generation functions.
    *   **Examples:**
        *   Test `visit_item_impl`: Assert that `current_definition_scope.last()` holds the correct parent module ID *before* the `impl_id` is pushed, and holds the `impl_id` *before* visiting methods. Assert `current_scope_cfgs` is updated correctly based on the `impl`'s attributes.
        *   Test `visit_item_fn`: Assert the correct `parent_scope_id` and `cfg_bytes` are passed when `generate_synthetic_node_id` is called for the function.
    *   **Goal:** Ensure the visitor correctly interprets the AST and manages its state. These tests act as diagnostics when paranoid tests fail – is the visitor providing the wrong input?

*   **Tier 3: Paranoid Helper "Canary" Tests**
    *   **Focus:** End-to-end validation of the ID generation pipeline for a *representative sample* of complex scenarios. These act as integration tests confirming the visitor and ID generation work together correctly.
    *   **Method:** A reduced subset of the existing tests in `tests/uuid_phase2_partial_graphs/nodes/`. These tests will use refactored paranoid helpers (`tests/common/paranoid/*`) that leverage Context Objects and centralized regeneration logic.
    *   **Examples:**
        *   `test_module_node_paranoid` (testing inline, file, declaration with nesting/CFGs).
        *   `test_struct_node_paranoid` (testing struct with generics, fields, CFGs).
        *   `test_impl_node_paranoid` (testing impl for generic struct with trait, methods, CFGs).
        *   One test per major node type, focusing on complexity (generics, nesting, CFGs).
    *   **Goal:** High-confidence "canary in the coal mine". If these pass, the core ID generation and visitor interaction are likely correct. Failures point towards issues potentially diagnosed by Tier 2 tests. Reduces the number of tests requiring exact UUID updates.

*   **Tier 4: Structural/Relational Tests**
    *   **Focus:** Verifying the overall structure of the `CodeGraph` and the relationships between nodes (`Contains`, `ImplementsTrait`, `ValueType`, `Method`, etc.).
    *   **Method:** Existing and future tests in `tests/uuid_phase2_partial_graphs/relations.rs` and potentially parts of `nodes/` tests that focus on connections rather than specific node fields. These tests use helpers like `assert_relation_exists`.
    *   **Examples:**
        *   Asserting a `ModuleNode` `Contains` the correct `FunctionNode`.
        *   Asserting an `ImplNode` `ImplementsTrait` the correct `TraitNode` (via TypeIds).
        *   Asserting a `FunctionNode` has `FunctionParameter` relations to the correct `TypeNode`s.
    *   **Goal:** Ensure the graph accurately represents the code's structure and connections. Relies implicitly on correct and unique IDs but doesn't usually assert their specific values.

### 2.3 Refactoring Test Helpers

*   **Context Objects:** Introduce structs like `NodeGenerationContext` to bundle the numerous arguments needed for ID regeneration. Paranoid helpers will construct this object.
*   **Centralized Regeneration:** Create central functions (e.g., in `uuid_ids_utils.rs`) like `regenerate_synthetic_id(context: &NodeGenerationContext) -> NodeId`. Paranoid helpers call this function instead of `ploke_core::NodeId::generate_synthetic` directly. This localizes the regeneration logic.
*   **`CodeGraph` Methods:** Add helper methods to `CodeGraph` and node structs (e.g., `get_parent_id`, `get_module_node`, iterators) to simplify context gathering within paranoid helpers and Phase 3 logic.

### 2.4 Error Handling

*   Define a crate-specific error enum (`SynParserError`) using `thiserror`.
*   Gradually replace `panic!`, `expect`, `unwrap` calls with `Result<T, SynParserError>` where appropriate (e.g., in helper functions, potentially in visitor logic where state might be inconsistent).
*   Use `?` for propagation.
*   Update tests to handle `Result` (e.g., `assert!(result.is_ok())`, `assert_matches!(result, Err(SynParserError::...))`).

## 3. Current Alignment & Next Steps

*   **Current State:** The test suite heavily relies on Tier 3 (Paranoid Helpers) with high brittleness. Tier 1 tests for core functions exist but could be expanded. Tier 2 tests are largely absent. Tier 4 tests exist for relations. Error handling is minimal (panics).
*   **Recent Fixes:** Addressed critical state management bugs (definition scope stack, CFG scope stack) identified by the existing paranoid tests. Fixed issues related to CFG string extraction and attribute filtering.
*   **Next Steps (Incremental):**
    1.  **(Done)** Achieve a fully passing test suite after recent CFG/stack fixes.
    2.  **Pilot Context Object/Centralization:** Refactor one paranoid helper set (e.g., `function_helpers.rs`) and associated tests to use `NodeGenerationContext` and a central `regenerate_synthetic_id` function.
    3.  **Expand Refactoring:** If the pilot is successful, refactor remaining paranoid helpers.
    4.  **Introduce Error Handling:** Define `SynParserError` and convert key panicking functions/helpers to return `Result`.
    5.  **Develop `CodeGraph` Methods:** Add helper methods as needed for test refactoring or Phase 3 preparation.
    6.  **Review Paranoid Test Coverage:** Once helpers are refactored, evaluate if the *number* of full paranoid tests can be reduced, relying more on the tiered strategy.
    7.  **Implement Tier 2 Tests:** Add explicit tests for visitor context gathering.
