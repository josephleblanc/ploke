# Ploke call graph restart spine

Date: 2026-06-22
Status: active feature restart spine
Short description: Current source of truth for the parser call-graph feature thread; read this first after compaction or before dispatching implementation work.

## Read order

1. This file.
2. [`../2026-06-22_call-graph-id-domain-correction.md`](../2026-06-22_call-graph-id-domain-correction.md) — binding design decision for call-site identity.
3. [`2026-06-22_structural-method-call-extraction-plan.md`](2026-06-22_structural-method-call-extraction-plan.md) — completed structural method-call slice.
4. [`2026-06-22_inherent-self-method-resolution-plan.md`](2026-06-22_inherent-self-method-resolution-plan.md) — completed first semantic resolver slice.
5. [`../2026-06-21_call-graph-fixture-nodes-orchestration-plan.md`](../2026-06-21_call-graph-fixture-nodes-orchestration-plan.md) — task sequence for the first `fixture_nodes` slice.
6. [`../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`](../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md) — broader typed call-graph rollout plan.
7. Long-horizon proof context only if needed:
   - [`../../../workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`](../../../workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md)
   - [`../../../workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`](../../../workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md)
   - [`../../../workflow/evalnomicon/drafts/formal/macro-buildrs-callgraph-sequencing-survey.md`](../../../workflow/evalnomicon/drafts/formal/macro-buildrs-callgraph-sequencing-survey.md)
   - [`../../../workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md`](../../../workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md)

## Current implementation state

Current branch/worktree state is uncommitted. Treat this as in-progress scaffolding, not landed feature support.

Implemented/scaffolded:

- `ploke_core::CallId` as a distinct base identity universe for parser-owned call expression occurrences.
- Typed call-site wrappers over `CallId`, not `NodeId`:
  - `PathCallSiteId`
  - `MethodCallSiteId`
  - `DynamicCallSiteId`
  - `MacroCallSiteId`
- Call endpoint families:
  - `AnyCallSiteId`
  - `CallBodyOwnerId = FunctionNodeId ∪ MethodNodeId`
- Structural graph storage:
  - `CodeGraph.call_sites`
  - `CodeGraph.call_site_relations`
  - `CallSiteRelation::BodyContainsCall { source: CallBodyOwnerId, target: AnyCallSiteId }`
- Structural body extraction for the first narrow method-call slice:
  - `syn::ExprMethodCall` with literal `self` receiver.
  - `CallNode::MethodCall` emission.
  - `CallSiteRelation::BodyContainsCall` emission.
- Typed call-resolution storage/accessor scaffold:
  - `CodeGraph.call_relations`
  - `CodeGraph.call_resolution_statuses`
  - `CallRelation::{Function, Method}`
  - `CallResolutionStatus::{Resolved, Unresolved, Ambiguous, External, Unsupported}`
- First semantic resolver slice:
  - `resolve::call_resolution::resolve_call_relations_after_tree(...)`
  - exact inherent `self.method()` resolution within the same impl block.
- GREEN fixture tests:
  - `fixture_nodes_public_method_records_self_private_method_call_site`
  - `fixture_nodes_public_method_resolves_self_private_method_edge`

Not implemented yet:

- Path call extraction.
- Dynamic call extraction.
- Macro call extraction.
- Non-`self` method receiver classification.
- Trait dispatch.
- Free-function path-call resolution.
- Associated-function path-call resolution.
- Transform/database projection.
- Proof-fact projection.

## Binding design decisions

### Call-site IDs are not NodeIds

The parser now uses three distinct identity universes:

```text
V_node = code item / code-graph node vertices, keyed by NodeId
V_type = structural type occurrence vertices, keyed by TypeId
V_call = call expression occurrence vertices, keyed by CallId
```

Call-site typed IDs must not be added to `AnyNodeId`, `PrimaryNodeId`, `AssociatedItemNodeId`, or any other node endpoint family.

### Constructors stay narrow

Production code should not expose broad constructors such as:

```rust
MethodCallSiteId::generate_synthetic(...)
```

The parser extraction path may use parser-internal helpers after seeing the corresponding `syn` expression shape. Tests may use explicit test/paranoid helpers for deterministic regeneration.

### Structural facts are not semantic edges

A `MethodCallSiteId` proves structural syntax class only. It does not prove the target method. Resolved call edges must be introduced later through typed `CallRelation` / `CallResolutionStatus` data.

### Fail closed

Unsupported, dynamic, macro, external, and ambiguous call shapes must remain visible as structural facts or explicit statuses. They must not become fake local function/method edges.

## Completed implementation slice: structural `self.method()` extraction

Implemented in this slice:

1. The parser visits original `syn` bodies for standalone functions and impl/trait methods after the typed body owner ID is known.
2. It observes `self.private_method()` as `syn::ExprMethodCall`.
3. It emits exactly one `CallNode::MethodCall` with:
   - owner `CallBodyOwnerId::Method(public_method_id)`;
   - receiver `MethodCallReceiver::SelfValue`;
   - method name `private_method`;
   - arg count `0`;
   - generic arg count `0`;
   - expected byte span and empty cfgs for the fixture row.
4. It emits exactly one `BodyContainsCall` relation from the owner to the call site.
5. Structural extraction itself still does not emit any resolved `CallRelation`.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
- `crates/ingest/syn_parser/src/parser/visitor/mod.rs`

## Completed implementation slice: exact inherent `self.method()` resolution

Implemented in this slice:

1. The resolver consumes structural call sites after `ModuleTree` construction/pruning.
2. It handles only `MethodCallReceiver::SelfValue` where the call owner is a method.
3. It proves the owner method belongs to an inherent impl through `ImplAssociatedItem`.
4. It searches the same impl for a unique method with the call site's method name.
5. It emits `CallRelation::Method { source, target }` and `CallResolutionStatus::Resolved { kind: LocalExact, .. }` only on exact proof.
6. It fails closed with statuses and no fake target edge for unsupported/unresolved/ambiguous cases.

Primary implementation files:

- `crates/ingest/syn_parser/src/resolve/call_resolution.rs`
- `crates/ingest/syn_parser/src/parser/relations.rs`
- `crates/ingest/syn_parser/src/parser/graph/{code_graph.rs,mod.rs,parsed_graph.rs}`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Verification commands

Known-good baseline after the ID-domain correction:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture
```

Structural fixture test now expected GREEN:

```bash
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_records_self_private_method_call_site -- --nocapture
```

Resolver fixture test now expected GREEN:

```bash
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_resolves_self_private_method_edge -- --nocapture
```

## Drift prevention checklist

Before implementing new call-graph work, verify:

- call-site wrappers are still backed by `CallId`, not `NodeId`;
- no call-site IDs appear in `AnyNodeId` or node-category enums;
- no public production call-site ID generator has been added;
- tests regenerate call IDs through test-only helpers;
- structural call-site facts remain separate from resolved semantic call edges;
- planning docs still point to this restart spine and the ID-domain correction note.

## Last verification summary

Commands run after structural extraction and first resolver slice landed:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture
cargo test -p syn_parser --features typed_type_graph call_sites -- --nocapture
```

Result: all passed. The `call_sites` filter ran two focused tests and both passed.

## Next implementation slice

Broaden structural coverage before broad semantic coverage. Recommended next RED tests:

1. structural path-call extraction for a simple local `helper()` or constructor/associated-function-shaped path call;
2. local free-function path-call resolution only after path-call structure is green;
3. associated-function path-call resolution (`Self::new()` / `SimpleStruct::new()`) after a dedicated structural path-call test.
