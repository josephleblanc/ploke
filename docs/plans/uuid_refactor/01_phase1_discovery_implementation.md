# Comprehensive Refactoring Plan: Implement Phase 1 - Discovery & Context Setup

## 1. Task Definition
**Task**: Implement the initial "Discovery & Context Setup" phase (Phase 1) of the UUID refactoring plan, as outlined in [ADR-002](docs/design/adrs/proposed/ADR-002-uuid-phase1-discovery.md) and the [UUID Refactor Overview](docs/plans/uuid_refactor/00_overview_batch_processing_model.md#phase-1-discovery--context-setup). This involves finding relevant Rust files, parsing crate metadata, generating namespaces, and creating an initial module map.
**Purpose**: To gather the necessary prerequisite information (file lists, namespaces, crate context) required before the parallel parsing (Phase 2) can commence. This ensures that Phase 2 workers have the context needed to start generating preliminary UUIDs.
**Success Criteria**:
    - A function exists (e.g., `run_discovery_phase(project_root: &Path, target_crates: &[String]) -> Result<DiscoveryOutput, DiscoveryError>`) that performs the phase's tasks.
    - The function correctly identifies all `.rs` files within specified target crates (workspace members + selected dependencies).
    - The function correctly parses `Cargo.toml` for each target crate and extracts name and version.
    - A constant `PROJECT_NAMESPACE` UUID is defined and used.
    - A unique `CRATE_NAMESPACE` UUID is correctly generated for each target crate.
    - An initial module map (file path -> potential module path segments) is generated.
    - The output `DiscoveryOutput` struct contains the file lists, namespaces, and module map.
    - All new code is gated behind the `uuid_ids` feature flag.
    - Unit tests verify file discovery, `Cargo.toml` parsing, namespace generation, and the structure of `DiscoveryOutput`.

## 2. Feature Flag Configuration
**Feature Name**: `uuid_ids` (already defined in ADR-001)

**Implementation Guide:**
```rust
// Example of gating new structs/functions
#[cfg(feature = "uuid_ids")]
pub struct DiscoveryOutput {
    // ... fields ...
}

#[cfg(feature = "uuid_ids")]
pub fn run_discovery_phase(/*...*/) -> Result<DiscoveryOutput, DiscoveryError> {
    // ... implementation ...
}

// Example within existing code (if modifying entry points)
pub fn analyze_files_parallel(/*...*/) -> Result</*...*/> {
    #[cfg(feature = "uuid_ids")]
    {
        // Call new discovery phase first
        let discovery_output = run_discovery_phase(/*...*/) ?;
        // Proceed to Phase 2 using discovery_output...
        todo!("Implement Phase 2 call");
    }

    #[cfg(not(feature = "uuid_ids"))]
    {
        // Existing usize-based implementation
        // ...
    }
}
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [~] 3.1.1. Review existing file processing entry points
  - **Purpose**: Understand how files are currently discovered and passed to the parser (`analyze_files_parallel`, `analyze_code`). Identify where Phase 1 needs to be integrated.
  - **Expected Outcome**: Clear understanding of current file handling logic.
  - **Files to Examine**:
    - `crates/ingest/syn_parser/src/parser/visitor/mod.rs`
    - Potentially CLI or main application entry points if they initiate parsing.
- [~] 3.1.2. Define Core Data Structures
  - **Purpose**: Define the Rust structs needed to hold the output of Phase 1 and related context.
  - **Expected Outcome**: Draft definitions for `DiscoveryOutput`, `CrateContext` (containing name, version, namespace, file list), `DiscoveryError`.
  - **Files to Modify/Create**: Likely a new module, e.g., `crates/ingest/syn_parser/src/discovery.rs` or similar.
- [x] 3.1.3. Add Dependencies
  - **Purpose**: Add necessary crates like `uuid` (with `v5`, `serde` features) and `toml`.
  - **Expected Outcome**: Updated `Cargo.toml` for `syn_parser` (and potentially a new discovery crate if we create one).
  - **Files to Modify**: `crates/ingest/syn_parser/Cargo.toml`.

### 3.2 Core Implementation (Gated by `uuid_ids`)
- [x] 3.2.1. Implement File Discovery Logic
  - **Purpose**: Walk directory trees for specified target crates and collect all `.rs` file paths.
  - **Files to Modify/Create**: New discovery module/functions.
  - **Reasoning**: Use standard library (`std::fs`) or crates like `walkdir` for robust directory traversal. Handle potential I/O errors.
  - **Testing Approach**: Unit test with mock directory structures or temporary directories containing sample files. Test exclusion of non-`.rs` files.
- [x] 3.2.2. Implement `Cargo.toml` Parsing
  - **Purpose**: Read and parse `Cargo.toml` for each target crate to extract `package.name` and `package.version`.
  - **Files to Modify/Create**: New discovery module/functions.
  - **Reasoning**: Use the `toml` crate for parsing. Handle file not found and parsing errors gracefully.
  - **Testing Approach**: Unit test with sample valid and invalid `Cargo.toml` content.
- [x] 3.2.3. Implement Namespace Generation
  - **Purpose**: Define `PROJECT_NAMESPACE` and implement the logic to derive `CRATE_NAMESPACE` using `Uuid::new_v5`.
  - **Files to Modify/Create**: New discovery module/functions. Define the constant `PROJECT_NAMESPACE`.
  - **Code Changes**:
    ```rust
    // Example (constants likely defined elsewhere)
    #[cfg(feature = "uuid_ids")]
    const PROJECT_NAMESPACE_UUID: Uuid = uuid!("..."); // Define actual UUID

    #[cfg(feature = "uuid_ids")]
    fn derive_crate_namespace(name: &str, version: &str) -> Uuid {
        let name_version = format!("{}@{}", name, version);
        Uuid::new_v5(&PROJECT_NAMESPACE_UUID, name_version.as_bytes())
    }
    ```
  - **Testing Approach**: Unit test `derive_crate_namespace` with known inputs and expected UUID outputs. Verify different names/versions produce different UUIDs.
- [x] 3.2.4. Implement Initial Module Mapping (Basic) (Integrated)
  - **Purpose**: Perform a minimal scan of key files (`lib.rs`, `main.rs`, `mod.rs`) to identify `mod my_module;` declarations and associate them with potential file paths (`src/my_module.rs` or `src/my_module/mod.rs`).
  - **Files to Modify/Create**: New discovery module/functions.
  - **Reasoning**: This provides a starting point for Phase 3 resolution. It doesn't need to be perfect yet. Use simple string matching or basic `syn` parsing focused only on `mod` items.
  - **Testing Approach**: Unit test with sample file structures and `mod` declarations.
- [~] 3.2.5. Integrate into Entry Point(s) (Still stubbed in `visitor/mod.rs`)
  - **Purpose**: Modify existing parser entry points (like `analyze_files_parallel`) to call `run_discovery_phase` first when the `uuid_ids` feature is enabled.
  - **Files to Modify**: `crates/ingest/syn_parser/src/parser/visitor/mod.rs`.
  - **Reasoning**: Ensure the discovery output is available before Phase 2 starts. (Note: Actual call logic is stubbed, pending signature changes/context provision).
  - **Testing Approach**: Integration tests (under the flag) should verify that the discovery phase runs and its output is potentially passed along (even if Phase 2 isn't implemented yet).

### 3.3 Testing & Integration
- [x] 3.3.1. Add Unit Tests for Discovery Components
    - Test file walking logic.
    - Test `Cargo.toml` parsing (valid, invalid, missing).
    - Test namespace generation logic.
    - Test initial module mapping logic.
    - Test `DiscoveryOutput` struct creation.
- [x] 3.3.2. Add Integration Test for `run_discovery_phase`
    - Use a sample project structure (perhaps `fixture_test_crate` or a dedicated test directory structure).
    - Run the full discovery phase and assert the correctness of the final `DiscoveryOutput` (file lists, namespaces, etc.).
- [~] 3.3.3. Test with and without feature flag enabled
    - Ensure existing tests pass with the flag *disabled*.
    - Ensure new tests pass with the flag *enabled*.
    - Ensure the code compiles correctly in both configurations.

### 3.4 Documentation & Knowledge Preservation
- [x] 3.4.1. Update code documentation (doc comments) for new structs and functions, explaining their purpose in Phase 1.
- [ ] 3.4.2. Document design decisions within this plan and the ADR.
- [ ] 3.4.3. Create commit message template capturing key changes (e.g., "feat(syn_parser): Implement file discovery for UUID Phase 1 [uuid_ids]").

## 4. Rollback Strategy
- Disable the `uuid_ids` feature flag.
- Revert commits related to this phase if necessary.

## 5. Progress Tracking
- [x] Analysis Phase: 3/3 complete
- [~] Implementation Phase: 4/5 complete (Integration stubbed)
- [ ] Testing Phase: 0/3 complete
- [ ] Documentation Phase: 0/3 complete

## 6. Phase 1 Data Flow Diagram

```mermaid
graph TD
    subgraph Phase 1: Discovery & Context Setup
        A[Input: Project Root, Target Crates] --> B{Iterate Target Crates};
        B -- Crate Path --> C[Find Cargo.toml];
        C -- Path --> D[Parse Cargo.toml];
        D -- Name, Version --> E[Derive CRATE_NAMESPACE];
        B -- Crate Path --> F[Walk Directory for .rs Files];
        F -- .rs File List --> G[Store Crate File List];
        E --> H[Store Crate Context (Name, Version, Namespace)];
        G --> H;
        F -- Key Files (lib/main/mod.rs) --> I[Scan for 'mod' declarations];
        I --> J[Build Initial Module Map];
        H --> K[Collect All CrateContexts];
        J --> K;
        K --> L[Output: DiscoveryOutput (File Lists, Namespaces, Module Map)];
    end
```

## 7. Key Data Structures / Files

-   **New/Modified Files:**
    -   `docs/design/adrs/proposed/ADR-002-uuid-phase1-discovery.md` (New ADR)
    -   `docs/plans/uuid_refactor/01_phase1_discovery_implementation.md` (This Plan)
    -   `crates/ingest/syn_parser/src/discovery.rs` (New module, likely)
    -   `crates/ingest/syn_parser/src/lib.rs` (To declare the new module)
    -   `crates/ingest/syn_parser/Cargo.toml` (Add dependencies `uuid`, `toml`)
    -   `crates/ingest/syn_parser/src/parser/visitor/mod.rs` (Integrate Phase 1 call)
    -   New test files for discovery logic.
-   **New Data Structures (Conceptual):**
    -   `DiscoveryOutput { crate_contexts: Vec<CrateContext>, initial_module_map: HashMap<PathBuf, Vec<String>> }`
    -   `CrateContext { name: String, version: String, namespace: Uuid, files: Vec<PathBuf> }`
    -   `DiscoveryError` (Enum for I/O, parsing errors)
