# Call graph ID-domain correction

Date: 2026-06-22
Title: Call-site ID domain correction before parser call-graph extraction
Short description: Stabilize call-site identity so parser call graph work follows the existing `NodeId` / `TypeId` refinement-family pattern instead of treating expression occurrences as ordinary node IDs.
Related planning files:
- `docs/active/agents/2026-06-21_call-graph-fixture-nodes-orchestration-plan.md`
- `.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`
- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/mod.rs`
- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_ids.rs`
- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_families.rs`

## Decision

Call sites must live in a separate identity universe from parser nodes and structural type occurrences.

Implementation status as of 2026-06-22: `ploke_core::CallId` has been added, and typed call-site wrappers now live in `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs` backed by `CallId`, not `NodeId`.

The intended parser graph universes are:

```text
V_node = code item / code-graph node vertices, keyed by NodeId
V_type = structural type occurrence vertices, keyed by TypeId
V_call = call expression occurrence vertices, keyed by CallId
```

`PathCallSiteId`, `MethodCallSiteId`, `DynamicCallSiteId`, and `MacroCallSiteId` should be typed wrappers over `CallId`, not wrappers over `NodeId`.

## Existing good pattern

`NodeId` is refined into concrete node-id wrappers such as `FunctionNodeId`, `MethodNodeId`, `StructNodeId`, and `ImplNodeId`. Finite endpoint families such as `PrimaryNodeId`, `AssociatedItemNodeId`, `GenericParamOwnerId`, and `AnyNodeId` encode valid node endpoint sets.

`TypeId` follows the same idea in a separate type universe. It is refined into wrappers such as `NamedTypeId`, `ReferenceTypeId`, and `TraitBoundTypeId`; then families such as `AnyTypeId`, `OrdinaryTypeUseId`, `OrdinaryTypeSourceId`, and `TraitTypeSourceId` encode admissible type-use and type-resolution endpoint sets.

A relation endpoint's Rust type is therefore a proof of membership in a valid set. For example:

```rust
SyntacticRelation::ImplAssociatedItem {
    source: ImplNodeId,
    target: AssociatedItemNodeId,
}
```

and:

```rust
TypeRelation::Ordinary {
    source: OrdinaryTypeSourceId,
    target: OrdinaryTypeTargetId,
}
```

## Problem with the first call-site scaffold

The first call-site scaffold defined call-site IDs with the node-id macro, making them wrappers over `NodeId`:

```rust
PathCallSiteId(NodeId)
MethodCallSiteId(NodeId)
DynamicCallSiteId(NodeId)
MacroCallSiteId(NodeId)
```

That is a quality and correctness regression because call sites are expression occurrences inside bodies, not code definition nodes. Using `NodeId` for them silently changes the meaning of the node universe from "code item / graph node" to "code item or expression occurrence" without updating the rest of the graph model.

## Why the departure matters

### Code quality and organization

Mixing call-site occurrences into `NodeId` blurs the boundary between item nodes and expression-level facts. It also encourages adding call-specific concerns into `nodes/ids/internal/mod.rs`, which is already the node/type ID hub. Call IDs should instead be isolated in a call-specific module, analogous to the type split between `type_ids.rs` and `type_families.rs`.

### Correctness

A typed endpoint wrapper should prove that the parser saw an AST construct in the corresponding syntactic class. A public constructor like `MethodCallSiteId::generate_synthetic(...)` weakens that proof: any caller with owner/name/span/cfg data can mint a method call-site ID without a parsed `syn::ExprMethodCall` or a stored `CallNode::MethodCall`.

Production call-site ID generation should be parser-internal and owned by the body extraction path. Integration tests can have explicit test/paranoid regeneration helpers, but ordinary production code should not get broad constructors for typed call-site IDs.

### ID lifecycle semantics

`NodeId` carries `Resolved` / `Synthetic` semantics that make sense for item/path identity. A call-site occurrence is not "resolved" in the same way as a definition node. The call site can have a semantic resolution status to a target, but the occurrence identity itself should not inherit `NodeId`'s path-resolution lifecycle.

### Downstream safety

Transform/DB code already treats node UUIDs as code-node identities. If call-site IDs are backed by `NodeId`, it becomes easier to accidentally project a call expression occurrence into node tables, module containment logic, or node-only relation invariants. A distinct `CallId` universe forces projection code to cross an explicit boundary when flattening to database UUIDs.

## Corrected structure

Use a distinct base ID:

```rust
pub enum CallId {
    Synthetic(Uuid),
}
```

Then define typed call-site wrappers and families over that universe:

```rust
PathCallSiteId(CallId)
MethodCallSiteId(CallId)
DynamicCallSiteId(CallId)
MacroCallSiteId(CallId)

AnyCallSiteId =
    PathCallSiteId
  ∪ MethodCallSiteId
  ∪ DynamicCallSiteId
  ∪ MacroCallSiteId

CallBodyOwnerId = FunctionNodeId ∪ MethodNodeId
```

Keep the structural relation typed:

```rust
CallSiteRelation::BodyContainsCall {
    source: CallBodyOwnerId,
    target: AnyCallSiteId,
}
```

Use a typed call-site kind enum for generation and diagnostics, not string tags:

```rust
CallSiteKind::{Path, Method, Dynamic, Macro}
```

## Implementation guardrails

1. Do not add call-site IDs to `AnyNodeId`, `PrimaryNodeId`, or other node families.
2. Do not add call-site variants to `ItemKind` unless there is an explicit decision to broaden `NodeId`, which this note rejects for now.
3. Do not expose broad production constructors for `PathCallSiteId`, `MethodCallSiteId`, etc.
4. Keep call-site ID generation parser-internal, preferably through a `VisitorState` helper used by body extraction.
5. Keep integration-test ID regeneration in the explicit `test_ids` / paranoid-helper path, matching existing test conventions without making it the production construction API.
6. Preserve fail-closed semantics: a call-site ID alone is not a resolved edge; semantic target proof belongs in a later typed `CallRelation` / `CallResolutionStatus` layer.

## Narrow target after correction

The first fixture-driven structural target is now implemented and green:

```text
fixture_nodes_public_method_records_self_private_method_call_site
```

The body visitor creates `CallNode::MethodCall` and a `BodyContainsCall` relation only after seeing the original `syn::ExprMethodCall` for `self.private_method()`.

The first exact inherent `self.method()` resolver slice is also now implemented and green. Future call-graph work should broaden structural coverage first, starting with path-style calls, before adding broader semantic resolution.
