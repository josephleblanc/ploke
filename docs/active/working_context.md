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
*   Handling of `#[path]` attributes during module resolution needs careful review.
*   Full `cfg` evaluation is deferred, but CFG strings are captured for ID generation.

---

This document provides a snapshot of the goals. We will update it as the design evolves or our focus shifts.
