# Plan: Canonical IDs for Types and Impls

## Goal

Define and implement a Phase 3+ resolution step that produces canonical IDs for:

- Defined code items (CanonId) using ModuleTree paths.
- Named type references, mapping `TypeKind::Named` usage sites to CanonId of the defining item.
- Impl semantics derived from canonical type identities (self type and optional trait).

This plan assumes Phase 2 remains syntactic and parallel, and Phase 3 performs batch resolution
after `build_module_tree` (per `docs/plans/uuid_refactor/00_overview_batch_processing_model.md`).

## Current State (Observed)

- `TypeId` is structural and scoped: it hashes `TypeKind`, related types, file path, and
  `parent_scope_id` (see `ploke-core/src/lib.rs`). It is **not** a definition ID.
- `TypeNode` stores structural type info (`TypeKind`, related types), not resolved definitions.
- `CanonIdResolver` exists but is not wired into the pipeline after `build_module_tree`.
- `ImplNodeId` collisions occur for multiple unnamed impl blocks with identical headers.

## Proposed Resolution Pipeline (Phase 3+)

1) **Build ModuleTree**
   - As today: `ParsedCodeGraph::build_module_tree()` constructs the module tree and linkages.

2) **Resolve Node CanonIds**
   - Invoke `CanonIdResolver::resolve_all` after ModuleTree construction.
   - Output: `AnyNodeId -> CanonId` mapping for all resolvable defined items.
   - Persist mapping in the graph or a sidecar structure used by later steps.

3) **Resolve Named Type References**
   - Create a pass that inspects `TypeNode.kind` for `TypeKind::Named`.
   - For each `TypeKind::Named { path, is_fully_qualified }`:
     - Resolve path to a definition using ModuleTree + import/reexport context.
     - If resolved to a local definition, map to that definition’s CanonId.
     - If unresolved or external, record as unresolved with canonicalized path string.
   - Store this mapping as a new relation or map:
     - Option A: `TypeId -> CanonId` map for resolved named types.
     - Option B: Add a new relation (e.g., `TypeRefResolvedTo`) from `TypeId` to `CanonId`.

4) **Derive Impl Semantics**
   - For each impl block:
     - Resolve `self_type`’s `TypeId` to a CanonId (via step 3).
     - Resolve `trait_type`’s `TypeId` to a CanonId when present.
   - Emit semantic edges:
     - `ImplementsTrait`: self CanonId -> trait CanonId (if trait impl).
     - `InherentImpl`: self CanonId -> impl block (or directly to methods).
   - Keep impl blocks as syntactic nodes; avoid grouping by Synthetic IDs.
   - Optional: introduce a minimal ImplGroup keyed by `(self CanonId, trait CanonId, generics sig)`
     if downstream consumers need a stable grouping entity.

## Data to Add or Update

- **CanonId mapping storage**: shared map or field on graph for resolved node IDs.
- **Type resolution mapping**: `TypeId -> CanonId` (for named types only).
- **Impl semantic relations**: new relation kind(s) for trait/inherent impl edges.

## Open Questions

- Where to store CanonId mappings (in `ParsedCodeGraph`, `ModuleTree`, or a sidecar struct)?
- Do we need a new `TypeRefResolvedTo` relation or a standalone map?
- How to represent unresolved/external types (store canonicalized path string)?
- Should impl edges be stored as relations or as a minimal ImplGroup node?
- How to encode generic parameters in impl semantics (generics signature vs. full TypeId graph)?

## Implementation Outline

1) Wire `CanonIdResolver` into the pipeline after `build_module_tree`.
2) Add a type resolution pass for `TypeKind::Named`.
3) Add impl semantic edge generation based on resolved CanonIds.
4) Add tests:
   - CanonId resolution is invoked and stored.
   - Named types resolve to local definitions when possible.
   - Impl blocks produce semantic edges for inherent and trait impls.

## Risks

- Import resolution for types is not complete; initial pass should explicitly scope to
  intra-crate, module-resolvable names and be resilient to unresolved paths.
- Overloading TypeId with semantic meaning would conflict with its structural design.
- Impl grouping may still be needed for downstream consumers; keep design flexible.

## Context Files (2025-12-29)

- crates/ploke-core/src/lib.rs: Defines `NodeId`/`TypeId` generation and hashing inputs; this plan depends on keeping `TypeId` structural and adding CanonId resolution.
- crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs: Owns `build_module_tree`; the plan adds a Phase 3 resolution step after this method.
- crates/ingest/syn_parser/src/parser/nodes/ids/internal.rs: Defines typed ID wrappers and generation via `VisitorState`; relevant if adding new relations or mapping CanonIds.
- crates/ingest/syn_parser/src/parser/nodes/impls.rs: Defines `ImplNode`; impl semantics and any future grouping/edges will hang off this shape.
- crates/ingest/syn_parser/src/parser/relations.rs: Defines `SyntacticRelation`; may need new semantic relations for impl edges and type resolution.
- crates/ingest/syn_parser/src/parser/types.rs: Defines `TypeNode` and structural typing model; informs how named type resolution should be layered.
- crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs: Builds impl nodes and relations; source of impl ID collisions and where salting would occur.
- crates/ingest/syn_parser/src/parser/visitor/type_processing.rs: Builds `TypeNode`/`TypeId` from syntax; used by the planned type-resolution pass.
- crates/ingest/syn_parser/src/resolve/id_resolver.rs: CanonId resolver; needs to be wired into the pipeline after ModuleTree.
- crates/ingest/syn_parser/src/resolve/module_tree.rs: ModuleTree construction and relation validation; provides canonical paths used by CanonId resolution.
- docs/plans/uuid_refactor/00_overview_batch_processing_model.md: Defines the intended multi-phase pipeline that this plan aligns to (Phase 3 resolution).
- docs/plans/uuid_refactor/90a_type_processing_overview.md: Documents type processing assumptions; helps reconcile TypeId structure vs. canonical resolution.
