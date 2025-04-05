# ADR-004: Expand Parser Scope to Include ID Resolution

## Status
PROPOSED

## Context
The UUID refactoring plan ([UUID Refactor Overview](docs/plans/uuid_refactor/00_overview_batch_processing_model.md)) introduces a multi-phase process where Phase 2 generates temporary `Synthetic` IDs and Phase 3 resolves these into final, stable `NodeId::Path` and `TypeId` identifiers.

Discussion highlighted that Phase 3 (Batch Resolution) requires deep access to the structural and contextual information gathered during parsing, including:
- The complete AST structure (or a detailed representation).
- Accurate module hierarchy information.
- Parsed `use` statements and their scope.
- Namespace information (`CRATE_NAMESPACE`) from Phase 1.

Attempting to perform this resolution in downstream crates (like `ploke-graph`) would necessitate passing large amounts of complex intermediate data or re-performing significant analysis. Performing resolution directly in the database (CozoDB) using CozoScript is considered impractical due to the complexity of Rust's name resolution rules and the limitations of string-based scripting for such tasks.

## Decision
Formally define the primary responsibility of the `syn_parser` crate as: **"Parsing Rust source code into a Resolved Code Graph with Stable Identifiers."**

This encompasses the full workflow described in the multi-phase model:
- **Phase 1:** Discovery & Context Setup (Input gathering).
- **Phase 2:** Parallel Parse & Provisional Graph Generation (AST parsing, `Synthetic` ID generation, `TrackingHash` calculation).
- **Phase 3:** Batch Resolution (Merging partial graphs, building the definitive module tree, resolving `Synthetic` IDs to final `NodeId::Path` and `TypeId` for items within the parsed set, updating relations).

The primary output artifact of `syn_parser` is the `CodeGraph` containing these resolved (or explicitly unresolved for external items, per ADR-003) identifiers and associated data, ready for consumption by downstream crates like `ploke-graph` and `ploke-db`.

## Consequences
- **Positive:**
    - Creates a logical grouping of tightly coupled tasks (parsing, context analysis, ID resolution) within a single crate.
    - Clarifies crate responsibilities and boundaries within the `ploke` architecture.
    - Avoids inefficiently passing large, complex intermediate data structures between crates.
    - Keeps complex Rust-specific resolution logic within the crate most knowledgeable about Rust syntax (`syn_parser`).
- **Negative:**
    - Increases the perceived scope and complexity of the `syn_parser` crate compared to a narrow definition of "parsing".
- **Neutral:**
    - Aligns the crate's defined responsibility with the necessary workflow required to produce stable, usable identifiers for the rest of the system.
    - Does not fundamentally change the technical steps required, but clarifies ownership.

## Compliance
- [PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items: N/A (Refines crate responsibility within existing architecture).
- [IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections: N/A.
- [CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A.
- Relates to: [UUID Refactor Overview](docs/plans/uuid_refactor/00_overview_batch_processing_model.md) (Phases 2, 3), [ADR-003-Defer-Dependency-Resolution.md](docs/design/adrs/accepted/ADR-003-Defer-Dependency-Resolution.md).
