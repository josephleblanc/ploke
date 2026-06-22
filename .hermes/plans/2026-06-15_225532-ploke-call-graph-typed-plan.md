# Typed Call Graph Plan for Ploke

Date: 2026-06-15 22:55:32

## Goal

Add a Ploke-native call graph that preserves the parser's existing compile-time safety style. The design should follow the typed endpoint-family pattern used by `SyntacticRelation`, `TypeRelation`, and the typed type graph, rather than introducing parser-side stringly relations or raw `Uuid` edges.

This is a plan only. Do not implement it until explicitly asked.

## 2026-06-22 ID-domain update

The first implementation pass corrected the call-site identity design before adding body extraction. Call sites now have a distinct `CallId` universe in `ploke-core`, and typed call-site wrappers (`PathCallSiteId`, `MethodCallSiteId`, `DynamicCallSiteId`, `MacroCallSiteId`) wrap `CallId`, not `NodeId`.

This is a binding direction for future work:

- Do not add call-site IDs to `AnyNodeId`, `PrimaryNodeId`, or other node endpoint families.
- Do not use public production constructors such as `MethodCallSiteId::generate_synthetic(...)`; parser extraction should use parser-internal constructors after observing the corresponding `syn` expression shape.
- Tests may use explicit test/paranoid regeneration helpers.
- Treat `docs/active/agents/call-graph/README.md` and `docs/active/agents/2026-06-22_call-graph-id-domain-correction.md` as the current restart/design notes for this correction.

## 2026-06-22 first resolver update

The first semantic resolver slice is implemented for exact inherent `self.method()` calls. Current concrete names are narrower than some older sketches below:

- `CallRelation::{Function, Method}` currently exists; associated functions and constructors remain future variants/slices.
- `CallResolutionStatus::{Resolved, Unresolved, Ambiguous, External, Unsupported}` currently stores the call-site source and a `CallResolutionKind` for resolved cases. The durable edge set remains `CallRelation`.
- `resolve::call_resolution::resolve_call_relations_after_tree(...)` returns a `CallResolutionReport`; broad trait dispatch, path calls, external summaries, macro expansion, and dynamic calls remain unsupported.
- `fixture_nodes_public_method_resolves_self_private_method_edge` is green.

## Current context and assumptions

- Existing AST graph lives mainly under `crates/ingest/syn_parser/src/parser/`.
- Existing typed relation precedent:
  - `crates/ingest/syn_parser/src/parser/relations.rs`
  - `crates/ingest/syn_parser/src/parser/nodes/ids/internal/mod.rs`
  - `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_families.rs`
  - `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs`
- Existing `CodeGraph` stores functions, defined types, type graph nodes, impls, traits, syntactic relations, modules, consts, statics, macros, imports, and unresolved nodes in `crates/ingest/syn_parser/src/parser/graph/code_graph.rs`.
- Existing function and method nodes store body text but do not currently expose a typed body-expression graph:
  - `crates/ingest/syn_parser/src/parser/nodes/function.rs`
  - `crates/ingest/syn_parser/src/parser/nodes/method.rs`
- `rust-analyzer` is useful as a reference for architecture: syntax identifies callable expressions, semantic analysis resolves call targets. It should not be copied directly because Ploke does not have rust-analyzer's full semantic engine.
- The first implementation should be conservative and exact. Missing precision should become explicit unresolved/unsupported statuses, not weakly typed relations.

## Core design principle

Parser-side call graph facts must be strongly typed. The transform/database layer may flatten them into strings and UUID columns, but only after the typed parser layer has proven endpoint membership.

Avoid parser-side shapes like:

```rust
struct CallEdge {
    source_id: Uuid,
    target_id: Uuid,
    source_kind: String,
    target_kind: String,
    relation_kind: String,
}
```

Prefer typed endpoint families like:

```rust
pub enum CallRelation {
    FreeFunction {
        source: PathCallSiteId,
        target: FunctionNodeId,
    },
    AssociatedFunction {
        source: PathCallSiteId,
        target: MethodNodeId,
    },
    Method {
        source: MethodCallSiteId,
        target: MethodNodeId,
    },
    TupleStructConstructor {
        source: PathCallSiteId,
        target: TupleStructConstructorId,
    },
    TupleVariantConstructor {
        source: PathCallSiteId,
        target: TupleVariantConstructorId,
    },
}
```

The resolver may inspect broad candidates while searching, but relation construction must happen only after source and target proof.

## Proposed parser-side type model

### 1. Add a separate call-site universe

Introduce a call-site identity universe distinct from `NodeId` and `TypeId`.

Current location:

- `crates/ploke-core/src/lib.rs` for the base `CallId`, shared across parser, transform, and database crates when needed.
- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs` for typed wrappers and call endpoint families.

Current base type:

```rust
pub enum CallId {
    Synthetic(Uuid),
}
```

Do not back call-site wrappers with `NodeId`. A call expression occurrence is not a code definition node and does not have `NodeId`'s path-resolution lifecycle.

Call sites are expression occurrences, so `CallId` generation can include:

- crate namespace
- file path
- owning item ID
- call syntax kind
- whole call byte range
- callee byte range
- normalized callee path/name when present
- configuration bytes if needed

This is more span-sensitive than item IDs, but call expressions are lower-level source coordinates. Document that tradeoff explicitly.

### 2. Add typed call-site IDs

Current file:

- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs`

Typed wrappers over `CallId`:

```rust
PathCallSiteId
MethodCallSiteId
DynamicCallSiteId
MacroCallSiteId
AnyCallSiteId
```

Meaning:

- `PathCallSiteId`: `foo()`, `crate::m::foo()`, `Type::new()`, tuple constructor syntax.
- `MethodCallSiteId`: `receiver.foo()`.
- `DynamicCallSiteId`: expression call where the callee is not a simple path target, such as `(f)()` or `make_fn()()`.
- `MacroCallSiteId`: `macro!(...)`, kept separate because macro expansion changes call visibility.
- `AnyCallSiteId`: finite union of the above.

### 3. Add typed owner family

Likely near existing category enums in:

- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/mod.rs`

Candidate family:

```rust
CallBodyOwnerId =
    FunctionNodeId
  ∪ MethodNodeId
  ∪ ConstNodeId
  ∪ StaticNodeId
```

If the first milestone intentionally supports only function/method bodies, define a narrower initial family:

```rust
CallableBodyOwnerId = FunctionNodeId ∪ MethodNodeId
```

Then add const/static only when initializer call extraction is implemented. Do not accept `AnyNodeId` as an owner just to simplify storage.

### 4. Add typed callable target families

Likely in a new endpoint-family module or alongside call IDs:

```rust
CallableTargetId =
    FunctionNodeId
  ∪ MethodNodeId
  ∪ TupleStructConstructorId
  ∪ TupleVariantConstructorId
```

Refined constructor IDs should require payload proof:

```rust
TupleStructConstructorId  // refined from StructNodeId only if the struct has tuple fields
TupleVariantConstructorId // refined from VariantNodeId only if the variant is tuple-like
```

Do not let every `StructNodeId` or `VariantNodeId` become a callable constructor target.

## Proposed call node model

Add call-site nodes under a new parser module, for example:

- `crates/ingest/syn_parser/src/parser/nodes/calls.rs`

Candidate shape:

```rust
pub enum CallNode {
    Path(PathCallNode),
    Method(MethodCallNode),
    Dynamic(DynamicCallNode),
    Macro(MacroCallNode),
}

pub struct PathCallNode {
    pub id: PathCallSiteId,
    pub path: Vec<String>,
    pub span: (usize, usize),
    pub callee_span: (usize, usize),
    pub arg_count: usize,
    pub generic_arg_count: usize,
    pub cfgs: Vec<String>,
}

pub struct MethodCallNode {
    pub id: MethodCallSiteId,
    pub method_name: String,
    pub receiver: ReceiverSyntax,
    pub span: (usize, usize),
    pub method_span: (usize, usize),
    pub arg_count: usize,
    pub generic_arg_count: usize,
    pub cfgs: Vec<String>,
}
```

`ReceiverSyntax` should be typed enough for conservative later resolution, for example:

```rust
pub enum ReceiverSyntax {
    SelfValue,
    LocalName(String),
    Path(Vec<String>),
    Other { text: String },
}
```

Do not over-model full expression semantics in the first pass.

## Proposed relation model

Add call-specific relation enums rather than overloading `SyntacticRelation`.

Likely location:

- `crates/ingest/syn_parser/src/parser/relations.rs`, or a new `parser/call_relations.rs` re-exported from parser modules.

Structural relation:

```rust
pub enum CallSiteRelation {
    BodyContainsCall {
        source: CallBodyOwnerId,
        target: AnyCallSiteId,
    },
}
```

Semantic resolved relation:

```rust
pub enum CallRelation {
    FreeFunction {
        source: PathCallSiteId,
        target: FunctionNodeId,
    },
    AssociatedFunction {
        source: PathCallSiteId,
        target: MethodNodeId,
    },
    Method {
        source: MethodCallSiteId,
        target: MethodNodeId,
    },
    TupleStructConstructor {
        source: PathCallSiteId,
        target: TupleStructConstructorId,
    },
    TupleVariantConstructor {
        source: PathCallSiteId,
        target: TupleVariantConstructorId,
    },
}
```

Resolution outcomes that are not proven edges should stay separate:

```rust
pub enum CallResolutionStatus {
    Resolved(CallRelation),
    External {
        source: AnyCallSiteId,
        path: Vec<String>,
    },
    Dynamic {
        source: DynamicCallSiteId,
    },
    Ambiguous {
        source: AnyCallSiteId,
        candidates: Vec<CallableTargetId>,
    },
    Unsupported {
        source: AnyCallSiteId,
        reason: CallResolutionGap,
    },
}
```

Only `CallResolutionStatus::Resolved` contributes to the durable typed `CallRelation` edge set.

## Parser collection plan

### Step 1: Add call-site structural storage

Files likely to change:

- `crates/ingest/syn_parser/src/parser/graph/code_graph.rs`
- `crates/ingest/syn_parser/src/parser/graph/mod.rs`
- `crates/ingest/syn_parser/src/parser/nodes/mod.rs`
- new `crates/ingest/syn_parser/src/parser/nodes/calls.rs`
- ID modules under `crates/ingest/syn_parser/src/parser/nodes/ids/internal/`

Add to `CodeGraph`:

```rust
pub call_sites: Vec<CallNode>,
pub call_site_relations: Vec<CallSiteRelation>,
```

Keep semantic relations separate, likely as resolver output rather than stored immediately in `CodeGraph`.

### Step 2: Add a body visitor

Files likely to change:

- `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
- possibly new `crates/ingest/syn_parser/src/parser/visitor/call_site_visitor.rs`

Add a dedicated visitor over function/method bodies that captures:

- `syn::Expr::Call`
- `syn::Expr::MethodCall`
- `syn::Expr::Macro`

For `Expr::Call`, classify the callee expression:

- `Expr::Path` -> `PathCallNode`
- anything else -> `DynamicCallNode`

For `Expr::MethodCall`, emit `MethodCallNode`.

Important implementation detail: collect call sites while the original `syn` body is available, not by reparsing `FunctionNode.body` string later.

### Step 3: Preserve owner scope with typed state

During `visit_item_fn`, `visit_item_impl`, and `visit_item_trait`, ensure body call extraction knows the typed current owner:

- standalone function body -> `FunctionNodeId`
- impl method body -> `MethodNodeId`
- trait default method body -> `MethodNodeId`

Emit `CallSiteRelation::BodyContainsCall { source, target }` with a typed owner family.

## Resolution plan

Add a resolver similar to type resolution v2.

Likely new file:

- `crates/ingest/syn_parser/src/resolve/call_resolution.rs`

Public shape:

```rust
pub struct CallRelationReport {
    pub relations: Vec<CallRelation>,
    pub statuses: Vec<CallResolutionStatus>,
    pub summary: CallRelationSummary,
}

pub struct CallRelationResolver<'a> {
    graph: &'a ParsedCodeGraph,
    tree: &'a ModuleTree,
    // indexes
}
```

Run it after module tree construction and after typed type resolution where useful.

### Resolver indexes

Build indexes for:

- call site by ID
- body owner by call site
- functions by containing module and name
- methods by impl/trait owner and name
- impls by resolved self type target, where available
- trait methods by trait and name
- imports/re-exports via existing `ModuleTree` / relation indexes

### Path call resolution

Milestone A should resolve:

```rust
callee()
module::callee()
crate::module::callee()
self::callee()
super::callee()
```

including local imports/re-exports where existing module-tree lookup supports it.

Proof boundary:

```rust
fn prove_function_target(candidate: AnyNodeId) -> Option<FunctionNodeId>;
fn prove_method_target(candidate: AnyNodeId) -> Option<MethodNodeId>;
```

### Associated function resolution

Resolve `Type::new()` conservatively when:

- `Type` resolves through type graph to a local type target.
- an inherent impl for that self type exists.
- a method of matching name exists in that impl.

Emit:

```rust
CallRelation::AssociatedFunction {
    source: PathCallSiteId,
    target: MethodNodeId,
}
```

### Method call resolution

Do not attempt full Rust method resolution initially.

First supported cases:

- `self.method()` inside an impl, using the current impl owner.
- parameter receiver with explicit parameter type:
  ```rust
  fn f(x: Foo) { x.method(); }
  ```
- local receiver with explicit annotation:
  ```rust
  let x: Foo = ...;
  x.method();
  ```

If receiver type proof fails, emit an explicit `Unsupported` or `Dynamic` status, not a broad or nullable relation.

## Transform and database plan

After parser-side typed facts exist, add persistence.

Likely files:

- `crates/ingest/ploke-transform/src/schema/edges.rs`
- `crates/ingest/ploke-transform/src/schema/mod.rs`
- `crates/ingest/ploke-transform/src/transform/mod.rs`
- possibly new `crates/ingest/ploke-transform/src/transform/call_graph.rs`
- `crates/ploke-db/src/` for query APIs
- `crates/ploke-db/tests/unit/` for DB traversal tests

DB schema can be flat/stringly because it is a projection boundary:

```text
call_site(id, owner_id, owner_kind, call_kind, span, callee_span, callee_path, callee_name, arg_count, cfgs)
call_relation(call_site_id, caller_id, target_id, target_kind, relation_kind)
call_resolution_status(call_site_id, status_kind, reason, candidate_ids?)
```

But all inserts should be fed by typed parser structs/enums, not ad-hoc strings from resolver internals.

## Staged implementation and testing plan

Use the existing type-resolution test ladder as the model, especially:

- `docs/testing/TYPE_RESOLUTION_COVERAGE.md`
- parser fixture tests in `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`
- the shared real-corpus matrix in `crates/test-utils/src/type_shape_matrix.rs`
- DB matrix tests in `crates/ploke-db/tests/unit/type_graph_queries/matrix.rs`
- RAG and TUI coverage in `crates/ploke-rag/src/core/unit_tests.rs` and `crates/ploke-tui/src/tools/request_code_context.rs`

The call-graph rollout should intentionally repeat that pattern: first artificial fixtures with exact edge/source assertions, then transform/database projection tests, then real-target matrix contracts, then application-layer payload tests. Each stage must include positive, negative, edge, and “known not covered” assertions so we can state exactly what is covered and what is not.

### Stage 0: Coverage inventory and fixture design

Objective: define the call-shape matrix before implementation starts.

Files likely to create or update:

- `docs/testing/CALL_GRAPH_COVERAGE.md`
- `tests/fixture_crates/fixture_call_graph/`
- eventually `crates/test-utils/src/call_shape_matrix.rs`

Steps:

1. Create a small artificial fixture crate with intentionally named functions, modules, impls, trait defaults, const/static initializers if included, dynamic calls, macro calls, repeated same-body calls, and ambiguous names.
2. Write a coverage document modelled after `TYPE_RESOLUTION_COVERAGE.md` with rows for:
   - source code item
   - call syntax shape
   - expected owner selector
   - expected call-site kind
   - expected target or non-resolution status
   - expected stage where it is asserted
3. Explicitly list out-of-scope cases for the first milestone, such as full autoderef/autoref method resolution, blanket impls, trait selection, macro-expanded calls, external target resolution, and call targets hidden behind dynamic function values.

Verification: no production code yet; review the fixture and matrix rows before implementation.

### Stage 1: Parser structural call-site extraction on artificial fixtures

Likely location:

- `crates/ingest/syn_parser/tests/`
- possibly a new module under existing parser fixture infrastructure

Add strict tests for call-site extraction before resolver work:

1. standalone free function call:
   ```rust
   fn callee() {}
   fn caller() { callee(); }
   ```
2. repeated calls from the same body produce distinct call-site IDs and stable source spans.
3. path calls across module forms:
   ```rust
   mod m { pub fn callee() {} }
   fn caller() { m::callee(); crate::m::callee(); self::m::callee(); }
   ```
4. owner-sensitive method call extraction:
   ```rust
   impl S { fn m(&self) {} fn caller(&self) { self.m(); } }
   ```
5. dynamic call extraction:
   ```rust
   fn caller(f: fn()) { f(); (f)(); make_fn()(); }
   ```
6. macro call is structurally recorded as a macro call site, but not resolved as a normal call.
7. edge cases: nested calls, calls inside blocks/conditionals/matches/closures/async blocks if supported by the visitor, calls in trait default methods, calls in impl methods, and calls with generic arguments.
8. negative/cardinality assertions: each expected call site appears exactly once; unsupported expression forms produce the intended structural kind instead of being dropped or misclassified.

Stage gate:

- parser fixture command passes for `call_site` extraction.
- coverage document records every fixture row as pass, fail, ignored, or explicitly out of scope.

### Stage 2: Parser resolver tests on artificial fixtures

Add strict endpoint assertions, mirroring typed type relation tests:

- exact source `PathCallSiteId` -> exact target `FunctionNodeId` for local free functions.
- exact source `PathCallSiteId` -> exact target `FunctionNodeId` for `module::function`, `crate::module::function`, `self::module::function`, and `super::module::function` where supported.
- exact source `PathCallSiteId` -> exact target `MethodNodeId` for conservative associated functions such as `Type::new()` only after self-type proof exists.
- exact source `MethodCallSiteId` -> exact target `MethodNodeId` for `self.method()` and explicitly typed receiver cases that the milestone supports.
- no relation emitted for unsupported dynamic calls.
- external dependency roots produce `External` or `Unsupported` status, not fake internal relations.
- ambiguous cases produce ambiguity status, not arbitrary first candidate.
- tuple struct and tuple variant constructor calls resolve only through refined constructor wrappers, never from all structs or all variants.

Stage gate:

- resolver tests assert both positive relations and exact non-relation/status outcomes.
- exact-source tests reject extra rows for the same call site.

### Stage 3: Compile-time endpoint-family safety tests

Where practical, add Rust type-level compile checks through ordinary unit tests that exercise construction APIs. The main guarantee is that incorrect combinations fail to type-check, for example:

- cannot construct `CallRelation::Method` from a `PathCallSiteId`.
- cannot construct `CallRelation::FreeFunction` targeting a `StructNodeId`.
- cannot construct tuple constructor relation without refined constructor target wrapper.
- cannot use an arbitrary `AnyNodeId` as a call body owner.

Stage gate:

- parser crate tests prove construction APIs only expose valid endpoint families.
- no broad `Uuid`/stringly constructor exists on the parser-side relation path.

### Stage 4: Transform and database projection tests

Once parser and resolver pass, add projection/persistence tests before real-corpus expansion:

- assert `call_site` rows persist with correct owner IDs, owner kinds, call kinds, spans, callee text/path/name, argument counts, and configuration fields.
- assert `call_relation` rows persist only for typed resolved relations.
- assert `call_resolution_status` rows preserve unsupported/external/dynamic/ambiguous outcomes without becoming relations.
- add outgoing-call and incoming-call query tests.
- add invariant tests: no relation references missing call sites, no call site has two contradictory exact statuses, typed target kind matches relation kind, repeated same-body call sites remain distinct.
- add no-target/unsupported tests equivalent to the type graph’s no-target cases so unsupported syntax remains visible without fabricating edges.

Stage gate:

- focused transform tests pass.
- focused DB query/invariant tests pass.
- parser-side typed facts remain the only insert source for resolved relations.

### Stage 5: Shared real-target call-shape matrix

After schema and artificial fixtures are stable, introduce a real-target matrix equivalent to `type_shape_matrix.rs`.

Likely file:

- `crates/test-utils/src/call_shape_matrix.rs`

Use source-pinned real corpora and backup fixtures in the same spirit as the type-resolution matrix. For each row, include:

- fixture identity and checkout/source pin
- owner selector
- call-site selector
- expected target selector or expected status
- call kind and relation kind
- depth or containment context if the call is nested
- coverage flags such as parser, database, RAG, TUI, live/manual

Include real rows for at least:

- local free function calls
- module-qualified free function calls
- associated functions with local self-type proof
- `self.method()` within inherent impls
- explicitly typed receiver method calls if supported
- repeated same-named calls in one owner
- dynamic/external/unsupported calls that must not become relations
- macro call sites that must remain structural-only unless already visible in parsed source

Stage gate:

- DB matrix tests pass over registered real backup fixtures.
- source-parse variants are either run and recorded, or deliberately ignored with the reason documented.
- coverage document distinguishes “backup persisted snapshot proof” from “fresh source parse proof.”

### Stage 6: RAG and application-layer payload tests

Only after DB queries are proven, add application-level proof analogous to the current RAG/TUI type-context coverage.

Tests should be separated by responsibility:

- owner-seeded RAG expansion proves call-neighbor context assembly independent of search.
- search retrieval smoke tests prove that expected owners can be found by sparse/dense/hybrid search.
- `request_code_context` or successor tool payload tests prove the production payload contains the expected call context by UUID/strict selector identity, not by ambiguous labels.
- live/provider tests remain ignored/manual until event call-id correlation and payload identity are strict enough to be authoritative.

Stage gate:

- DB path exists for the matrix row.
- RAG owner-seeded expansion materializes the expected call context.
- TUI/tool tests assert exact payload identity and do not rely on BM25 seed selection as proof of call expansion.

### Stage 7: Full verification and documentation sync

Before claiming the feature is working:

- run focused parser/resolver tests.
- run focused transform/database tests.
- run real-target matrix tests.
- run application-layer tests that are not quarantined/manual.
- run workspace compile/test checks appropriate to the touched crates.
- update `docs/testing/CALL_GRAPH_COVERAGE.md` with command output, pass/fail/ignored state, known gaps, and weaknesses in the current approach.

This stage should mirror the type-resolution coverage discipline: never claim exhaustive Rust semantic coverage; claim only the fixture, matrix, and application surfaces that were actually exercised.

## Verification commands to run when execution is approved

Do not run these during plan mode.

Parser-focused checks:

```bash
cargo test -p syn_parser call_site -- --nocapture
cargo test -p syn_parser call_relation -- --nocapture
```

Feature-aware typed graph checks, if call resolution depends on typed type graph:

```bash
cargo test -p syn_parser --features typed_type_graph call_relation -- --nocapture
```

Transform and database checks after persistence is added:

```bash
cargo test -p ploke-transform call_graph -- --nocapture
cargo test -p ploke-db call_graph -- --nocapture
```

General compile check:

```bash
cargo check --workspace 2>&1 | rg -A 8 E0
```

Repository guidelines say test runs should use a sub-agent when actually executing implementation verification.

## Risks and tradeoffs

### 1. Call-site identity stability

Expression-level IDs are inherently more span-sensitive than item IDs. If call-site IDs include byte ranges, edits near the expression can change IDs. That may be acceptable for initial ingestion, but it should be documented.

Alternative: compute call-site ordinal paths within owner bodies. That may be more stable under whitespace edits but less stable under expression reordering and more complex to implement.

### 2. Method resolution complexity

Full Rust method resolution requires autoderef, autoref, trait selection, generic bounds, blanket impls, and external crate knowledge. Initial implementation should explicitly avoid promising full precision.

### 3. Macro expansion

Without macro expansion, macro-generated calls cannot be faithfully represented. Initial behavior should record macro invocation sites and mark expanded calls as unsupported unless already visible in parsed source.

### 4. Constructor refinement

Tuple struct and tuple variant callability should require payload proof. This adds more ID-family work but preserves the parser's current correctness standard.

### 5. Schema churn

Adding call graph persistence will affect backup fixtures. Do parser/resolver work first; add database persistence once the typed model is stable.

## Open questions

1. Should base `CallId` live in `ploke-core`, or should call-site IDs remain parser-local until persistence forces a shared core type?
2. Should const/static initializer calls be included in the first owner family, or delayed until after function/method bodies are solid?
3. Should trait default methods and impl methods share the same `MethodNodeId` target family, or do we need a refined `CallableMethodId` to distinguish callable associated functions from non-callable method-like declarations?
4. How much receiver syntax should be captured initially: just text/name/path, or a small expression tree sufficient for local typed receiver inference?
5. Should unresolved statuses be persisted in the first database milestone, or kept in parser reports until the resolved edge schema is stable?

## Recommended first milestone

Implement only the structural call-site graph with compile-time-safe IDs and owner relations:

```text
CallBodyOwnerId -> AnyCallSiteId
```

Do not resolve calls yet. This milestone proves the call-site universe, owner family, ID generation, body visitor, and fixture tests without taking on method resolution complexity.

Second milestone:

```text
PathCallSiteId -> FunctionNodeId
```

for local free functions and module/import path calls.

Third milestone:

```text
PathCallSiteId   -> MethodNodeId
MethodCallSiteId -> MethodNodeId
```

only for conservative inherent impl cases with explicit receiver/self type proof.
