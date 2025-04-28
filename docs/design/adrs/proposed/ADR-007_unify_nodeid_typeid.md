# ADR-007: Unify NodeId and TypeId

## Status
PROPOSED

## Context
The current system uses two separate enums, `NodeId` and `TypeId`, both wrapping `Uuid`s with `Synthetic` and `Resolved` variants. `NodeId` typically represents definitions (structs, functions, modules, etc.), while `TypeId` represents type usages (in parameters, fields, return types, etc.). This separation arose organically to handle collisions and distinguish definition sites from usage sites during parsing. However, this split increases conceptual complexity, diverges from `rustc`'s unified `DefId` approach, and requires careful management of two ID spaces. Issues with generic handling and definition/usage clarity persist.

## Decision
Unify `NodeId` and `TypeId` into a single enum, potentially named `SemanticId { Resolved(Uuid), Synthetic(Uuid) }`. This single ID type would represent all definable and referenceable entities (definitions and type usages). The distinction between a definition and a usage would be handled structurally within the `CodeGraph` (e.g., a node *has* a `SemanticId`, while a field *stores* the `SemanticId` of its type).

## Consequences
- Positive:
    - Simplifies the ID system conceptually.
    - Aligns the graph model more closely with `rustc`'s internal semantics (`DefId`).
    - Potentially simplifies ID generation logic.
    - Forces a clear structural representation of definition vs. usage.
- Negative:
    - Requires significant refactoring across the codebase (`ploke-core`, `syn_parser` nodes, visitors, relations, graph structure) and extensive test updates.
    - The immediate functional benefit to the current Phase 2 parsing output might be limited compared to the effort required.
- Neutral:
    - This change is proposed but deferred due to the high implementation cost relative to other immediate priorities.

## Related Documents
- Detailed Refactoring Plan (Deferred): [/home/brasides/code/second_aider_dir/ploke/docs/plans/uuid_refactor/thoughts/possible_future_refactoring.md](/home/brasides/code/second_aider_dir/ploke/docs/plans/uuid_refactor/thoughts/possible_future_refactoring.md)

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items: (To be reviewed if accepted)
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections: (To be reviewed if accepted)
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: (To be reviewed if accepted)
