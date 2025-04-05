# Phase 1 Discovery Implementation Review (UUID Refactor)

## Summary

This document reviews the implementation of Phase 1 (Discovery & Context Setup) of the UUID refactor for `ploke`, specifically within the `syn_parser` crate. The implementation adheres closely to the [original plan](01_phase1_discovery_implementation.md) and the [overall batch processing model](00_overview_batch_processing_model.md).

**Key Outcomes & Decisions:**

*   **Functionality:** Successfully implemented logic to discover `.rs` files, parse `Cargo.toml`, generate deterministic `CRATE_NAMESPACE` UUIDs, and perform a basic scan for module declarations (`mod xyz;`) in crate entry points (`lib.rs`/`main.rs`).
*   **Error Handling:** Adopted a fail-fast strategy using `Result<DiscoveryOutput, DiscoveryError>`. If any critical error occurs during the processing of a target crate (e.g., missing `Cargo.toml`, missing `src` directory, I/O error), the entire discovery phase returns `Err`, preventing subsequent phases from running with incomplete context. This prioritizes correctness for downstream phases over partial success reporting at this stage.
*   **Feature Gating:** All new code is correctly gated behind the `uuid_ids` feature flag.
*   **Testing:** Comprehensive unit and integration tests cover namespace generation, file discovery, TOML parsing, error conditions, and basic module scanning using the `fixture_test_crate`.

## Departures from Plans

*   **[01_phase1_discovery_implementation.md](01_phase1_discovery_implementation.md):** No significant departures. The implementation follows the task breakdown closely. The error handling strategy (returning `Result` vs. a tuple for partial success) was discussed and finalized during implementation to align better with the goal of Phase 1 acting as a gatekeeper.
*   **[00_overview_batch_processing_model.md](00_overview_batch_processing_model.md):** No departures. The implementation aligns with the description of Phase 1, providing the necessary inputs (file lists, namespaces, initial module map) for Phase 2.

## Input/Output Expectations for Integration

*   **`run_discovery_phase(project_root: &PathBuf, target_crates: &[PathBuf]) -> Result<DiscoveryOutput, DiscoveryError>`**
    *   **Input:**
        *   `project_root`: Path to the overall project root (currently unused but kept for potential future use).
        *   `target_crates`: A slice of **absolute paths** to the root directories of the crates to be analyzed. The function assumes these paths are valid directories.
    *   **Output (on Success - `Ok(DiscoveryOutput)`):**
        *   `DiscoveryOutput`: Contains context for *all* specified `target_crates`.
            *   `crate_contexts: HashMap<String, CrateContext>`: Maps crate names to their context.
                *   `CrateContext`: Includes name, version, derived `CRATE_NAMESPACE` UUID, root path, and a `Vec<PathBuf>` of *all* `.rs` files found within the crate's `src` directory.
            *   `initial_module_map: HashMap<PathBuf, Vec<String>>`: Maps file paths (like `src/module.rs` or `src/module/mod.rs`) identified via `mod module;` declarations in `lib.rs` or `main.rs` to their basic module path segments (e.g., `["crate", "module"]`).
    *   **Output (on Failure - `Err(DiscoveryError)`):**
        *   `DiscoveryError`: An enum indicating the *first* critical error encountered (e.g., `CratePathNotFound`, `Io`, `TomlParse`, `SrcNotFound`, `Walkdir`). Processing stops immediately upon encountering such an error.

## Known Limitations & Rationale

*   **Fail-Fast Error Handling:** As discussed, `run_discovery_phase` returns `Err` on the first critical failure. While returning partial results (`(DiscoveryOutput, Vec<DiscoveryError>)`) was considered, the `Result` approach was chosen because subsequent phases (like Phase 2 parallel parsing) strictly require complete and correct context for all target crates. Proceeding with partial context could lead to incorrect graph construction or runtime errors later. This design prioritizes system correctness. Non-critical errors (like individual file read errors during `scan_for_mods` if implemented differently) could potentially be collected, but errors preventing the basic context gathering for a crate are treated as critical.
*   **`scan_for_mods` Simplicity:** The current `scan_for_mods` function uses a basic line-by-line text scan for `mod name;`. It does *not* handle conditional compilation (`#[cfg] mod ...`) or modules declared inside other blocks. This is deemed sufficient for Phase 1's goal of providing an *initial* map; Phase 3 will build the definitive module tree using full AST parsing.
*   **Namespace Versioning:** `derive_crate_namespace` uses the full version string (e.g., "0.1.1"). Minor patch updates will result in a new namespace. This is simple but might lead to more cache invalidation than desired later. Future enhancements could consider using only major/minor versions based on configuration.
*   **Files Outside Module Tree:** The discovery correctly finds all `.rs` files, including those not declared in `mod` statements (e.g., `not_in_mod.rs`). These files are included in `CrateContext.files`. It is the responsibility of later phases (Phase 3) to determine which files are actually part of the compiled module tree. Phase 1 simply identifies candidates.
*   **Absolute Paths:** `run_discovery_phase` assumes it receives absolute paths for `target_crates`. Path resolution logic is expected to be handled by the caller (e.g., UI or CLI).

## Testing Overview

*   **Location:** Tests reside in `crates/ingest/syn_parser/tests/uuid_phase1_discovery/`.
    *   `mod.rs`: Declares the test module.
    *   `discovery_tests.rs`: Contains unit and integration tests.
*   **Coverage:**
    *   **Unit Tests:** Cover `derive_crate_namespace` consistency and uniqueness, and various scenarios for `run_discovery_phase` using temporary directories (valid crate, missing/invalid `Cargo.toml`, missing `src`, non-existent crate path, multiple crates with one failing).
    *   **Integration Test:** `test_discovery_on_fixture_crate` uses the `fixture_test_crate` (located at `workspace_root/fixture_test_crate`) to verify the discovery process on a more complex, realistic structure. It checks:
        *   Correct `CrateContext` generation (name, version, namespace).
        *   Discovery of *all* `.rs` files within `src`, including nested ones and those not in the module tree.
        *   Correct population of the `initial_module_map` based on `mod` declarations in `main.rs`.
*   **Gating:** All tests in this module are gated by `#[cfg(feature = "uuid_ids")]`.

This concludes the implementation and initial review of Phase 1. The foundation is set for proceeding to Phase 2.
