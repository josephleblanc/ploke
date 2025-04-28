# syn_parser Phase 3 Resolution Testing Strategy

**Version:** 2.0
**Date:** 2025-04-23
**Status:** Adopted

## 1. Condensed Strategy

Ensure high confidence in the correctness and robustness of Phase 3 logic (`ModuleTree` construction, path resolution, visibility checks) through a tiered testing approach focused on structural integrity, diagnostic checks, complex scenarios, and explicit error handling validation.

1.  **Tier 1: Core Function Unit Tests:** Exhaustively test critical *helper* functions used within Phase 3 algorithms (e.g., path manipulation, visibility helpers, relation filtering) in isolation.
2.  **Tier 2: Diagnostic & Invariant Tests:** Verify key structural invariants and properties of the `ModuleTree` *after* `build_module_tree` completes (or fails gracefully). These tests are not exhaustive of all build logic but act as diagnostics for higher-tier failures. Examples:
    *   All `ModuleNode`s from the input `CodeGraph` are present in the `ModuleTree.modules` map.
    *   All `Contains` relations from the `CodeGraph` involving modules are reflected in `ModuleTree.tree_relations`.
    *   All non-root, file-based `ModuleNode`s are linked via `ResolvesToDefinition` or `CustomPath`.
    *   No unexpected `NodeId::Synthetic` remain for `ModuleNode`s.
3.  **Tier 3: Canary Tests:** Use complex fixtures (`fixture_spp_edge_cases`) to perform detailed, brittle checks on specific items. Verify expected paths (SPP, canonical), relations, and properties for a few representative items in challenging scenarios (deep re-exports, nested `#[path]`, complex `cfg` interactions). These act as high-level integration checks.
4.  **Tier 4: Error Handling & Propagation Tests:** Explicitly test the error handling logic within `build_module_tree` and resolution functions. Verify:
    *   Fatal errors correctly halt processing and return appropriate `Err` variants.
    *   Recoverable errors/warnings (e.g., unlinked modules, external `#[path]`) are handled correctly (logged, potentially returned in a specific non-fatal `Err` variant) without halting unnecessarily.
    *   Error types clearly distinguish between states indicating an invalid/corrupt graph (unsafe for DB update) and states representing known limitations or recoverable issues.

This strategy balances rigorous validation of core logic and complex interactions with diagnostic checks for maintainability and explicit validation of critical error handling paths.

## 2. Detailed Elaboration

### 2.1 Rationale

Phase 3 transforms the syntactically-focused `CodeGraph` from Phase 2 into a `ModuleTree` and enables resolution logic (like `shortest_public_path` and canonical path finding) crucial for generating stable IDs (`PubPathId`, `CanonId`). The process involves complex graph interpretation, relation building, visibility checks, and error handling. Testing must ensure not only the correctness of the final outputs (e.g., paths) but also the structural integrity of the intermediate `ModuleTree` and the robustness of the error handling mechanisms, particularly concerning the safety of downstream database operations.

### 2.2 Test Tiers Explained

*   **Tier 1: Core Function Unit Tests**
    *   **Focus:** Exhaustive testing of small, pure, or easily mockable helper functions used within `build_module_tree` or resolution algorithms (e.g., `ModuleTree::resolve_relative_path`, path comparison logic, specific relation filtering helpers).
    *   **Method:** Standard Rust unit tests (`#[test]`) located near function definitions.
    *   **Goal:** Highest rigor on fundamental algorithms and utility functions.

*   **Tier 2: Diagnostic & Invariant Tests**
    *   **Focus:** Verifying the structural integrity and key properties of the `ModuleTree` *after* calling `CodeGraph::build_module_tree`. These tests act as crucial diagnostics when Tier 3 (Canary) tests fail, helping pinpoint whether the issue lies in the resolution logic itself or in the underlying tree structure provided to it.
    *   **Method:** Integration tests (e.g., in `tests/uuid_phase3_resolution/invariants.rs` - *to be created*). These tests run `build_module_tree` on various fixtures and assert properties of the resulting `ModuleTree`.
    *   **Examples:**
        *   Assert `tree.modules.len()` matches the count of `ModuleNode`s in the input `CodeGraph`.
        *   Assert every `ModuleNodeId` from the input graph exists as a key in `tree.modules`.
        *   Assert every `Contains` relation between modules in the input graph exists in `tree.tree_relations`.
        *   Assert every non-root, file-based `ModuleNode` in `tree.modules` has an incoming `ResolvesToDefinition` or `CustomPath` relation in `tree.tree_relations`.
        *   Assert `tree.path_index` and `tree.decl_index` correctly map paths to the expected `NodeId`s based on the fixture.
    *   **Goal:** Ensure `build_module_tree` produces a structurally sound and complete representation. Detect regressions in tree construction logic. Provide diagnostic information for failures in path resolution or visibility tests.

*   **Tier 3: Canary Tests**
    *   **Focus:** End-to-end validation of resolution logic (SPP, canonical path, visibility) for a *representative sample* of complex scenarios using highly detailed assertions.
    *   **Method:** Integration tests (e.g., `shortest_path.rs`, `canonical_path.rs`) using complex fixtures (`fixture_spp_edge_cases`). Tests select specific, challenging items within the fixture and assert:
        *   The exact expected shortest public path (`Ok(["crate", "mod", ...])`).
        *   The exact expected canonical path.
        *   The presence and correctness of specific relations involving the item or its containing modules within the `ModuleTree`.
        *   Expected visibility results (`is_accessible`).
    *   **Examples:** Test SPP for `final_deep_item`, `item_in_nested_target_2`, an item accessed via glob import, `impossible_item`.
    *   **Goal:** High-confidence "canary in the coal mine" for complex interactions. Failures indicate potential issues in resolution logic or underlying tree structure (diagnosed by Tier 2). These tests are intentionally brittle to catch subtle regressions.

*   **Tier 4: Error Handling & Propagation Tests**
    *   **Focus:** Explicitly verifying the error handling behavior of `build_module_tree` and resolution functions.
    *   **Method:** Integration tests (e.g., in `tests/uuid_phase3_resolution/error_handling.rs` - *to be created*) using both valid and malformed fixtures (`malformed_fixture_spp_edge_cases`). Tests trigger specific error conditions and assert:
        *   `build_module_tree` returns the expected `Err(SynParserError::...)` variant for fatal errors (e.g., `DuplicatePath`, `DuplicateModuleId`, critical internal errors).
        *   `build_module_tree` returns `Ok(tree)` even when non-fatal issues occur (e.g., `FoundUnlinkedModules`), potentially logging warnings.
        *   Resolution functions (like SPP) return expected errors (`ItemNotPubliclyAccessible`, `ExternalItemNotResolved`, `ReExportChainTooLong`).
        *   Error types clearly map to whether the resulting graph/tree state is considered valid for database updates.
    *   **Examples:** Test `build_module_tree` with fixtures containing duplicate module definitions, unlinked files, external `#[path]` targets. Test SPP with fixtures containing re-export cycles or private items.
    *   **Goal:** Ensure predictable and safe error handling, preventing invalid states from propagating downstream (especially to the database) and correctly handling expected limitations.

### 2.3 Fixture Strategy

*   Utilize dedicated fixtures (`fixture_path_resolution`, `fixture_spp_edge_cases`, `malformed_fixture_spp_edge_cases`, etc.) to isolate and clearly demonstrate specific scenarios for different test tiers.
*   Maintain valid, stable Rust in primary fixtures (`fixture_crates/`).
*   Use `malformed_fixtures/` specifically for testing error handling and robustness against invalid code patterns.

### 2.4 Testing Philosophy & Handling Failures

*   **No Tests Confirming Undesired States:** Tests must assert the *desired* behavior. Tests for unimplemented features *must fail* initially.
*   **Handling Known Limitations:** If a failing test represents a feature deliberately deferred or a known limitation deemed acceptable *and* non-corrupting:
    1.  Document the limitation thoroughly in `docs/design/known_limitations/`, linking to the specific test(s).
    2.  Mark the test(s) with `#[ignore = "Reason for ignoring (e.g., Known Limitation: XYZ - See docs/...)"]`.
    3.  Add a comment within the test explaining the situation and linking to the documentation.
    4.  Consider creating an ADR (`docs/design/decision_tracking/`) proposing a future solution or formally accepting the limitation.
*   **Prioritize Graph Integrity:** Error handling tests must rigorously verify that scenarios potentially leading to inconsistent or invalid graph states result in errors that clearly signal "do not update database".
