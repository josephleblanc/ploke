# 2026-07-14 Binding Tracking Proof Carrier Plan

Date: 2026-07-14
Title: Binding tracking proof carrier plan
Short description: implementation plan for the next call-graph semantic-expansion carrier after the small binding/type-aware parser slices have saturated.

Related planning files:
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`2026-07-01_call-graph-goal-coverage-matrix.md`](2026-07-01_call-graph-goal-coverage-matrix.md)
- [`2026-07-05_binding-type-aware-resolver-plan.md`](2026-07-05_binding-type-aware-resolver-plan.md)
- [`2026-07-07_call-graph-usage-question-gap-audit.md`](2026-07-07_call-graph-usage-question-gap-audit.md)
- [`2026-07-13_call-graph-next-bucket-inventory.md`](2026-07-13_call-graph-next-bucket-inventory.md)
- [`../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`](../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md)

## Current Position

The current call-graph query surface is strong enough for impact, navigation,
proof context, targetless frontier, runtime-dispatch need, and exact tool
payload questions. The remaining high-value gaps are not mostly query-helper
gaps. They are proof-input gaps:

- non-local callable value flow;
- callable fields whose construction path is not locally complete;
- async future value flow and poll/resume evidence across function boundaries;
- broader trait-object dispatch where the concrete target is not proven by a
  local initializer or complete private caller set;
- macro/generated source bodies that require a bounded template rather than
  general expansion.

The existing `LocalBindingProof` machinery has carried many narrow parser-local
cases, but it is intentionally scoped to extraction/resolution in one body. The
next larger step needs a persistent, typed proof carrier that follows the same
style as syntax relations, type graph relations, `call_body_owner`, and
`call_callee_evidence`: encode exact evidence, project it to DB explicitly, and
keep unresolved shapes visible as blockers instead of promoting edges.

## Established Patterns To Reuse

- **Typed endpoint families first.** The call graph split call-site IDs into a
  `CallId` universe instead of overloading `NodeId`. Binding/value-flow evidence
  should use typed endpoint IDs or typed owner/site references rather than
  flattened string keys.
- **Small relation modules.** Type graph projection uses typed role/kind
  wrappers at insertion boundaries. Binding projection should avoid one large
  catchall file and keep parser model, transform rows, DB decode, and query
  helpers separated.
- **Source facts before resolver edges.** `call_callee_evidence` persists
  parser-side callee evidence even when no local edge is admitted. Binding
  evidence should do the same: record why a value is or is not proven before
  resolver code consumes it.
- **No inferred authority from body text alone.** Body strings, local names, and
  spans are lookup aids. Resolver authority should come from typed owners,
  callsite IDs, type graph edges, import facts, and exact initializer/argument
  relationships.
- **Fail closed with proof blockers.** Existing external summary and runtime
  dispatch summaries discharge authoring needs without fabricating local
  traversal edges. Binding tracking should preserve this rule.

## Proposed Carrier Shape

Keep the first implementation narrower than a general borrow checker or Rust
MIR substitute. The initial carrier should describe only source-visible,
acyclic evidence that the existing parser already inspects.

### Binding Facts

Represent value bindings under a call body owner:

- owner ID: `CallBodyOwnerId`;
- binding ID: new typed local-binding identity, or a typed composite of owner
  plus binding span if a new ID universe is deferred;
- binding kind: parameter, let binding, pattern binding, field projection,
  tuple projection, return expression, or generated/local-item binding;
- display name and source span;
- optional explicit type path or type-use ID when already available;
- optional initializer callsite/path/closure/local-item target when exact.

### Binding Edges

Represent exact one-step relationships:

- owner contains binding;
- binding aliases another binding;
- binding initialized by path item;
- binding initialized by closure owner;
- binding initialized by callsite result;
- binding projects a named field or tuple index from another binding;
- function returns binding or callsite result;
- call argument supplies binding/path/closure to a private callee parameter.

Each edge must keep its source evidence narrow: owner, span, syntactic shape,
and target endpoint. Missing, public, multi-target, external, or expression-only
cases should project a blocker reason rather than a weak edge.

### DB Projection

Add relations only after a parser model exists:

- `local_binding`
- `local_binding_edge`
- `local_binding_evidence`
- `local_binding_resolution_status`, if needed for candidate-only or blocked
  binding proof states

The projection should mirror nearby type graph and call graph projection style:
small modules, typed insertion helpers, strict decode/query validation, and no
permissive fixture import fallback.

## First Implementation Slice

Start with a projection-only slice before changing resolver behavior.

Candidate source oracles:

1. Fixture returned-callable rows already using parser-local proof:
   `make_forwarded_returned_closure()` and
   `call_forwarded_returned_closure()`.
2. Axum callable field rows that remain targetless:
   `boxed.rs:120 (self.into_route)(self.router, state)` and
   `serve/listener.rs:236 (self.tap_fn)(&mut io)`.
3. The forwarded returned async future blocker:
   `make_forwarded_returned_async_future()` and
   `call_forwarded_returned_async_future()`.

Preferred first slice:

- project binding facts for the fixture returned-callable pair;
- assert in `ploke-db` that the caller, producer, returned binding/callsite,
  and closure owner are linked by binding evidence;
- do not add a new traversal edge beyond the edge already admitted by the
  existing resolver;
- expose blocker/proof rows for the async returned-future counterpart without
  admitting a path to `local_target`.

This gives DB/RAG/TUI query work a durable proof substrate without widening
runtime dispatch or async poll/resume semantics.

## 2026-07-14 Projection-First Checkpoint

Committed slice: `2f8aac74f Project returned callable binding evidence`.

What is complete:

- `ploke-db` proof projection now emits a strict proof-only
  `binding_evidence` fact for `ReturnedPathCall` and
  `AwaitedReturnedPathCall` callee evidence.
- Proof-store validation recognizes the new fact kind, requires
  `binding_evidence_id`, call-site/caller/build-domain fields, typed
  `binding_evidence_kind`, typed `callee_kind`, non-empty `callee_path`,
  `resolution_state`, `detail`, `source_span`, and `evidence_use`.
- Fixture DB tests assert returned functions, returned function parameters,
  returned closures, awaited returned async closures, same-block stored
  returned async closures, and fail-closed forwarded returned async futures all
  surface searchable returned-callable binding evidence.
- The forwarded returned async future remains targetless and still projects the
  poll/resume proof blocker. No new traversal edge is admitted.

What remains:

- This is not yet the `local_binding` / `local_binding_edge` relation family
  sketched below. It reuses existing parser and transform
  `call_callee_evidence`.
- The next code slice should add one durable parser-owned binding relationship
  only if the source oracle needs a relation beyond callsite-local callee
  evidence. Otherwise keep emitting explicit proof blockers or summaries.
- Do not use this checkpoint to justify broad callable-field, public callable
  parameter, trait-object, or async poll/resume traversal.

## 2026-07-14 Durable Local Binding Edge Checkpoint

Committed slice: durable returned-callable `local_binding` and
`local_binding_edge` projection.

What is complete:

- Parser extraction now records `LocalBindingNode` rows under call body owners
  for exact return-expression evidence.
- The first supported source shapes are:
  - closure literal returns, including
    `make_target_closure() -> || local_target()`;
  - path-call return forwarding, including
    `make_forwarded_returned_closure() -> make_target_closure()`;
  - targetless dynamic returned-path call results, including
    `make_forwarded_returned_async_future() -> make_returned_async_closure()()`.
- Transform projection persists those rows in `local_binding` with a typed
  local binding ID, owner, span, binding kind, source kind, source endpoint,
  and optional returned-path callee fields.
- Transform projection also persists `local_binding_edge` facts for
  owner-to-binding containment and binding-to-source endpoints.
- `ploke-db` exposes `local_bindings_for_owner` and
  `local_binding_edges_for_owner` with strict row-shape validation.
- Fixture-backed DB tests prove the sync returned-closure carrier and the
  forwarded returned async future fail-closed carrier, including the expected
  `OwnerContainsBinding`, `BindingSourceClosure`, and
  `BindingSourceCallResult` edge rows. The async case remains targetless and
  still has no path to `local_target`.
- Active call-graph fixtures were regenerated and `verify-backup-dbs` passed;
  the shared corpus snapshots now include 66 relations.

What remains:

- No general `let` binding, parameter binding, field projection, tuple
  projection, argument-to-parameter flow, callable-field value flow,
  trait-object dispatch, or async poll/resume traversal is admitted by this
  slice.
- The next durable carrier step should be driven by a DB-first source oracle
  that needs one exact relationship beyond return-expression source facts.

## 2026-07-14 Parameter Binding Carrier Checkpoint

Committed slice: durable function/method parameter `local_binding` projection.

What is complete:

- Parser extraction now records `ParameterBinding` rows for value parameters
  under function, method, generated-method, trait-default-method, proc-macro,
  and executable local-item owners.
- Transform projection persists these rows in the existing `local_binding`
  relation with `source_kind = "Parameter"` and no source endpoint columns.
- `local_binding_edge` persists owner-to-parameter containment only. This
  intentionally does not add argument-to-parameter flow, aliases, field
  projection, or new traversal edges.
- `ploke-db` validates the strict parameter row shape and rejects parameter
  rows with source callsite/path/callee endpoint fields.
- Fixture-backed DB coverage proves the existing private callable-parameter
  edge for
  `call_single_function_pointer_param(f: fn() -> i32) { f() }` remains
  traversable through complete caller proof, and also proves the callee
  parameter `f` is queryable as one durable `ParameterBinding` with exactly one
  `OwnerContainsBinding` edge.

What remains:

- The next carrier step is argument-to-parameter proof, not another
  parameter-row widening pass. That should add a typed edge/status row only
  when the source oracle proves the argument binding/path/closure supplied to a
  private callee parameter.

## 2026-07-14 Returned-Callable Binding-Flow Query Checkpoint

Committed slice: DB query helper over returned-callable binding evidence.

What is complete:

- `ploke-db` exposes `returned_call_binding_flows_for_owner` as an
  owner-scoped explanatory query over existing persisted facts. It joins a
  resolved dynamic returned-callable callsite to the matching producer path call
  and the producer's `ReturnExpression` `local_binding` source edge.
- The fixture-backed sync oracle
  `call_forwarded_returned_closure() -> make_forwarded_returned_closure()`
  returns exactly one flow. The row identifies the caller dynamic site, the
  producer path call, the producer return binding, and the
  `BindingSourceCallResult` edge to `make_target_closure()`.
- The forwarded returned async future oracle remains fail-closed. The caller
  that awaits `make_forwarded_returned_async_future()` returns no flow, and the
  producer itself returns no flow because its returned async future dynamic row
  is unsupported, targetless, and still blocked on future value flow /
  poll-resume proof.

What remains:

- This is not a new resolver edge, not a parser extraction expansion, and not a
  general value-flow graph. It is a query surface that proves when existing
  returned-callable traversal has durable binding evidence.
- RAG/TUI can consume this row shape later, but this slice deliberately keeps
  downstream presentation out until the DB helper is stable.

## 2026-07-14 Exact RAG Binding-Flow Surface Checkpoint

Committed slice: exact RAG propagation of returned-callable binding flows.

What is complete:

- `ploke-rag` exposes `exact_returned_call_binding_flows_for_owner`, preserving
  the DB helper's fail-closed semantics.
- `ploke_core::rag_types` defines typed returned-call binding-flow payloads
  rather than adding stringly fields to `CallContextInfo`.
- The fixture-backed exact RAG test proves the sync forwarded-closure flow and
  proves both sides of the forwarded returned async future remain empty.

What remains:

- The helper still does not model future value flow, callable fields, trait
  objects, or general local bindings.

## 2026-07-14 Exact TUI Binding-Flow Payload Checkpoint

Committed slice: exact tool payload propagation of returned-callable binding
flows.

What is complete:

- `ConciseContext` now carries `returned_call_binding_flows` as typed
  `ReturnedCallBindingFlowInfo` payloads.
- `code_item_lookup` and `code_item_edges` obtain those rows from
  `RagService::exact_returned_call_binding_flows_for_owner`, following the
  existing exact-helper pattern for downstream call-graph summaries.
- The fixture-backed lookup and edge-tool tests for
  `call_forwarded_returned_closure()` assert one flow with the expected caller,
  producer path, dynamic closure relation, return-binding source relation, and
  UI count.

What remains:

- This remains a proof/explanation payload over the existing traversal edge; it
  does not add a new traversal relation.
- The forwarded returned async future remains fail-closed in DB/RAG because
  future value flow and cross-function poll/resume proof are still not modeled.
- Broader local bindings, callable fields, trait objects, and async
  poll/resume traversal still require separate source-oracle-driven carrier
  slices.

## 2026-07-14 Returned-Future Execution Proof Checkpoint

Committed slices: DB contextual returned-future execution proof query plus
exact RAG/TUI payload propagation.

What is complete:

- `ploke-db` exposes `returned_future_execution_flows_for_owner` for the
  reviewed forwarded returned async future oracle. The row links the awaiting
  caller's producer path call, the producer return binding, the producer
  dynamic `ReturnedPathCall` future site, the returned async-closure maker
  call, the maker return binding, and the closure body edge.
- `ploke-rag` exposes the same row through
  `exact_returned_future_execution_flows_for_owner`.
- `ConciseContext`, `code_item_lookup`, and `code_item_edges` expose the row as
  `returned_future_execution_flows` with a UI count.
- Focused DB/RAG/TUI tests prove the contextual row and preserve the strict
  boundary: `returned_call_binding_flows` stays empty, the producer itself has
  no caller-context execution flow, and ordinary `call_paths_between` from the
  awaiting caller to `local_target` remains empty.

What remains:

- This is not a resolver edge and not general async poll/resume traversal.
  Broader non-local future value flow still needs typed producer/poll-point
  carriers before any traversal relation can be admitted.

## 2026-07-14 Awaited Future Let-Binding Carrier Checkpoint

Committed slices: `a62bf6d59 Project awaited future let binding evidence` and
`0dfd1d6d3 test: refresh awaited future binding fixtures`.

What is complete:

- Parser extraction now persists a strict `LetBinding` row for same-block
  awaited future call-result evidence. The source oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2375-2377`, where
  `let future = make_returned_async_closure()(); future.await` proves the
  earlier dynamic returned-callable result is polled.
- The row uses the existing typed `DynamicCallResult` source shape and records
  `AwaitedReturnedPathCall` callee evidence for
  `make_returned_async_closure()`. It does not use body-text lookup as
  authority.
- `ploke-db` validation remains strict: `LetBinding` rows may now use
  `PathCallResult` or `DynamicCallResult` sources only when the same typed
  source-shape checks already required for return bindings pass.
- Fixture-backed DB tests assert the `future` binding, its source callsite, and
  the `OwnerContainsBinding` plus `BindingSourceCallResult` edges.
- Active call-graph fixtures were regenerated and `verify-backup-dbs` passed
  with the refreshed committed seed checksums.

What remains:

- This is not non-local returned future value flow, general `let` binding
  tracking, async poll/resume traversal, or a new call edge. The forwarded
  returned async future remains fail-closed until a carrier proves both the
  producer future and the poll point across the function boundary.

## 2026-07-14 Returned Future Flow Query Checkpoint

Committed slices: `c2dcb19f8 Add returned future flow query` and
`f4f40efc6 Expose returned future flows in tools`.

What is complete:

- `ploke-db` exposes `returned_future_flows_for_owner` as a proof query for the
  forwarded returned async future oracle. It joins the awaiting caller's
  `CallResultAwaited` producer path call to the producer's `ReturnExpression`
  `local_binding` and the producer's dynamic `ReturnedPathCall` source.
- Exact RAG and exact `code_item_lookup` / `code_item_edges` payloads expose
  the row as typed `returned_future_flows`.
- The existing fail-closed boundary is preserved: `returned_call_binding_flows`
  remains empty for the forwarded async future caller, the producer still has
  no awaited callsite, and no path is admitted from the caller to
  `local_target`.

What remains:

- This is a proof/explanation row over one reviewed producer/poll boundary, not
  a resolver edge. General non-local future value flow, async poll/resume
  traversal, async callable trait-object dispatch, and arbitrary stored/forwarded
  future aggregates still need separate proof carriers.

## 2026-07-15 Argument-to-Parameter Carrier Checkpoint

Committed slice: `a4bc8715e Project argument parameter edges`.
This added durable one-hop `ArgumentSuppliesParameter` `local_binding_edge`
projection.

What is complete:

- Transform projection now derives `ArgumentSuppliesParameter` edges from
  existing typed facts: resolved `CallRelation::Function` path calls,
  structural `CallArgument` evidence, and the callee's durable
  `ParameterBinding` row.
- The first admitted shape is intentionally narrow: one-hop resolved private
  function path calls whose argument is an already-modeled exact callable
  source. The source oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1523-1528`, where
  `call_single_function_pointer_param_with_local_target()` supplies
  `local_target` to the private helper parameter `f`.
- `ploke-db` accepts the strict `Path -> LocalBinding` edge shape and
  `local_binding_edges_for_owner` returns incoming argument-to-parameter edges
  for bindings owned by the requested callee.
- Fixture-backed DB coverage proves the helper callsite is linked to the
  callee parameter binding while preserving the existing resolved call edges.
- Active call-graph fixtures were regenerated after the projection change and
  copied into the committed seed snapshot paths.

What remains:

- This is not public API argument inference, method argument binding flow,
  tuple projection flow, multi-hop parameter forwarding proof, trait object
  dispatch, or async poll/resume traversal. Those need separate source oracles
  and typed carrier slices.

## 2026-07-15 Named-Field Projection Carrier Checkpoint

Implemented slice: durable named-field projection `local_binding` evidence.

What is complete:

- Parser extraction now persists a constructed base `LetBinding` only when it
  is needed as the base endpoint for an exact dynamic field projection. The
  admitted source oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:870-874`, where
  `NamedCallbackHolder { callback: local_target }` is bound to `holder` and
  `(holder.callback)()` is already resolved by existing parser-local proof.
- The projected binding is stored as `FieldProjection` with a typed
  `BindingProjectsField` edge from `holder.callback` to the base `holder`
  binding. The row records both the projected field path and the exact
  initializer path `local_target`.
- Transform projection and DB decoding follow the existing local-binding
  relation pattern: no table-shape change, strict row validation, and no
  permissive fixture fallback.
- Fixture-backed DB coverage proves the constructed base row, projected field
  row, owner containment edges, projection edge, and the pre-existing dynamic
  call edge to `local_target`.
- The fail-closed counterpart proves public parameter-field calls such as
  `call_field_function_param(holder) { (holder.callback)() }` stay targetless
  and do not emit `FieldProjection` or `BindingProjectsField` rows.
- Active call-graph fixtures were regenerated and verified after the projection
  change, then the regenerated shared snapshots were copied into
  `tests/backup_dbs/`.

What remains:

- This is not general callable-field value flow, tuple/index projection,
  method argument binding flow, complete private field forwarding, trait-object
  dispatch, or a new traversal edge. Those need separate source oracles and
  proof carrier slices.

## 2026-07-15 Constructed Field Argument Carrier Checkpoint

Implemented slice: durable `ArgumentSuppliesParameter` proof for constructed
callable-field arguments.

What is complete:

- Transform projection now treats `CallArgument::Constructed` with exact
  path-valued field initializers as an argument shape eligible for the existing
  `ArgumentSuppliesParameter` edge. This reuses the resolver's already-admitted
  private caller proof instead of adding a new resolver path.
- The fixture-backed DB oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1610-1616`, where
  `call_single_named_field_function_param_with_local_target()` calls the
  private helper with `CallbackHolder { callback: local_target }`, and the
  helper body calls `(holder.callback)()`.
- DB coverage proves the dynamic field call still resolves to `local_target`,
  the caller's helper call resolves to the private helper, and the helper-call
  site now has a durable `ArgumentSuppliesParameter` edge to the helper-owned
  `holder` `ParameterBinding`.
- Active fixture regeneration and registry-backed backup verification passed;
  no tracked snapshot/checksum drift was produced by this projection slice.

What remains:

- This is not general constructed object value-flow, public parameter-field
  proof, tuple/index projection, trait-object dispatch, or a new call edge.
  Broader field forwarding still needs separate exact source oracles and typed
  carrier slices.

## 2026-07-15 Initialized Path Binding Carrier Checkpoint

Implemented slice: durable `LetBinding` source fact for exact initialized path
callable bindings.

What is complete:

- Parser extraction now persists `LocalBindingSource::InitializedPath` for
  local let bindings whose initializer path is already proven exact by the
  existing parser-local `LocalBindingProof::Initialized` machinery.
- The fixture-backed DB oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:185-187`, where
  `call_local_function_item_binding()` binds `let f = local_target;` and then
  calls `f()`.
- Transform projection stores the initializer path in `local_binding.source_path`
  with no source ID, source call kind, callee kind, or callee path. This mirrors
  the established row-level source-fact pattern for source-visible evidence
  without inventing a new edge family.
- `ploke-db` strict decoding accepts `InitializedPath` only for nonempty source
  paths and only as a `LetBinding` source shape.
- Fixture-backed DB coverage proves the existing `f()` call edge to
  `local_target`, the durable `f` binding row, and its owner containment edge.
- Active fixture regeneration and registry-backed backup verification passed;
  no tracked snapshot/checksum drift was produced by this projection slice.

What remains:

- This does not add a `BindingSourcePath` edge, general alias propagation,
  public parameter proof, trait-object dispatch, or broad dynamic callable
  value flow. Those should be separate source-oracle-driven slices with typed
  endpoint families if they need new edges.

## 2026-07-15 Value Alias Binding Carrier Checkpoint

Implemented slice: durable alias source fact plus typed binding-to-binding
alias edge for exact local value aliases.

What is complete:

- Parser extraction now persists `LocalBindingSource::ValueAlias` for local let
  bindings whose initializer path is an exact visible parameter or prior local
  alias according to the existing `LocalBindingProof::ValueAlias` machinery.
- The fixture-backed DB oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1685-1687`, where
  `call_single_aliased_function_pointer_param(f)` binds `let g = f;` and then
  calls `g()`.
- Transform projection stores the alias source path in
  `local_binding.source_path` and derives a `BindingAliasesBinding`
  `local_binding_edge` only when that path names exactly one same-owner
  binding. This lets parameter bindings participate without pushing parameter
  span/ID knowledge into the parser body visitor.
- `ploke-db` strict decoding accepts `ValueAlias` only as a `LetBinding` source
  shape with a nonempty source path and no callsite/callee endpoint fields, and
  validates `BindingAliasesBinding` as `LocalBinding -> LocalBinding`.
- Fixture-backed DB coverage proves the existing `g()` call edge to
  `local_target`, the durable `g` binding row, and the alias edge from `g` to
  the callee-owned `f` parameter binding.
- Active fixture regeneration and registry-backed backup verification passed;
  no tracked snapshot/checksum drift was produced by this projection slice.

What remains:

- This does not add broad alias propagation, public API caller inference,
  arbitrary interprocedural value flow, trait-object dispatch, or a new call
  edge. Multi-hop/ambiguous aliases should be separate typed carrier slices if
  a reviewed source oracle needs them.

## 2026-07-15 Indexed Field Projection Carrier Checkpoint

Implemented slice: durable field-projection binding evidence for exact
constructed indexed callable projections.

What is complete:

- Parser extraction now records `FieldProjection` rows for
  `DynamicCallCallee::IndexedInitializedLocalBinding` when the base endpoint is
  still an exact same-owner constructed local binding. This reuses the existing
  `BindingProjectsField` edge family rather than adding a second indexed-edge
  relation.
- The fixture-backed DB oracles are
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:889-893`, where
  `CallbackArrayHolder { callbacks: [local_target] }` is bound to `holder` and
  called through `holder.callbacks[0]`, and
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:914-916`, where
  `TupleCallbackArrayHolder([local_target])` is bound to `holder` and called
  through `holder.0[0]`.
- The DB projection proof is table-driven with the existing direct
  `holder.callback` case. It asserts the constructed holder binding, the
  projected binding (`holder.callbacks.0` / `holder.0.0`), the
  `BindingProjectsField` edge back to the holder binding, and the pre-existing
  dynamic call edge to `local_target`.
- The fail-closed table now covers public parameter field, indexed named-field,
  and indexed tuple-field calls. Those rows remain targetless and do not emit
  `FieldProjection` or `BindingProjectsField` evidence.
- Active fixture regeneration and registry-backed backup verification passed;
  no tracked snapshot/checksum drift was produced by this projection slice.

What remains:

- This does not add public parameter-field inference, general aggregate
  value-flow, aliases of constructed holders, tuple/index source edge families,
  trait-object dispatch, or a new call edge. Broader aggregate forwarding needs
  its own reviewed source oracle and typed carrier slice.

## 2026-07-15 Aggregate Returned Future Carrier Checkpoint

Implemented slice: durable same-block aggregate storage evidence for returned
async-closure futures.

What is complete:

- The fixture-backed source oracles are
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2380-2382`, where
  `make_returned_async_closure()()` is stored in `futures.0` and awaited;
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2385-2389`, where the
  same returned future is stored in `holder.future` and awaited; and
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2392-2394`, where the
  future is stored in `futures[0]` and awaited.
- Parser extraction now lets tuple, array, and struct awaited-future storage
  reuse the same `future_call_span` classifier as direct local future
  bindings. This marks returned-path dynamic calls as awaited only when the
  exact same-block aggregate slot is awaited.
- Parser extraction records a durable `LetBinding` row for those aggregate
  slots only when the source is a returned-call dynamic result. The rows use
  names like `futures.0` / `holder.future`, `source_kind =
  "DynamicCallResult"`, `source_call_kind = "Dynamic"`, and `callee_kind =
  "AwaitedReturnedPathCall"`.
- Transform and DB decoding reuse the existing `local_binding` and
  `BindingSourceCallResult` relation shape. The DB proof is table-driven and
  asserts the resolved returned async-closure dynamic row, the aggregate
  binding row, and the binding-to-dynamic-call edge.
- Active fixture regeneration and registry-backed backup verification passed.

What remains:

- This is not non-local future value flow, general aggregate alias/value-flow,
  async callable trait-object dispatch, or a general poll/resume traversal
  model. It only proves exact same-block aggregate slots whose producer
  dynamic callsite and poll point are both source-visible in one owner.

## 2026-07-15 Stored Forwarded Future Carrier Checkpoint

Implemented slice: durable aggregate storage evidence for an awaited path-call
future producer whose returned future is explained through existing
returned-future proof queries.

What is complete:

- The fixture-backed source oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2405-2407`, where
  `call_stored_forwarded_returned_async_future_tuple_field()` stores
  `make_forwarded_returned_async_future()` in `futures.0` and awaits that exact
  aggregate slot.
- Parser extraction now permits aggregate awaited-future storage rows sourced
  by either `DynamicCallResult` or `PathCallResult`. This reuses the existing
  `future_call_span`, `return_binding_source`, `local_binding`, and
  `BindingSourceCallResult` machinery rather than adding a new relation family.
- The DB local-binding proof asserts a `LetBinding` named `futures.0` with
  `source_kind = "PathCallResult"`, `source_call_kind = "Path"`, and a
  `BindingSourceCallResult` edge to the awaited producer callsite.
- The returned-future proof query for the same caller identifies the awaited
  producer, the producer return binding, the returned future dynamic row, the
  returned async-closure maker, and the closure body `local_target()` edge.
  Ordinary call-path traversal from the caller to `local_target` remains empty.
- Focused DB tests passed for the new local-binding edge, the new
  returned-future proof flow, and the broader `mixed_proof::returned` module.

What remains:

- This does not admit a new traversal edge, does not model general
  cross-function future value flow, and does not handle arbitrary aggregate
  aliases, async callable trait objects, or general poll/resume. It only records
  the source-visible poll point for one aggregate slot whose producer is an
  existing resolved path call.

## 2026-07-15 Initialized Path Source Function Edge Checkpoint

Implemented slice: durable `BindingSourceFunction` edge for exact initialized
path callable bindings.

What is complete:

- The fixture-backed source oracle remains
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:185-187`, where
  `call_local_function_item_binding()` binds `let f = local_target;` and then
  calls `f()`.
- The parser relation family now includes the typed endpoint relation
  `BindingSourceFunction ⊆ LocalBindingId × FunctionNodeId`, and parsed-graph
  pruning retains it only when both the binding and function target are live.
- Transform derives the relation only from existing exact proof: a resolved
  `CallRelation::Function` path call whose callee is the same
  `InitializedValueBinding` and whose persisted `InitializedPath` binding is
  unique for that owner/name/path.
- DB decoding strictly accepts the edge only as
  `LocalBinding -> Function`.
- Fixture-backed DB coverage now asserts the existing `f()` call edge to
  `local_target`, the durable `InitializedPath` binding row, the
  `OwnerContainsBinding` edge, and the new `BindingSourceFunction` edge.
- Active fixture regeneration and registry-backed backup verification passed
  with no tracked snapshot/checksum drift.

What remains:

- This is not general source-path resolution, public parameter inference,
  alias propagation, callable-field value flow, trait-object dispatch, or a new
  call traversal edge. It is an explanatory proof edge for an already-admitted
  exact initialized-path call.

## 2026-07-15 Field Projection Source Function Edge Checkpoint

Implemented slice: durable `BindingSourceFunction` edges for exact constructed
callable field/index projections.

What is complete:

- The fixture-backed source oracles are the existing projection table:
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:870-874` for
  `holder.callback`, `:889-893` for `holder.callbacks[0]`, and `:914-916` for
  `holder.0[0]`.
- Transform derives a `BindingSourceFunction` edge only from existing exact
  proof: a resolved `CallRelation::DynamicFunction` whose dynamic callee is a
  `FieldInitializedLocalBinding` or `IndexedInitializedLocalBinding`, matched
  against a unique persisted `FieldProjection` binding with the same owner,
  projection path, and initializer path.
- The existing DB projection test now asserts each projection binding has
  `OwnerContainsBinding`, `BindingProjectsField`, and
  `BindingSourceFunction` edges, while the fail-closed public parameter-field
  table still asserts no `FieldProjection` or `BindingProjectsField` rows.
- Active fixture regeneration and registry-backed backup verification passed
  with no tracked snapshot/checksum drift.

What remains:

- This is not general object value-flow, aliases of constructed holders, public
  parameter-field inference, trait-object dispatch, or a new traversal edge.
  It explains already-admitted exact projection calls through typed binding
  evidence only.

## Exit Criteria

For the first carrier slice:

- parser model records one exact binding evidence shape without relying on
  string-only lookup;
- transform projection inserts that evidence through typed helpers;
- `ploke-db` tests prove the exact source oracle and fail-closed counterpart;
- active fixtures are regenerated and verified;
- docs update the coverage matrix once for the completed chunk;
- no broad public callable parameter, callable field, trait-object dispatch, or
  async poll/resume edge is promoted.

## Non-Goals

- No general Rust type inference.
- No MIR-level dataflow.
- No broad trait dispatch or vtable target guessing.
- No public API caller-set inference.
- No external frontier traversal edge from an admitted summary alone.
- No catchall fixture/helper file growth.
