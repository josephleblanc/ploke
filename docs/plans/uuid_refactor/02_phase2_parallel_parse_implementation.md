# Comprehensive Refactoring Plan: Implement Phase 2 - Parallel Parse & Provisional Graph Generation

### Post-implementation Notes:

**Minor Devaiations:**

*   No `generate_synthetic_type_id` helper directly on `VisitorState`; logic is in `type_processing.rs`.
*   Error handling uses `syn::Error` instead of a custom `Phase2Error`.
*   `TrackingHash` uses token stream `to_string()`, which might be less robust than a pure AST hash.

## 1. Task Definition
**Task**: Implement the "Parallel Parse & Provisional Graph Generation" phase (Phase 2) of the UUID refactoring plan, as outlined in the [UUID Refactor Overview](docs/plans/uuid_refactor/00_overview_batch_processing_model.md). This involves modifying the existing parallel file parsing logic (`analyze_files_parallel`) to use the `DiscoveryOutput` from Phase 1, generate temporary `NodeId::Synthetic` and `TypeId::Synthetic` UUIDs, calculate `TrackingHash` values, and produce partial `CodeGraph` structures containing this provisional data.
**Purpose**: To leverage parallelism (`rayon`) for the CPU-bound task of parsing Rust code into ASTs and initial graph nodes, while generating the necessary temporary identifiers and change-tracking hashes required for the sequential resolution (Phase 3) and incremental update logic.
**Success Criteria**:
   - The `analyze_files_parallel` function (under the `uuid_ids` flag) accepts `DiscoveryOutput` (or necessary parts like file lists and namespaces).
   - It uses `rayon` to parse files in parallel.
   - Each parser worker correctly uses the `CRATE_NAMESPACE` and file context passed to it.
   - The `CodeVisitor` (and its `VisitorState`) generates `NodeId::Synthetic(Uuid)` for graph nodes (functions, structs, etc.).
   - The `CodeVisitor` generates `TypeId::Synthetic(Uuid)` for type references, storing unresolved path strings where necessary.
   - The `CodeVisitor` generates `TrackingHash` (Uuid) for relevant nodes based on their content.
   - Relations within the partial graphs use the generated `Synthetic` UUIDs.
   - `analyze_files_parallel` returns a collection of `Result<CodeGraph, _>` where each `CodeGraph` contains nodes with `Synthetic` IDs and `TrackingHash` values.
   - All new/modified code related to UUIDs is gated behind the `uuid_ids` feature flag.
   - Unit tests verify synthetic ID and tracking hash generation logic.
   - Integration tests (using a simple fixture crate) verify the parallel execution and the structure of the output partial `CodeGraph`s.

## 2. Feature Flag Configuration
**Feature Name**: `uuid_ids` (already defined in ADR-001)

**Implementation Guide:**
- Continue gating all new/modified structs, functions, and logic related to UUID generation under `#[cfg(feature = "uuid_ids")]`.
- Use conditional compilation (`#[cfg(...)]`) within functions like `analyze_files_parallel` to separate the old `usize`-based path from the new UUID-based path.
- Ensure type definitions for `NodeId`, `TypeId`, etc., are conditionally compiled based on the flag.

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Review Phase 1 Output Structure (`DiscoveryOutput`, `CrateContext`)
    - **Purpose**: Confirm understanding of the data available from Phase 1 (file lists per crate, namespaces, initial module map).
    - **Expected Outcome**: Clear plan for how `analyze_files_parallel` will consume this data and distribute it to workers.
    - **Files to Examine**: `crates/ingest/syn_parser/src/discovery.rs`.
- [ ] 3.1.2. Define/Refine Core Identifier Types
    - **Purpose**: Finalize the definitions for `NodeId`, `TypeId`, `LogicalTypeId`, and `TrackingHash` under the `uuid_ids` flag. Address potential naming conflicts.
    - **Decision**: Use conditional compilation for the `NodeId` and `TypeId` type aliases/definitions. The existing names will refer to `usize` when the flag is off, and the new `Uuid`-based enum/struct when the flag is on. This avoids introducing temporary names like `NodeIdx` throughout the code.
    - **Expected Outcome**: Updated type definitions in `parser/types.rs` (or potentially a new `parser/ids.rs`).
    - **Files to Modify/Create**: `crates/ingest/syn_parser/src/parser/types.rs` (or new `crates/ingest/syn_parser/src/parser/ids.rs` and update `mod.rs`).
- [ ] 3.1.3. Define Phase 2 Error Handling
    - **Purpose**: Define specific error types that can occur during Phase 2 (e.g., parsing errors per file, ID generation failures).
    - **Expected Outcome**: Add variants to `error.rs` or create a new `Phase2Error` enum. Decide how errors from individual workers are collected and reported by `analyze_files_parallel`.
    - **Files to Modify/Create**: `crates/ingest/syn_parser/src/error.rs`.
- [ ] 3.1.4. Plan `VisitorState` Modifications
    - **Purpose**: Determine the necessary changes to `VisitorState` to support Phase 2 requirements.
    - **Expected Outcome**: List of fields to add/modify in `VisitorState` (e.g., `crate_namespace: Uuid`, `current_file_path: PathBuf`, methods for ID/hash generation).
    - **Files to Examine**: `crates/ingest/syn_parser/src/parser/visitor/state.rs`.

### 3.2 Core Implementation (Gated by `uuid_ids`)

-   **[ ] 3.2.1. Implement UUID-based Identifier Types**
    -   **Purpose**: Define the actual `enum NodeId { Path(Uuid), Synthetic(Uuid) }`, `struct TypeId { ... }`, etc., under `#[cfg(feature = "uuid_ids")]`. Define the `usize` versions under `#[cfg(not(feature = "uuid_ids"))]`.
    -   **Files to Modify**: `crates/ingest/syn_parser/src/parser/types.rs` (or new `ids.rs`).
    -   **Testing Approach**: Compile check with and without the flag.

-   **[ ] 3.2.2. Update Node & Graph Structures**
    -   **Purpose**: Modify all `*Node` structs (`FunctionNode`, `StructNode`, etc.) and `CodeGraph` fields to use the new `NodeId` and `TypeId` types *when the `uuid_ids` feature is enabled*. Add `tracking_hash: Option<Uuid>` field where appropriate.
    -   **Files to Modify**:
        -   `crates/ingest/syn_parser/src/parser/nodes.rs`
        -   `crates/ingest/syn_parser/src/parser/types.rs` (e.g., `GenericParamKind` uses `TypeId`)
        -   `crates/ingest/syn_parser/src/parser/relations.rs` (If `Relation` struct needs changes, though it likely uses `NodeId` which is handled by the type alias change).
        -   `crates/ingest/syn_parser/src/parser/graph.rs`
    -   **Testing Approach**: Compile check with the flag enabled.

-   **[ ] 3.2.3. Update `VisitorState`**
    -   **Purpose**: Implement the planned changes from 3.1.4. Add fields for `crate_namespace` and `current_file_path`. Modify `next_node_id`/`next_type_id` to track internal counters for synthetic ID generation *within a single file parse* if needed, but primarily focus on adding methods to generate UUIDs using context.
    -   **Files to Modify**: `crates/ingest/syn_parser/src/parser/visitor/state.rs`.
    -   **Testing Approach**: Compile check. Unit tests for new helper methods if complex logic is added.

-   **[ ] 3.2.4. Implement ID and Hash Generation Logic**
    -   **Purpose**: Create helper functions (likely methods on `VisitorState` or in a dedicated module) for:
        -   `generate_synthetic_node_id(&self, item_name: &str, span: (usize, usize)) -> NodeId` (using namespace, file path, name, span).
        -   `generate_synthetic_type_id(&self, type_str: &str) -> TypeId` (using namespace, type string).
        -   `generate_logical_type_id(&self, type_path: &[String]) -> LogicalTypeId` (using project namespace, crate name, type path - might be deferred slightly but plan for it).
        -   `generate_tracking_hash(&self, node_tokens: &TokenStream) -> Uuid` (using namespace, file path, token hash).
    -   **Files to Modify/Create**: `crates/ingest/syn_parser/src/parser/visitor/state.rs` (or new `ids.rs`).
    -   **Testing Approach**: Unit tests for each generation function with known inputs and expected (or at least consistent) outputs.

-   **STOP & TEST 1**: Unit test the ID and hash generation logic thoroughly.

-   **[ ] 3.2.5. Modify `CodeVisitor` (Incremental)**
    -   **Purpose**: Update the visitor logic to use the new ID/hash generation methods and handle type resolution appropriately for Phase 2. **Modify incrementally to manage complexity.**
    -   **Files to Modify**:
        -   `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
        -   `crates/ingest/syn_parser/src/parser/visitor/type_processing.rs`
    -   **Sub-steps:**
        -   **[ ] 5a: Update ID Generation Calls:** Replace `state.next_node_id()` and `add_contains_rel` logic with calls to `state.generate_synthetic_node_id(...)`. Ensure the necessary context (name, span) is passed.
        - 5a NOTE: This helper does *not* exist in `state.rs`. Type ID generation is handled within `get_or_create_type` in `type_processing.rs`, which calls `ploke_core::TypeId::generate_synthetic`. 

        -   **[ ] 5b: Update Type Handling:** Modify `get_or_create_type` in `type_processing.rs`. Instead of just returning an ID, it should:
            -   Attempt local resolution (within the current file's `type_map`).
            -   If found, return existing `Synthetic(Uuid)`.
            -   If not found, generate a *new* `TypeId::Synthetic(Uuid)` using `state.generate_synthetic_type_id(type_str)`.
            -   Store the unresolved type string (`type_str`) alongside the `TypeNode` or the reference to it, perhaps by modifying `TypeNode` or how `related_types` is used.
            -   Update direct `TypeId` usage in `code_visitor.rs` (e.g., `visit_item_type`, `visit_item_extern_crate`) accordingly.
        -   **[ ] 5c: Update Relation Creation:** Ensure `Relation` objects are created using the `NodeId::Synthetic` and `TypeId::Synthetic` values returned by the updated generation logic.
        -   **[ ] 5d: Add TrackingHash Generation:** In relevant `visit_*` methods (e.g., `visit_item_fn`, `visit_item_struct`), after creating the node, call `state.generate_tracking_hash(...)` using the item's `TokenStream` and store the result in the node's `tracking_hash` field.
        - 5d NOTE: **Deviation** Improve recent implementation. TrackingHash uses item_tokens.to_string(), making it sensitive to formatting.
   and comments within the token stream, not just the AST structure.
    -   **Testing Approach**: After each sub-step (5a-5d), attempt to parse the `simple_crate` fixture. Check intermediate results (e.g., print debug info) or add basic assertions if possible, focusing on the aspect just changed. Compile checks are essential.

-   **STOP & TEST 2**: After all `CodeVisitor` modifications, parse the `simple_crate` fixture. Verify (e.g., via RON serialization or debug printing) that:
    -   Nodes have `NodeId::Synthetic(Uuid)` IDs.
    -   Types have `TypeId::Synthetic(Uuid)` IDs.
    -   Unresolved type information (path strings) is preserved.
    -   Relations use synthetic UUIDs.
    -   `TrackingHash` values are present on relevant nodes.

-   **[ ] 3.2.6. Integrate Phase 2 into Entry Point (`analyze_files_parallel`)**
    -   **Purpose**: Implement the parallel execution logic using `rayon`. Distribute files and context from `DiscoveryOutput` to workers. Collect `Result<CodeGraph, _>` from each worker.
    -   **Files to Modify**: `crates/ingest/syn_parser/src/parser/visitor/mod.rs`.
    -   **Implementation Details**:
        -   Modify the function signature to accept `DiscoveryOutput` or relevant parts.
        -   Iterate through `discovery_output.crate_contexts`.
        -   For each crate, use `par_iter()` on its `context.files`.
        -   Inside the `map` operation:
            -   Create a `VisitorState` initialized with the `crate_context.namespace` and the current file path.
            -   Call a modified `analyze_code` function (or a new function like `analyze_code_phase2`) that takes the state and file path, performs the visit, and returns the `CodeGraph`.
            -   Handle `syn::Error` from parsing.
        -   Collect the results. Decide on error handling strategy (e.g., return `Vec<Result<CodeGraph, Phase2Error>>`, or collect errors separately).
    -   **Testing Approach**: Integration test using `simple_crate`.

### 3.3 Testing & Integration
- [ ] 3.3.1. Create `simple_crate` Fixture
    - **Location**: `tests/fixture_crates/simple_crate`
    - **Content**: A minimal crate with `src/lib.rs`, `Cargo.toml`, maybe one module (`src/mod1.rs`), a struct, a function, a type alias, and a `use` statement for an external (unparsed) crate and an internal item.
- [ ] 3.3.2. Add Unit Tests for ID/Hash Generation
    - Test consistency and uniqueness of synthetic IDs based on varying context (namespace, path, name, span, type string).
    - Test tracking hash generation consistency.
- [ ] 3.3.3. Add Integration Test for `analyze_files_parallel` (Phase 2)
    - Create a test function gated by `uuid_ids`.
    - Run Phase 1 on `simple_crate` to get `DiscoveryOutput`.
    - Call `analyze_files_parallel` with the discovery output.
    - Assert that the output is a `Vec<Result<CodeGraph, _>>` with the expected number of entries (one per file in `simple_crate`).
    - For each `Ok(graph)`:
        - Check that nodes (`graph.functions`, `graph.defined_types`, etc.) exist.
        - Assert that node IDs are `NodeId::Synthetic(uuid)`.
        - Assert that type IDs used in nodes are `TypeId::Synthetic(uuid)`.
        - Assert that `tracking_hash` fields are populated (`Some(uuid)`).
        - Assert that relations use `Synthetic` UUIDs.
        - (Optional) Serialize the graph to RON and compare against a snapshot (though UUIDs will change, structure should be consistent).
- [ ] 3.3.4. Test with and without feature flag enabled
    - Ensure existing `usize`-based tests still pass with the flag *disabled*.
    - Ensure new Phase 2 tests pass with the flag *enabled*.
    - Ensure compilation in both configurations.

### 3.4 Error Handling
- Define `Phase2Error` enum in `crates/ingest/syn_parser/src/error.rs`.
    - NOTE: **Deviation:**: Custom `Phase2Error` enum. The current implementation uses the existing `syn::Error` type to wrap file I/O errors and propagate parsing errors.
        - Should be improved.
- Include variants for:
    - `SynError(syn::Error)`: Wrap parsing errors from `syn`.
    - `IoError(std::io::Error)`: For file reading issues within the worker.
    - `IdGenerationError(String)`: If UUID generation itself fails (unlikely but possible).
- The `analyze_files_parallel` function should return `Vec<Result<CodeGraph, Phase2Error>>`. Errors from individual file processing are contained within the `Result` for that file.
- The caller of `analyze_files_parallel` will be responsible for handling the vector of results (e.g., logging errors, deciding whether to proceed to Phase 3 if some files failed).

### 3.5 Documentation & Knowledge Preservation
- [ ] 3.5.1. Update code documentation (doc comments) for modified structs and functions, explaining synthetic IDs, tracking hashes, and Phase 2 context.
- [ ] 3.5.2. Update this plan document (`02_...`) with any deviations or refinements discovered during implementation.
- [ ] 3.5.3. Add links to relevant ADRs ([ADR-003](docs/design/adrs/accepted/ADR-003-Defer-Dependency-Resolution.md), [ADR-004](docs/design/adrs/proposed/ADR-004-Parser-Scope-Resolution.md)) and the overview plan ([00_...](docs/plans/uuid_refactor/00_overview_batch_processing_model.md)).
- [ ] 3.5.4. Use descriptive commit messages referencing this plan and the `uuid_ids` feature flag.

## 4. Rollback Strategy
- Disable the `uuid_ids` feature flag to revert to the `usize`-based implementation.
- Use git commits to revert specific changes if major issues arise. The incremental modification approach for `CodeVisitor` should make rollbacks easier.

## 5. Progress Tracking (Example)
- [ ] Analysis Phase: 0/4 complete
- [ ] Implementation Phase: 0/6 complete
- [ ] Testing Phase: 0/4 complete
- [ ] Documentation Phase: 0/4 complete

## 6. Key Files & Structures Involved

-   **Files to Modify:**
    -   `crates/ingest/syn_parser/src/parser/types.rs` (or new `ids.rs`)
    -   `crates/ingest/syn_parser/src/parser/nodes.rs`
    -   `crates/ingest/syn_parser/src/parser/graph.rs`
    -   `crates/ingest/syn_parser/src/parser/relations.rs` (Potentially)
    -   `crates/ingest/syn_parser/src/parser/visitor/state.rs`
    -   `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
    -   `crates/ingest/syn_parser/src/parser/visitor/type_processing.rs`
    -   `crates/ingest/syn_parser/src/parser/visitor/mod.rs` (`analyze_files_parallel`)
    -   `crates/ingest/syn_parser/src/error.rs`
-   **Files to Create:**
    -   `tests/fixture_crates/simple_crate/` (Directory)
    -   `tests/fixture_crates/simple_crate/Cargo.toml`
    -   `tests/fixture_crates/simple_crate/src/lib.rs`
    -   `tests/fixture_crates/simple_crate/src/mod1.rs` (Optional)
    -   New test file for Phase 2 integration tests (e.g., `tests/integration/phase2_tests.rs`)
-   **Key Data Structures:**
    -   `NodeId` (Enum: `Path`/`Synthetic`)
    -   `TypeId` (Struct: `crate_id`, `type_id` - both Uuid)
    -   `LogicalTypeId` (Uuid)
    -   `TrackingHash` (Uuid)
    -   `VisitorState` (with added context)
    -   `CodeGraph` (containing nodes/relations with synthetic IDs)
    -   `DiscoveryOutput`, `CrateContext` (Input)
    -   `Phase2Error` (Enum)

This plan provides a detailed breakdown for implementing Phase 2, focusing on incremental changes and testing to manage complexity.
