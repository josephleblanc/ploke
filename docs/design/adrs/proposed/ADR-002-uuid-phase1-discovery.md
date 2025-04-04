# ADR-002: Implement Phase 1 - Discovery and Context Setup for UUID Refactor

## Status
PROPOSED

## Context
[ADR-001](ADR-001-uuid-for-ids.md) mandates using UUIDs for node identifiers and outlines a [Multi-Phase Batch Processing Model](docs/plans/uuid_refactor/00_overview_batch_processing_model.md). Before parallel parsing (Phase 2) can begin, essential context about the target crates (file lists, crate names, versions, namespaces, initial module structure) must be gathered. This preparatory step, defined as Phase 1, is currently missing from the implementation.

## Decision
Implement **Phase 1: Discovery & Context Setup** as described in the [UUID Refactor Overview](docs/plans/uuid_refactor/00_overview_batch_processing_model.md#phase-1-discovery--context-setup). This phase will:
1.  Identify all `.rs` files within specified target crates.
2.  Parse `Cargo.toml` for each target crate to extract `name` and `version`.
3.  Define a constant `PROJECT_NAMESPACE` UUID.
4.  Derive a `CRATE_NAMESPACE` UUID for each target crate based on its name and version.
5.  Perform a minimal scan of key files (`lib.rs`, `main.rs`, `mod.rs`) to build an initial, potentially incomplete, map of module structures.
6.  Package this information (file lists, namespaces, initial module map) into a context object to be passed to subsequent phases.
7.  All implementation will occur under the `uuid_ids` feature flag defined in ADR-001.

## Consequences
-   **Positive:**
    -   Provides necessary input context (namespaces, file lists) for Phase 2 (Parallel Parse).
    -   Establishes the foundation for deterministic UUID generation.
    -   Decouples discovery logic from parsing logic.
-   **Negative:**
    -   Adds an initial processing step before parsing begins, increasing latency for the first run.
    -   Introduces new dependencies (e.g., `toml` crate for `Cargo.toml` parsing).
    -   Requires new data structures to hold the discovery context.
    -   Initial module mapping might be inaccurate until Phase 3 resolution.
-   **Neutral:**
    -   Introduces new functions/modules dedicated to discovery.
    -   Requires careful handling of file system interactions and potential errors (e.g., missing files, invalid `Cargo.toml`).

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items: Supports the overall architecture by preparing data for the parallel processing pipeline (`ingest`).
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
    - C-COMMON-TRAITS: New context structs should derive common traits.
    - C-GOOD-ERR: Error handling for file I/O and TOML parsing must be robust.
    - C-METADATA: Requires adding `toml` dependency to relevant `Cargo.toml`.
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items:
    - Error handling: Use `Result<_, Box<dyn Error>>` at boundaries.
    - Type System: New context structs must be `Send + Sync` if passed across threads (likely needed for Phase 2).
