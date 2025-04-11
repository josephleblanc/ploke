# ADR-005: Deferred Relation Linking via PendingRelation Enum

## Status
ACCEPTED

git tag: 5fb4eab49c5ae99b704666e61bda4b040cb3734e

## Context
During the parallel parsing phase (Phase 2, see [02_phase2_parallel_parse_implementation.md](../../../../../docs/plans/uuid_refactor/02_phase2_parallel_parse_implementation.md)), the `CodeVisitor` identifies syntactic constructs implying relationships (e.g., `use` statements, type usages in signatures, `mod` declarations). However, the identifier (`NodeId` or `TypeId`) for the *target* of the relationship often cannot be determined using only the information within that single file, as the target might be defined elsewhere or require global context established later. Creating the final `Relation` struct with definitive source and target identifiers is therefore not always possible during Phase 2. A mechanism is needed to capture the *intent* and *context* of these potential relationships for processing during the sequential resolution phase (Phase 3, see [00_overview_batch_processing_model.md](../../../../../docs/plans/uuid_refactor/00_overview_batch_processing_model.md)).

## Decision
We will adopt the **`PendingRelation` Enum** approach.
1.  Define a `PendingRelation` enum. Each variant will represent a specific type of unresolved link identified during Phase 2 (e.g., `ResolveModuleDecl`, `ResolveImport`, `ResolveTypeUsage`).
2.  Each `PendingRelation` variant will store the necessary context required for later resolution, such as the source node's ID, the unresolved path or name used in the source code, relevant spans, visibility, and any attributes associated with the linking construct itself.
3.  During Phase 2, the `CodeVisitor` will generate intermediate node representations (using `Synthetic` IDs) and populate a list of `PendingRelation` objects within its `VisitorState` for links whose target identifier cannot be immediately determined. Links that *can* be fully formed immediately (e.g., intra-file `Contains` using known `Synthetic` IDs) can be added to a separate `formed_relations` list.
4.  The output of Phase 2 for each worker will consist of the discovered intermediate nodes, formed relations, and the list of `PendingRelation` tasks.
5.  Phase 3 will aggregate these outputs from all workers. Its responsibility includes processing the aggregated list of `PendingRelation` tasks. Using the complete set of discovered nodes and the resolved module tree, it will attempt to determine the target identifier (`NodeId` or `TypeId`, which might still be `Synthetic` if the target is external or unparsed) for each pending task.
6.  Upon determining the target identifier, Phase 3 will generate the final `Relation` structs (using `GraphId` wrappers). The identifiers within these `Relation` structs represent the outcome of the linking process, though the `TypeId`s themselves might not be fully resolved to the `Resolved` variant until later in Phase 3 or if external dependencies are considered (per [ADR-003](../../../../../docs/design/adrs/accepted/ADR-003-Defer-Dependency-Resolution.md)).

## Consequences
- **Structural Changes:**
    -   Introduces a `PendingRelation` enum to represent link intents identified in Phase 2.
    -   Requires `VisitorState` to manage lists of intermediate nodes, formed relations, and pending relations.
    -   Phase 3 logic must be designed to consume the `PendingRelation` list and produce the final set of `Relation` structs.
    -   The final `Relation` struct remains relatively simple, containing source and target `GraphId`s established by Phase 3.
- **Process Implications:**
    -   Clearly separates the task of identifying potential links (Phase 2) from the task of determining the target identifier and forming the final link (Phase 3).
    -   Facilitates parallel parsing by deferring cross-file lookups required for linking.
    -   Provides a structured way to handle different types of unresolved links, storing the necessary context for each type.
- **Resolution Scope:**
    -   This mechanism focuses on establishing the *link* (the `Relation` struct) with the best available target identifier determined by Phase 3.
    -   It does *not* guarantee that the target `TypeId` within the final `Relation` will be a `TypeId::Resolved` variant; final type resolution depends on the completeness of Phase 3's analysis and handling of external dependencies.

## Compliance
- **[PROPOSED_ARCH_V3.md](../../../../../PROPOSED_ARCH_V3.md) Items:**
    -   Supports the multi-phase Processing Pipeline by defining the state transfer between Phase 2 (Parser) and Phase 3 (Resolution).
    -   Contributes to creating a queryable CozoDB graph by ensuring relationships are processed and formed before transformation (Phase 5).
- **[IDIOMATIC_RUST.md](../../../../../ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:**
    -   Uses enums (C-TYPE-SAFETY) to explicitly model the different states of unresolved links.
    -   Contributes to Predictability (C-PREDICTABILITY) by making the linking process a distinct step.
- **[CONVENTIONS.md](../../../../../ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items:**
    -   Maintains separation between the Compute Domain (Phase 2) and subsequent sequential processing (Phase 3).

## Relevant Documents
-   [UUID Refactor Overview (00_overview_batch_processing_model.md)](../../../../../docs/plans/uuid_refactor/00_overview_batch_processing_model.md)
-   [Phase 2 Implementation Plan (02_phase2_parallel_parse_implementation.md)](../../../../../docs/plans/uuid_refactor/02_phase2_parallel_parse_implementation.md)
-   [ADR-003: Defer Dependency Resolution](../../../../../docs/design/adrs/accepted/ADR-003-Defer-Dependency-Resolution.md)
-   [ADR-004: Parser Scope Resolution](../../../../../docs/design/adrs/proposed/ADR-004-Parser-Scope-Resolution.md)
