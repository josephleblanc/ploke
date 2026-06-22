# Ploke call graph restart spine

Date: 2026-06-22
Status: active feature restart spine
Short description: Current source of truth for the parser call-graph feature thread; read this first after compaction or before dispatching implementation work.

## Read order

1. This file.
2. [`../2026-06-22_call-graph-id-domain-correction.md`](../2026-06-22_call-graph-id-domain-correction.md) — binding design decision for call-site identity.
3. [`2026-06-22_call-site-coverage-matrix.md`](2026-06-22_call-site-coverage-matrix.md) — active structural/resolution test matrix for future slices.
4. [`2026-06-22_structural-method-call-extraction-plan.md`](2026-06-22_structural-method-call-extraction-plan.md) — completed structural method-call slice.
5. [`2026-06-22_inherent-self-method-resolution-plan.md`](2026-06-22_inherent-self-method-resolution-plan.md) — completed first semantic resolver slice.
6. [`2026-06-22_structural-path-call-extraction-plan.md`](2026-06-22_structural-path-call-extraction-plan.md) — completed structural path-call slice.
7. [`2026-06-22_local-free-function-path-call-resolution-plan.md`](2026-06-22_local-free-function-path-call-resolution-plan.md) — completed first local path-call resolver slice.
8. [`2026-06-22_structural-macro-call-extraction-plan.md`](2026-06-22_structural-macro-call-extraction-plan.md) — completed structural macro-call slice.
9. [`../2026-06-21_call-graph-fixture-nodes-orchestration-plan.md`](../2026-06-21_call-graph-fixture-nodes-orchestration-plan.md) — task sequence for the first `fixture_nodes` slice.
10. [`../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`](../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md) — broader typed call-graph rollout plan.
11. Long-horizon proof context only if needed:
   - [`../../../workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`](../../../workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md)
   - [`../../../workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`](../../../workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md)
   - [`../../../workflow/evalnomicon/drafts/formal/macro-buildrs-callgraph-sequencing-survey.md`](../../../workflow/evalnomicon/drafts/formal/macro-buildrs-callgraph-sequencing-survey.md)
   - [`../../../workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md`](../../../workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md)

## Current implementation state

Current branch/worktree state is active feature work. Treat this as the restart spine for the incremental call-graph implementation.

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
- Structural path-call extraction:
  - `syn::ExprCall` with `syn::Expr::Path` callee.
  - `CallNode::PathCall` emission.
  - deterministic parser-internal `PathCallSiteId` construction from owner + path + span + cfgs.
- Structural macro-call extraction:
  - `syn::ExprMacro` and statement-position `syn::StmtMacro`.
  - `CallNode::MacroCall` emission.
  - deterministic parser-internal `MacroCallSiteId` construction from owner + macro path + span + cfgs.
- Typed call-resolution storage/accessor scaffold:
  - `CodeGraph.call_relations`
  - `CodeGraph.call_resolution_statuses`
  - `CallRelation::{Function, Method}`
  - `CallResolutionStatus::{Resolved, Unresolved, Ambiguous, External, Unsupported}`
- First semantic resolver slices:
  - `resolve::call_resolution::resolve_call_relations_after_tree(...)`
  - exact inherent `self.method()` resolution within the same impl block.
  - explicit local `crate`/`self`/`super` path-call resolution to local standalone functions.
- GREEN fixture tests now use a call-site paranoid harness and cover 12 concrete call expressions:
  - `fixture_nodes_public_method_records_and_resolves_self_private_method_call_site`
  - `fixture_nodes_get_secret_len_records_self_field_len_method_call_site`
  - `fixture_nodes_use_imported_items_records_hashmap_new_path_call_site`
  - `fixture_nodes_use_imported_items_records_fs_read_to_string_path_call_site`
  - `fixture_nodes_use_imported_items_records_pathbuf_new_path_call_site`
  - `fixture_nodes_use_imported_items_records_enum_variant1_path_call_site`
  - `fixture_nodes_use_imported_items_records_duration_from_secs_path_call_site`
  - `fixture_nodes_use_imported_items_records_arc_new_path_call_site`
  - `fixture_nodes_use_imported_items_records_tuple_struct_path_call_site`
  - `fixture_nodes_use_imported_items_records_documented_macro_call_site`
  - `fixture_nodes_use_all_const_static_records_println_macro_call_site`
  - `fixture_path_resolution_call_restricted_resolves_super_restricted_func_path_call_site`

Not implemented yet:

- Dynamic call extraction.
- Non-`self` method receiver classification.
- Trait dispatch.
- Unqualified/import/re-export path-call resolution.
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

The parser extraction path may use parser-internal helpers after seeing the corresponding `syn` expression shape. Tests use explicit test/paranoid helpers for deterministic regeneration, primarily through the call-site paranoid harness in `tests/common/call_site_paranoid.rs` and `paranoid_call_site_test!`.

### Structural facts are not semantic edges

A `MethodCallSiteId` proves structural syntax class only. It does not prove the target method. Resolved call edges must be introduced later through typed `CallRelation` / `CallResolutionStatus` data.

### Fail closed

Unsupported, dynamic, macro, external, and ambiguous call shapes must remain visible as structural facts or explicit statuses. They must not become fake local function/method edges.

## Completed implementation slice: call-site paranoid test harness

Implemented in this slice:

1. Added `tests/common/call_site_paranoid.rs` as the call-site analogue of the node-level paranoid helpers.
2. Added `paranoid_call_site_test!` to generate exact fixture-backed call-site tests.
3. Each generated test regenerates the typed `CallId`, checks exact-ID lookup, checks value lookup, checks the `BodyContainsCall` relation, and checks resolver status plus expected semantic edge/no-edge policy.
4. Converted the focused call-site coverage from hand-written tests/table rows to named paranoid tests.

Primary implementation files:

- `crates/ingest/syn_parser/tests/common/call_site_paranoid.rs`
- `crates/ingest/syn_parser/tests/common/macro_rule_tests.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

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

## Completed implementation slice: self-field method-call extraction

Implemented in this slice:

1. Method-call receiver classification now recognizes field projections rooted at `self`, such as `self.secret`.
2. `self.secret.len()` in `fixture_nodes::impls::PrivateStruct::get_secret_len` is recorded as `CallNode::MethodCall` with `MethodCallReceiver::SelfField { field_path: ["secret"] }`.
3. The resolver fails closed for this call with `Unsupported` and emits no fake local `CallRelation::Method` edge.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/nodes/call.rs`
- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Completed implementation slice: structural path-call extraction

Implemented in this slice:

1. The body visitor records `syn::ExprCall` expressions whose callee is `syn::Expr::Path`.
2. It emits `CallNode::PathCall` with path segments, value arg count, generic arg count, span, owner, and cfgs.
3. It emits `BodyContainsCall` for the path call site.
4. The focused fixture target is `PathBuf::new()` inside `fixture_nodes::imports::use_imported_items`.
5. The resolver initially failed closed for path-call sites; local explicit path-call resolution was added in a later slice.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Completed implementation slice: local free-function path-call resolution

Implemented in this slice:

1. `CallRelationResolver` now attempts local standalone-function resolution for explicit `crate::`, `self::`, and `super::` path calls.
2. The resolver traverses from the call owner's containing module, handles local path prefixes, walks intermediate module segments, and proves terminal `FunctionNodeId` targets.
3. `super::restricted_func()` in `fixture_path_resolution::restricted_vis_mod::inner::call_restricted` resolves to `restricted_func` with `CallRelation::Function` and `Resolved(LocalExact)` status.
4. Unsupported path-call shapes such as external associated-function-looking calls remain `Unsupported`.

Primary implementation files:

- `crates/ingest/syn_parser/src/resolve/call_resolution.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- `docs/active/agents/call-graph/2026-06-22_local-free-function-path-call-resolution-plan.md`

## Completed implementation slice: structural macro-call extraction

Implemented in this slice:

1. The body visitor records `syn::ExprMacro` invocations and statement-position `syn::StmtMacro` invocations.
2. It emits `CallNode::MacroCall` with macro path/name, span, owner, and cfgs.
3. It emits `BodyContainsCall` for the macro call site.
4. Focused fixture targets include `documented_macro!(fixture alias coverage)` inside `fixture_nodes::imports::use_imported_items` and `println!(...)` inside `fixture_nodes::const_static::use_all_const_static`.
5. No macro expansion or macro target resolution is attempted.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Verification commands

Known-good baseline after the ID-domain correction:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture
```

Focused structural/resolver call-site fixture tests now expected GREEN:

```bash
cargo test -p syn_parser --features typed_type_graph call_sites -- --nocapture
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

Commands run after structural extraction, resolver slices, path-call extraction, and macro-call extraction landed:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture
cargo test -p syn_parser --features typed_type_graph call_sites -- --nocapture
```

Result: all passed. The `call_sites` filter ran twelve paranoid fixture tests and all passed.

## Next implementation slice

Use [`2026-06-22_call-site-coverage-matrix.md`](2026-06-22_call-site-coverage-matrix.md) as the test-selection source of truth. Continue broadening structural coverage before broad semantic coverage. Recommended next RED tests:

1. broaden explicit local path-call resolution with `crate::...` / `self::...` fixtures and then imports/re-exports;
2. dynamic-call extraction for non-path callees such as `(f)()` or `make_fn()()` once a fixture target exists;
3. decide whether path-to-binding calls such as `alias_checker(...)` remain `PathCall` structurally or need a later binding-aware dynamic reclassification;
4. associated-function path-call resolution after a dedicated `Self::new()` / local associated-function path-call test.
