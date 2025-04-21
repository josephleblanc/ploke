# Ploke Project: syn_parser Working Context

**Last Updated:** 2025-04-21

**Current Focus:** Enhancing `shortest_public_path` in `module_tree.rs` to handle re-exports correctly.

## Core Goals & Architecture

1.  **ID Resolution:** The primary goal after Phase 2 parsing is to resolve temporary `NodeId::Synthetic` and `TypeId::Synthetic` identifiers into stable `NodeId::Resolved` and `TypeId::Resolved` identifiers.
    *   *(Update: We now have more specific resolved ID types: `CanonId` based on canonical path and `PubPathId` based on shortest public path).*
2.  **Incremental Parsing:** The system must support incremental parsing. After initial parsing, a file watcher will trigger reparsing of only changed files.
3.  **Change Detection:** To support incremental updates, we need mechanisms to:
    *   Map `NodeId::Synthetic` from new partial graphs to existing `NodeId::Resolved`.
    *   Use `TrackingHash` to detect if the content of an existing node has changed, requiring updates.
    *   Identify genuinely new items (no existing path or synthetic ID).
    *   Identify deleted items (path/synthetic ID existed previously but not in the new partial graph).
    *   Identify removed relationships (e.g., a `Contains` relation source exists, but the target no longer does).
4.  **Data Flow:**
    *   Phase 1 (Discovery) -> Phase 2 (Parallel Parsing -> `Vec<ParsedCodeGraph>`) -> Merge -> `CodeGraph` (with Synthetic IDs).
    *   Phase 3 (Resolution): Build `ModuleTree` from `CodeGraph`, resolve paths, calculate shortest public paths, resolve Synthetic IDs to Resolved IDs (`CanonId`, `PubPathId`).
    *   The final `CodeGraph` (with Resolved IDs) is passed to `ploke-graph` for database ingestion.
5.  **Minimal State:** After initial parsing and ID resolution, the `ModuleTree` might be consumed to create a smaller data structure holding only the essential information for incremental parsing (e.g., Synthetic -> Resolved ID mappings, `TrackingHash`es, path index).
6.  **Dependency Handling:** The ID resolution strategy (especially using `PubPathId` derived from the shortest public path) is crucial for linking items across crate boundaries (user code <-> dependencies) when parsing them independently. `cfg` flags need careful handling for this cross-crate linking to work reliably.
7.  **`shortest_public_path` Motivation:** This function is needed primarily to generate the `PubPathId`, which allows consistent ID generation between a dependency crate and the user crate that imports items from it, even if the import path differs from the canonical definition path within the dependency.

## Current Implementation Notes

*   `NodeId::Synthetic` generation incorporates file path, relative module path, item name, item kind, parent scope ID, and a hash of effective `cfg` strings.
*   `shortest_public_path` currently finds the path to the *defining* module, not accounting for re-exports.
*   Handling of `#[path]` attributes during module resolution needs careful review. See "Phase 1 & 2 Summary" below.
*   Full `cfg` evaluation is deferred, but CFG strings are captured for ID generation.

## Phase 1 & 2 Summary (File Discovery & Parsing)

*   **Phase 1 (Discovery - `discovery.rs`):**
    *   Uses `WalkDir` to find *all* `.rs` files within each target crate's `src/` directory.
    *   Parses `Cargo.toml` for crate name and version to generate a `crate_namespace` UUID.
    *   Does **not** interpret `#[path]` attributes during file collection.
    *   Output: `DiscoveryOutput` containing a list of all found `.rs` files per crate.
*   **Phase 2 (Parallel Parsing - `visitor/mod.rs`, `code_visitor.rs`):**
    *   `analyze_files_parallel` iterates over the files found in Phase 1.
    *   For each file, `derive_logical_path` calculates a *provisional* module path based solely on the file's path relative to `src/` (e.g., `src/foo/bar.rs` -> `["crate", "foo", "bar"]`). This **ignores** `#[path]`.
    *   `analyze_file_phase2` parses the content of the single file using `syn::parse_file`.
    *   It creates a root `ModuleNode` for the file being parsed, using the provisional logical path derived above.
    *   `CodeVisitor` traverses the AST of *only the current file*.
        *   It creates `ModuleNode`s for inline definitions (`mod foo {}`) and declarations (`mod foo;`) found *within this file*.
        *   It does **not** have access to `#[path]` attributes defined in *other* files. The `#[path]` attribute string itself **is currently stored** in the `ModuleNode.attributes` list via `attribute_processing::extract_attributes` (which only filters `doc` and `cfg`). The goal is to **preserve** this information.
*   **`#[path]` Resolution:** The connection between a declaration (`mod foo; #[path="bar.rs"]`) parsed in one file and the definition parsed from the target file (`bar.rs`) is established **later**, during Phase 3 (`ModuleTree::build_logical_paths`). This function currently works by finding module definitions (FileBased or Inline) and looking for corresponding declarations in the `decl_index` using the *same logical path*. It does **not** currently use the stored `#[path]` attribute value from the declaration node during this linking process. If a match is found based on the path, it creates a `ResolvesToDefinition` relation. If no declaration is found for a file-based module's path, it's currently treated as an "unlinked module".

---

This document provides a snapshot of the goals. We will update it as the design evolves or our focus shifts.
