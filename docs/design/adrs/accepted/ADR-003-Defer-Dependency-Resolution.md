# ADR-003: Defer Full Cross-Crate Dependency ID Resolution

## Status
ACCEPTED

## Context
The multi-phase batch processing model ([UUID Refactor Overview](docs/plans/uuid_refactor/00_overview_batch_processing_model.md)) aims to generate stable UUID-based identifiers (`NodeId`, `TypeId`) for code elements. Initial analysis ([Phase 1 Review](docs/plans/uuid_refactor/01a_phase1_discover_review.md)) and further discussion revealed significant complexity in resolving identifiers for items imported from external dependencies, particularly those not included in the current parsing batch.

Specifically:
- Items defined in parsed dependencies need their `NodeId::Path` (generated within their own crate's context) to be correctly linked from the user's code.
- Items defined in *unparsed* dependencies cannot have a `NodeId::Path` generated. References to them in user code would remain unresolved.
- Handling re-exports across crates adds another layer of resolution complexity.
- Attempting full cross-crate resolution in the initial implementation significantly increases the complexity of Phase 3 (Batch Resolution) and could block progress on core intra-crate functionality.

## Decision
The initial implementation of the UUID refactor (Phases 1-5 as defined in the [UUID Refactor Overview](docs/plans/uuid_refactor/00_overview_batch_processing_model.md)) will **focus on generating stable `NodeId::Path` and final `TypeId` / `LogicalTypeId` identifiers primarily for items defined *within* the set of currently parsed crates.**

Resolution of references *to* items defined in external dependencies will be handled as follows:
- If the dependency is *parsed* within the same batch, Phase 3 will attempt to resolve the reference to the dependency item's final `NodeId::Path` or `TypeId`. (Implementation details TBD, may require access to dependency resolution maps).
- If the dependency is *not parsed*, references to its items will remain unresolved. They will likely be represented using `NodeId::Synthetic` or `TypeId::Synthetic` identifiers, potentially including the unresolved path string (e.g., `"some_dep::SomeType"`) as metadata to aid future resolution or analysis.
- The `CodeGraph` structure and downstream processes (like `ploke-graph` transformation) must be designed to handle these potentially unresolved/synthetic identifiers gracefully.

**Full, robust cross-crate dependency ID resolution is deferred as a future enhancement.** The initial focus is on establishing the core UUID infrastructure and intra-crate resolution.

## Consequences
- **Positive:**
    - Simplifies the initial implementation scope of Phase 3 (Batch Resolution).
    - Allows for faster delivery of the core UUID refactoring benefits for intra-crate analysis.
    - Avoids getting blocked early on complex dependency management logic.
    - Provides a working foundation upon which full dependency resolution can be built later.
- **Negative:**
    - Graph queries involving items from unparsed dependencies will be incomplete or may not link correctly across the user-dependency boundary using final IDs.
    - Requires careful design of synthetic IDs and unresolved markers to store sufficient information for potential future resolution.
    - Downstream consumers need to be aware of and handle potentially unresolved identifiers.
- **Neutral:**
    - The core strategy for generating `NodeId::Path` and `TypeId` for locally defined items remains valid.
    - The multi-phase batch processing model itself is still appropriate.

## Compliance
- [PROPOSED_ARCH_V3.md](./../../../../PROPOSED_ARCH_V3.md) Items: N/A (This ADR refines implementation strategy, not core architecture).
- Relates to: [UUID Refactor Overview](docs/plans/uuid_refactor/00_overview_batch_processing_model.md) (Phases 2, 3, 5, ID States Table).
