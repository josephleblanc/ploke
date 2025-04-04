# Plan Overview: UUID Refactor using Multi-Phase Batch Processing

## 1. Goal

Refactor the `ploke` codebase, primarily `syn_parser`, to use stable, deterministic `uuid::Uuid` identifiers (`NodeId`, `TypeId`, `LogicalTypeId`) instead of `usize`. This enables reliable graph merging, parallel processing, and efficient incremental updates. This plan details the initial implementation using a multi-phase batch processing model.

## 2. Core Identifier Strategy (See ADR-001)

-   **Identifiers:** `NodeId` (Enum: `Path(Uuid)`/`Synthetic(Uuid)`), `TypeId` (Struct: `crate_id: Uuid`, `type_id: Uuid`), `LogicalTypeId` (Uuid), `TrackingHash` (Uuid).
-   **Generation:** UUIDv5 based on hierarchical namespaces (`PROJECT -> CRATE -> ITEM/TYPE`).
-   **Storage:** Native `cozo::DataValue::Uuid` in CozoDB.
-   **Feature Flag:** `uuid_ids`.

## 3. Multi-Phase Batch Processing Model

This model breaks down the parsing, resolution, and database insertion process into distinct, sequential phases, allowing for parallelism within specific phases.

### Phase 1: Discovery & Context Setup

-   **Input:** Project root path, list of target crates (workspace members + specified dependencies).
-   **Process:**
    1.  Identify all `.rs` files within target crates.
    2.  Parse `Cargo.toml` for each crate to determine `crate_name` and `crate_version`.
    3.  Perform a minimal scan (`lib.rs`, `main.rs`, `mod.rs`) to build an initial map of file paths to potential module segments (e.g., `src/module/file.rs` might belong to `crate::module`).
    4.  Define the global `PROJECT_NAMESPACE: Uuid`.
    5.  Derive `CRATE_NAMESPACE: Uuid` for each target crate using `PROJECT_NAMESPACE`, `crate_name`, and `crate_version`.
-   **Output:**
    -   List of `.rs` files to parse per crate.
    -   Map of `crate_name` -> `CRATE_NAMESPACE`.
    -   Initial (potentially incomplete) module structure map.
-   **Concurrency:** Can potentially parallelize file discovery and `Cargo.toml` parsing per crate.

### Phase 2: Parallel Parse & Provisional Graph Generation

-   **Input:** List of files per crate, `CRATE_NAMESPACE` for each crate.
-   **Process (Parallel per file using `rayon`):**
    1.  Each worker parses its assigned `.rs` file using `syn`.
    2.  A `VisitorState` tracks context *within the current file* (e.g., relative module path).
    3.  **`NodeId` Generation:**
        -   For defined items (functions, structs, etc.), generate a *temporary* `NodeId::Synthetic(Uuid)` using `CRATE_NAMESPACE`, file path, relative path/span, and item name. Store the context needed for final `Path` ID calculation (relative path, name) with the node.
    4.  **`TypeId` Generation:**
        -   When encountering type paths (`syn::TypePath`), attempt local resolution.
        -   If unresolved, store the path string (e.g., `foo::Bar`) as an unresolved reference marker within the node using it (e.g., field, parameter). Generate a *temporary* `TypeId::Synthetic(Uuid)` based on the unresolved string + `CRATE_NAMESPACE`.
    5.  **`Relation` Generation:** Create `Relation` objects using the *temporary* `Synthetic` IDs.
    6.  **`TrackingHash` Generation:** Calculate the `TrackingHash` based on AST tokens for relevant nodes.
-   **Output (per worker):** A partial `CodeGraph` containing:
    -   Full node details (`FunctionNode`, `StructNode`, etc.).
    -   Temporary `NodeId::Synthetic` and `TypeId::Synthetic` identifiers.
    -   Unresolved type reference markers.
    -   `Relation`s using temporary IDs.
    -   `TrackingHash` values.
-   **Concurrency:** High degree of parallelism via `rayon` across files.

### Phase 3: Batch Resolution (Sequential)

-   **Input:** Collection of all partial `CodeGraph`s from Phase 2, initial module map and namespaces from Phase 1.
-   **Process (Single-threaded or coarse-grained locking if state is shared):**
    1.  **Merge Graphs:** Combine all partial `CodeGraph`s into a single, potentially large, in-memory representation (or use iterators/maps to avoid one giant structure if memory is a concern).
    2.  **Build Definitive Module Tree:** Use all parsed `mod` items and the initial map to construct the final, accurate module hierarchy (mapping absolute module paths to file locations and contained items).
    3.  **Finalize `NodeId`s:**
        -   Iterate through all nodes with temporary `Synthetic` IDs.
        -   Using the definitive module tree and the stored context (relative path, name), calculate the absolute item path.
        -   Generate the final `NodeId::Path(Uuid)`.
        -   Build and maintain a mapping: `TemporarySynthId -> FinalPathId`. Update the node's ID in the merged representation.
    4.  **Resolve Types & Finalize `TypeId`s:**
        -   Create a lookup map: `absolute_type_path_string -> (FinalPathId, FinalTypeId, FinalLogicalTypeId)`.
        -   Iteratively process nodes containing unresolved type references:
            *   Use `use` statements (parsed in Phase 2) and the module context to resolve the reference path string to an absolute path.
            *   Look up the absolute path in the map to find the defining item's final IDs.
            *   Generate the final `TypeId` (struct) and `LogicalTypeId` (Uuid) for the resolved type.
            *   Replace the temporary `TypeId::Synthetic` and unresolved markers with the final IDs.
        -   Repeat resolution attempts until no further progress is made (handles dependency chains and cycles).
    5.  **Update Relations:** Iterate through all `Relation` objects. Use the `TemporarySynthId -> FinalPathId` map to replace temporary source/target IDs with final `NodeId::Path` or `TypeId` values. Discard relations that could not be fully resolved.
-   **Output:**
    -   A fully resolved `CodeGraph` (or equivalent data structures) with final `NodeId::Path`, `TypeId`, `LogicalTypeId`, `TrackingHash`, and resolved `Relation`s.
    -   **Persisted State:** The definitive module tree and the `TemporarySynthId -> FinalPathId` maps.

### Phase 4: Embedding Generation

-   **Input:** Resolved `CodeGraph` (specifically, nodes with code content and their `LogicalTypeId`).
-   **Process:**
    1.  Identify nodes requiring embedding (e.g., functions, structs).
    2.  Extract relevant code text/snippets.
    3.  For each snippet, call `ploke-embed` (which interfaces with external models like Ollama/Candle) to generate a vector embedding.
    4.  Associate the generated vector with the node's `LogicalTypeId`.
-   **Output:** A map of `LogicalTypeId -> Vec<f32>`.
-   **Concurrency:** Can be parallelized per node/snippet, potentially limited by external embedding model throughput.

### Phase 5: Transform & Insert (Database Interaction)

-   **Input:** Resolved `CodeGraph`, Map of `LogicalTypeId -> Vec<f32>`.
-   **Process (`ploke-graph` crate):**
    1.  Define CozoDB schema using native `Uuid`, `Vector`, etc. Create necessary indices (standard on IDs, HNSW on vectors).
    2.  Iterate through the resolved `CodeGraph` nodes and relations.
    3.  Transform Rust structs (`FunctionNode`, `Relation`, etc.) and associated embeddings into `cozo::DataValue` representations (using `DataValue::Uuid`, `DataValue::Vector`).
    4.  Generate CozoScript `:put` statements (upserts) for nodes, relations, and embeddings (linking embeddings via `LogicalTypeId`).
    5.  Execute CozoScript against the embedded CozoDB instance, likely in batches for efficiency. Handle potential transaction errors.
-   **Output:** Populated CozoDB instance.
-   **Concurrency:** Can potentially parallelize transformation and batch insertion into CozoDB (leveraging Cozo's MVCC).

## 4. Incremental Updates

-   When file changes are detected:
    1.  Load the **Persisted State** (module tree, ID maps) from the previous run.
    2.  Run **Phase 1** only if `Cargo.toml` or crate structure changed significantly.
    3.  Run **Phase 2** (Parallel Parse) only for the changed files and potentially directly dependent files/crates.
    4.  Run **Phase 3** (Batch Resolution), using the loaded state as a starting point. Only new/changed items need full resolution; existing IDs can be reused. Update the persisted state.
    5.  Run **Phase 4** (Embed) only for items whose `TrackingHash` changed or are new, using `LogicalTypeId`.
    6.  Run **Phase 5** (Transform/Insert) to update CozoDB with changes.

This model provides a structured way to implement the UUID refactor while enabling parallelism and laying the groundwork for efficient incremental updates.
