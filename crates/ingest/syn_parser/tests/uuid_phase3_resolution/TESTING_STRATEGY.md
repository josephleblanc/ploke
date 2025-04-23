# syn_parser Phase 3 Resolution Testing Strategy

**Version:** 1.0
**Date:** 2025-04-23
**Status:** Proposed

## 1. Condensed Strategy

Ensure high confidence in the correctness of Phase 3 resolution logic (e.g., `shortest_public_path`, canonical path resolution, visibility checks) by testing against diverse and complex code structures represented in the `ModuleTree` and `CodeGraph`.

1.  **Module Tree Structure Verification:** Implicitly test the correctness of `ModuleTree` construction (`ResolvesToDefinition`, `CustomPath` relations) by relying on it as input for resolution tests. Failures in resolution tests may indicate underlying `ModuleTree` issues.
2.  **Core Resolution Function Unit Tests:** Unit test helper functions used within main resolution algorithms (e.g., path manipulation, visibility checking helpers) in isolation.
3.  **Shortest Public Path (SPP) Integration Tests:** Exhaustively test `ModuleTree::shortest_public_path` using dedicated fixtures (`fixture_path_resolution`, `fixture_spp_edge_cases`) covering:
    *   Basic visibility (`pub`, private).
    *   Restricted visibility (`pub(crate)`, `pub(super)`, `pub(in path)`) - expecting `Err`.
    *   Single and multi-step re-exports (including renaming `as`).
    *   Modules defined via `#[path]`.
    *   Glob re-exports (`*`).
    *   Complex interactions (e.g., re-exports of items from `#[path]` modules).
    *   Edge cases (deep chains, branching paths, shadowing, `cfg` interactions).
4.  **Canonical Path Resolution Tests (Future):** Once implemented, add tests similar to SPP tests to verify canonical path calculation.
5.  **Visibility Logic Tests:** While SPP tests cover public accessibility, add specific tests (if needed beyond SPP) verifying internal visibility logic (e.g., `ModuleTree::is_accessible`) for `pub(crate)`, `pub(super)`, etc., within the crate.
6.  **Fixture-Based Approach:** Utilize dedicated fixture crates to represent specific code structures and edge cases clearly. Maintain valid Rust code in primary fixtures (`fixture_crates/`) and potentially problematic code in `malformed_fixtures/`.
7.  **Acknowledge Phase 2 Limitations:** Tests must account for the state of the graph after Phase 2 (e.g., `cfg` attributes present but not fully evaluated). SPP tests, for instance, should verify the *syntactic* path based on the graph, even if `cfg` conflicts make that path impossible at compile time.

This strategy focuses on validating the resolution algorithms against realistic and challenging code patterns captured in the Phase 2 graph representation.

## 2. Detailed Elaboration

### 2.1 Rationale

Phase 3 resolution logic operates on the `CodeGraph` and `ModuleTree` produced by Phase 2 parsing. The correctness of this logic is crucial for generating stable identifiers (`CanonId`, `PubPathId`) and enabling accurate code analysis and querying. Key functions like `shortest_public_path` involve complex graph traversal, visibility rule application, and handling of Rust features like re-exports and `#[path]` attributes.

Testing must ensure these algorithms correctly interpret the graph structure and apply Rust's rules, even in complex edge cases. This requires dedicated fixtures designed to exercise specific scenarios.

### 2.2 Test Tiers Explained

*   **Tier 1: Module Tree Structure Verification (Implicit)**
    *   **Focus:** Ensuring the `ModuleTree` (built by `CodeGraph::build_module_tree`) accurately represents module relationships (`Contains`, `ResolvesToDefinition`, `CustomPath`).
    *   **Method:** Primarily implicit. If resolution tests (like SPP) fail unexpectedly, investigate potential errors in the underlying `ModuleTree` structure or the relations it uses. Explicit tests for `build_module_tree` could be added if specific structural bugs are suspected.
    *   **Goal:** Foundational correctness of the input data for resolution algorithms.

*   **Tier 2: Core Resolution Function Unit Tests**
    *   **Focus:** Testing small, reusable helper functions used within larger resolution algorithms (e.g., path joining/splitting, relative path resolution, specific visibility checks).
    *   **Method:** Standard Rust unit tests (`#[test]`) located near the function definitions (likely in `module_tree.rs` or utility modules).
    *   **Goal:** Verify the correctness of low-level building blocks.

*   **Tier 3: Shortest Public Path (SPP) Integration Tests**
    *   **Focus:** End-to-end testing of `ModuleTree::shortest_public_path`.
    *   **Method:** Integration tests located in `tests/uuid_phase3_resolution/shortest_path.rs`. Use helper macros (`assert_spp!`) and dedicated fixtures (`fixture_path_resolution`, `fixture_spp_edge_cases`).
    *   **Examples:** See `shortest_path.rs` and the documented scenarios in `fixture_spp_edge_cases/src/lib.rs`. Tests cover basic cases, re-exports, `#[path]`, visibility rules (expecting `Err` for non-public), and various edge cases.
    *   **Goal:** High confidence in SPP calculation, which is critical for `PubPathId` generation.

*   **Tier 4: Canonical Path Resolution Tests (Future)**
    *   **Focus:** End-to-end testing of canonical path resolution logic (once implemented).
    *   **Method:** Similar to SPP tests, using fixtures and potentially a dedicated test module (`tests/uuid_phase3_resolution/canonical_path.rs`).
    *   **Goal:** High confidence in canonical path calculation for `CanonId` generation.

*   **Tier 5: Visibility Logic Tests**
    *   **Focus:** Explicitly testing internal visibility rules beyond simple public access (e.g., `pub(crate)` access from within the crate).
    *   **Method:** Integration tests potentially using `ModuleTree::is_accessible` or similar functions, verifying access between different modules within a fixture.
    *   **Goal:** Ensure correct application of Rust's visibility rules for potential internal analysis tasks, complementing the external focus of SPP.

### 2.3 Fixture Strategy

*   **`fixture_path_resolution`:** Covers fundamental module structures, `#[path]`, basic re-exports, and `cfg` usage relevant to general path finding.
*   **`fixture_spp_edge_cases`:** Contains complex and potentially interacting scenarios specifically designed to stress test SPP logic (multi-step re-exports, inline `#[path]`, glob imports, conflicting `cfg`s, etc.). Must contain only valid, stable Rust.
*   **`malformed_fixture_spp_edge_cases`:** Contains code patterns that are invalid Rust or might cause issues like infinite loops (e.g., re-export cycles) to test the robustness and error handling of the resolution logic.

### 2.4 Handling Phase 2 Limitations

*   Phase 3 tests must operate on the graph as produced by Phase 2.
*   Crucially, tests involving `cfg` attributes should assert the expected outcome based on the *syntactic* graph structure and visibility, even if conflicting `cfg`s make a path impossible at compile time. For example, `shortest_public_path` *should* find the path `crate::foo::bar` for `impossible_item` in `#[cfg(a)] mod foo { #[cfg(not(a))] pub mod bar { pub fn impossible_item(); } }` if `foo` and `bar` are syntactically `pub`. This validates the traversal logic itself. Correct `cfg` filtering is a separate concern, likely for Phase 3 analysis or a `cfg`-aware SPP variant.
