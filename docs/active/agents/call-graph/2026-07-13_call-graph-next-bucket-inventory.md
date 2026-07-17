# 2026-07-13 Call Graph Next Bucket Inventory

Short description: restart-safe inventory of the remaining call-graph work after
the current generated-item, binding/type-aware, DB/RAG/TUI, and proof-context
slices. Use this before selecting another implementation bucket so the next
slice does not repeat already-covered parser breadth.

Related planning files:
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`2026-07-01_call-graph-goal-coverage-matrix.md`](2026-07-01_call-graph-goal-coverage-matrix.md)
- [`2026-07-05_binding-type-aware-resolver-plan.md`](2026-07-05_binding-type-aware-resolver-plan.md)
- [`2026-07-07_call-graph-usage-question-gap-audit.md`](2026-07-07_call-graph-usage-question-gap-audit.md)
- [`2026-07-14_binding-tracking-proof-carrier-plan.md`](2026-07-14_binding-tracking-proof-carrier-plan.md)
- [`../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`](../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md)

## Current Selection Frame

Root plan: `.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`

Current phase: binding/type-aware semantic resolution plus proof-authoritative
downstream surfacing.

Current completed checkpoint:

- Parser-side call-site identity, typed call-site endpoint families, and body
  ownership are established.
- Transform/DB projection, call context, traversal, impact, reach, proof
  projection, RAG, and exact TUI tool surfaces are implemented for the current
  representative matrix.
- Active corpus fixtures have been regenerated with baseline call-graph
  relations and verified.
- The latest binding-carrier projection slice adds the durable `local_binding`
  relation for exact returned-callable return-expression evidence. Parser-owned
  `LocalBindingNode` rows now persist closure literal returns, path-call return
  forwarding, and targetless dynamic returned-path call results. DB tests prove
  the sync returned-closure producer path and the fail-closed forwarded
  returned async future blocker without admitting a new traversal edge.
- The returned-callable binding-flow proof is now surfaced through DB, exact
  RAG, and exact TUI lookup/edges payloads for the sync
  `call_forwarded_returned_closure()` oracle. It remains an explanatory proof
  payload over existing traversal, not a new call edge.
- The latest binding-carrier projection slice adds a strict proof-only
  `binding_evidence` fact for returned-callable callee evidence. Resolved
  returned functions, returned closure values, awaited returned async closures,
  and same-block stored returned async closures now project searchable
  `returned_callable` binding evidence. Un-awaited and forwarded returned async
  future rows project the same binding evidence with blocked resolution plus
  the existing poll/resume blocker, without admitting a path to `local_target`.
- The latest committed returned-async slice records returned-path dynamic
  callee awaitedness, resolves `make_returned_async_closure()().await` to the
  returned async-closure owner, also resolves the same returned future after a
  same-block `let future = ...; future.await` binding, and keeps the un-awaited
  `make_returned_async_closure()()` targetless with no traversal edge.
- The latest durable binding slice persists the same-block stored returned
  async future proof as a `LetBinding` sourced by a `DynamicCallResult`. The
  fixture oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2375-2377`, where
  `let future = make_returned_async_closure()(); future.await` proves the
  earlier dynamic call result is polled. This adds owner/source
  `local_binding_edge` evidence only; it does not model non-local future flow
  or add a new traversal edge.
- The latest source-function binding slices add typed
  `BindingSourceFunction ⊆ LocalBindingId × FunctionNodeId` edges for exact
  initialized path bindings and exact constructed field/index projection
  bindings. They are derived only from existing resolved `Function` or
  `DynamicFunction` call relations plus unique persisted binding evidence, so
  they explain already-admitted edges without adding source-path lookup,
  general object value-flow, public parameter-field inference, or new traversal
  semantics.
- The latest stored-forwarded future slice records one aggregate slot whose
  source is an awaited path-call future producer. The fixture oracle is
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2405-2407`, where
  `make_forwarded_returned_async_future()` is stored in `futures.0` and awaited.
  DB proof coverage asserts the `PathCallResult` local binding edge and the
  existing returned-future execution proof row, while ordinary traversal to
  `local_target` remains empty. Exact RAG and exact TUI lookup/edges tests now
  table-drive this owner with the direct forwarded-future owner.
- The latest returned-future query slice exposes the forwarded returned async
  future source oracle as `returned_future_flows` through DB, exact RAG, and
  exact TUI tool payloads. It identifies the awaiting caller's producer path
  call, the producer return binding, and the producer dynamic
  `ReturnedPathCall` source while keeping `returned_call_binding_flows` empty
  and admitting no traversal path to `local_target`.
- The latest returned-future execution proof slice exposes
  `returned_future_execution_flows` through DB, exact RAG, and exact TUI tool
  payloads. It composes the awaiting caller, producer return binding, returned
  future dynamic row, returned async-closure maker, maker return binding, and
  closure body edge for the reviewed forwarded async future oracle, while still
  admitting no ordinary traversal path to `local_target`.
- The latest proof-authority slice keeps memchr
  `memchr/src/tests/substring/mod.rs:94,110` boxed `dyn FnMut` path rows
  targetless, but admits runtime-dispatch summaries that discharge their
  owner-scoped authoring needs without fabricating call edges.
- The latest async-frontier proof-authority slice keeps reviewed axum future
  poll rows unsupported and targetless, but admits runtime-dispatch summaries
  that discharge owner-scoped authoring needs without modeling poll/resume
  traversal. Covered source oracles are `error_handling/mod.rs:251` boxed dyn
  `Future::poll` and `middleware/from_fn.rs:375`
  `BoxFuture::as_mut().poll(cx)`.
- The latest macro-boundary proof slice keeps axum
  `axum-core/src/body.rs:251,252` and `axum/src/util.rs:114,115`
  `assert_eq!`-wrapped `try_downcast::<i32, _>(...)` rows as unsupported
  targetless `Macro` callsites, and proves those rows project
  `macro_expansion_not_available` blockers without flattening the macro
  arguments into local traversal edges.
- The latest shared-matrix downstream slice splits bounded RAG prompt
  collection from exact RAG lookup coverage. `CallPipelineCoverage::RagApi`
  now means `collect_call_context` bounded owner collection, while
  `CallPipelineCoverage::RagExactApi` means exact `exact_call_context`
  coverage. The chrono `StrftimeItems::parse_next_item`
  `self.queue.is_empty()` external slice frontier has DB, focused reach,
  exact-RAG, and exact TUI lookup/edges coverage. Bounded RAG still truncates
  this late long-owner callsite by design.
- The latest committed runtime slice models the bounded axum
  `middleware/map_response.rs` generated `impl_service!(...)` frontier rows,
  regenerates active fixtures, and preserves `std::mem::replace` as targetless
  `External` rows without converting frontiers into traversal edges.
- The latest committed callable-field slice regenerates the active axum fixture
  and keeps `axum/src/boxed.rs:159,163` `self.layer` as ambiguous
  `DynamicClosure` candidates from the two visible `MethodRouter` closures plus
  the transparent `Router::layer` `map_inner!` closure, with no local traversal
  edge. The remaining `self.into_route` and `self.tap_fn` rows stay targetless
  blockers.
- The latest returned future field producer slice is surfaced through DB,
  exact RAG, and exact TUI lookup/edges for
  `axum/src/error_handling/mod.rs:134-148`
  `HandleError::call`: the returned `HandleErrorFuture { future }` field
  preserves a `return.future -> Box::pin(...)` `BindingSourceCallResult` proof
  while the later `HandleErrorFuture::poll` dyn dispatch remains targetless.
  The exact TUI tools now accept an optional `body_contains` filter so
  owner_trait + owner_type can select the hand-written `HandleError::call`
  owner among generated same-name impls without weakening ambiguity handling.
- The latest method-callback binding slice adds durable
  `BindingSourceFunction` proof for the fixture oracle
  `call_single_result_callback(f) { Ok::<i32, ()>(1).and_then(f) }` with the
  single local caller supplying `local_result_target`. This is an explanatory
  local-binding edge over an already-resolved `MethodCallbackFunction` row; it
  does not add callback traversal breadth or trait-object dispatch.

Next bucket rule: pick one row below only when there is a fresh proof input and
a DB-first assertion. Do not add another fixture-only breadth slice for shapes
already listed as covered in the goal coverage matrix.

Selection update, 2026-07-14:

- Generated/macro-expanded and object/field candidates were rechecked after
  fixture regeneration. The obvious generated rows are already bounded by
  existing generated-item models or explicit macro blockers, and the obvious
  axum callable-field rows either resolve only with exact local evidence or
  correctly remain targetless/ambiguous with runtime-dispatch summaries.
- The next implementation should therefore start the persistent binding
  tracking carrier in
  [`2026-07-14_binding-tracking-proof-carrier-plan.md`](2026-07-14_binding-tracking-proof-carrier-plan.md)
  instead of adding more parser breadth. The first slice is projection-first:
  persist exact returned-callable binding evidence and its async fail-closed
  counterpart before promoting any new resolver edge.
- Status update: the proof-only slice is complete as `binding_evidence` facts
  over existing `call_callee_evidence`, and the first durable parser-owned
  binding relationship is now projected through `local_binding`. The next
  implementation should either add the next exact binding edge/status relation
  needed by a reviewed source oracle, or choose a reviewed async/value-flow
  blocker that uses the carrier before any new traversal edge is admitted.

Selection update, 2026-07-15:

- The one-hop constructed-holder alias projection cases are now covered by the
  existing local-binding projection table. This closes the stale
  "aliases of constructed holders" note for exact copied constructed-holder
  proof, but it does not add general aggregate alias/value-flow semantics.
- Active call-graph fixture regeneration and `verify-backup-dbs` passed. The
  full `fixtures regenerate --all` command still cannot complete typed
  OpenRouter embedding fixtures in this environment because OpenRouter returned
  HTTP 402 for the embedding endpoint.
- Rechecking the candidate buckets still leaves no safe small promotion from
  the nearby projection family. The next code slice should choose one larger
  proof model explicitly: broader object/field callable value flow, async
  poll/resume future flow, generated/macro-expanded source bodies, callable
  trait-object dispatch, or external/source-sink policy summaries.

Follow-up update, 2026-07-15:

- The selected larger proof-model slice for owner-local aliased parameter field
  proof is complete for the exact private complete-caller oracle
  `call_single_aliased_named_field_function_param`. The resolver now normalizes
  exactly one same-owner `ValueAlias` before reusing the existing
  parameter-field proof, and DB tests assert the alias, parameter, argument,
  and dynamic-call facts together.
- Active call-graph fixture regeneration and `verify-backup-dbs` passed after
  the new source oracle. This closes the one-hop owner-local alias case for
  private callable holder parameters, but it does not change the remaining
  need for broad object/field value-flow, trait-object dispatch, async
  poll/resume, generated-body modeling, or policy/effect summaries.

Post-regeneration checkpoint, 2026-07-16:

- `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  completed for all registered active checkout-local and shared call-graph
  corpus fixtures. The regenerated shared corpus snapshots still report 66
  relations. The regenerated shared corpus snapshots were copied into
  `tests/backup_dbs/` so fixture-backed tests consume the refreshed receiver
  payloads.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all registered active fixtures after regeneration and seed promotion.
- Axum `HandleErrorFuture::poll` now preserves the source-visible
  method-result field receiver as `project().future` for
  `self.project().future.poll(cx)`. This is proof evidence only: the dyn
  `Future::poll` frontier remains targetless/unsupported and no local
  traversal edge is admitted.
- Rechecking the usage-question and larger-plan surfaces shows the remaining
  blocker is still proof input, not a missing generic traversal/query helper:
  impact, reach, target context, policy, frontier, runtime-dispatch need,
  external-summary need, test-selection, unsafe-block, RAG exact, and TUI
  exact payload surfaces are already represented by current real-corpus tests.
- The next implementation slice should pick one larger proof model and a fresh
  DB-first source oracle. Do not promote axum `self.tap_fn`, router-side
  `self.into_route`, dyn future `poll`, or other targetless receiver rows into
  traversal edges until a typed proof carrier records the missing
  source-visible receiver/value-flow evidence. Targetless receiver rows
  deliberately preserve the fail-closed frontier when proof is insufficient.

Follow-up downstream checkpoint, 2026-07-16:

- Axum `TapIo::accept` now has proof-only downstream payload coverage for the
  reviewed `self.tap_fn` frontier. The DB query
  `self_field_parameter_flows_for_owner` composes the targetless dynamic
  callsite with the constructor-side `tap_io` return/field/parameter binding
  evidence from `axum/src/serve/listener.rs:116-123` and `:236`. Exact RAG,
  `code_item_lookup`, and `code_item_edges` expose the same
  `self_field_parameter_flows` payload. This keeps `self.tap_fn` unsupported
  and targetless; it only records the source-visible constructor parameter
  frontier. Router-side `self.into_route`, dyn future `poll`, callable
  trait-object dispatch, broader public parameter proof, and general
  interprocedural value flow remain outside this slice.

Future-poll producer checkpoint, 2026-07-16:

- Axum `HandleErrorFuture::poll` now has proof-only downstream payload
  coverage for the reviewed dyn `Future::poll` frontier. The DB query
  `future_poll_field_producer_flows_for_owner` composes the targetless
  `self.project().future.poll(cx)` callsite at
  `axum/src/error_handling/mod.rs:251` with the producer-side
  `HandleError::call` return/field/source proof from
  `axum/src/error_handling/mod.rs:140,147`: `return.future` is sourced by
  `Box::pin(...)` through a `BindingSourceCallResult` edge. Exact RAG,
  `code_item_lookup`, and `code_item_edges` expose the same
  `future_poll_field_producer_flows` payload. This keeps dyn `Future::poll`
  unsupported and targetless; it only records the source-visible returned
  field producer. General poll/resume traversal, trait-object dispatch, and
  broader future value flow remain outside this slice.

Local-binding projection checkpoint, 2026-07-14:

- Source oracles:
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2375-2377` for
  same-block stored returned async future let-binding evidence,
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520` for sync
  returned-closure forwarding, and
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2380-2385` for
  fail-closed forwarded returned async futures.
- Persisted relations: `local_binding` with strict owner, binding kind, span,
  source kind, source endpoint, and optional returned-path callee fields; and
  `local_binding_edge` with typed owner-to-binding and binding-to-source
  endpoint facts.
- Verified behavior: `make_target_closure()` has a `Closure` return binding,
  `make_forwarded_returned_closure()` has a `PathCallResult` return binding
  pointing at the `make_target_closure` callsite, and
  `make_forwarded_returned_async_future()` has a `DynamicCallResult` return
  binding pointing at its targetless `ReturnedPathCall` dynamic row. Each owner
  now has exactly the expected `OwnerContainsBinding` edge plus a
  `BindingSourceClosure` or `BindingSourceCallResult` edge.
- Additional verified behavior: `call_stored_returned_async_closure()` now has
  a `LetBinding` named `future` whose source is the awaited returned-callable
  `DynamicCallResult` for `make_returned_async_closure()()`, with
  `OwnerContainsBinding` and `BindingSourceCallResult` edges.
- Query behavior: `returned_future_flows_for_owner` exposes the non-local
  forwarded returned async future proof row for the awaiting caller only. The
  row joins the caller's awaited `make_forwarded_returned_async_future()` path
  call to the producer's return binding and dynamic `ReturnedPathCall` source,
  but it deliberately does not create a local call edge.
- Query behavior: `returned_future_execution_flows_for_owner` exposes the
  contextual proof path for the same awaiting caller only. The row joins the
  producer-side returned future evidence to the
  `make_returned_async_closure()` maker, the maker's returned async-closure
  binding, and the closure body's `local_target()` edge. This remains an
  explanatory proof row over persisted facts, not a resolver/traversal edge.
- Query behavior: `returned_call_binding_flows_for_owner` now joins the
  caller's resolved returned-callable dynamic row to the producer path call and
  the producer's persisted return binding/source edge. The sync forwarded
  closure oracle returns exactly one explanatory flow; the forwarded returned
  async future oracle returns no flow for both caller and producer because the
  producer-side dynamic row remains targetless. Exact RAG and exact
  `code_item_lookup` / `code_item_edges` payloads expose the sync flow and
  preserve the same fail-closed boundary.
- The latest argument-edge coverage slice table-drives durable
  `ArgumentSuppliesParameter` proof assertions across the already-supported
  forwarded callable chains: constructed holder parameters, function-pointer
  parameters, referenced `&dyn Fn` parameters, and boxed `dyn Fn` parameters.
  This consolidates DB proof coverage for existing traversal and does not add
  a new resolver edge or a new relation family.
- Fixtures: active call-graph fixtures regenerated and verified; shared corpus
  snapshots now report 66 relations.
- Boundary: this is not general let-binding flow, callable-field value flow,
  trait-object dispatch, async poll/resume traversal, or a new call traversal
  edge.

Post-regeneration checkpoint, 2026-07-13:

- Active call-graph fixtures were regenerated and backup DB verification passed
  with no tracked fixture DB diff to commit.
- Current bucket: async poll/resume and future value flow.
- Exit criteria: the next code slice must introduce a typed future-flow proof
  carrier over a reviewed source oracle, or an explicit blocker/summary for a
  reviewed non-local future-flow row. The DB assertion comes first, and no
  local traversal edge is admitted unless the producer future and poll point are
  both identified.
- Status: same-block async closure future bindings, aliases, tuple/named/indexed
  aggregate storage, directly awaited returned async closures, and same-block
  stored returned async closures are already covered. The remaining useful
  source shapes are non-local flow, forwarded returned futures, stored returned
  futures inside broader aggregates, async callable trait objects, and general
  poll/resume.
- Next bucket if this does not produce a concrete source oracle: generated or
  macro-expanded source bodies with a bounded real-corpus template.
- Reason to stay in this bucket: it is the first remaining row in the matrix
  that names a genuine missing proof carrier rather than another same-family
  fixture variant.

Forwarded returned-future checkpoint, 2026-07-13:

- Source oracle:
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2368-2373`.
  `make_forwarded_returned_async_future()` returns the future produced by
  `make_returned_async_closure()()`, and
  `call_forwarded_returned_async_future()` awaits the producer function result.
- DB/RAG status: the caller has one resolved `Function` edge to the producer.
  The producer has a resolved path call to `make_returned_async_closure` plus a
  targetless `Unsupported` dynamic row for the returned async closure call. RAG
  node context also includes the incoming caller row when the producer itself
  is a seed. The transform now persists `ReturnedPathCall` callee evidence for
  that dynamic row with no closure target, and DB proof projection derives a
  `dynamic_dispatch_unbounded` returned-future poll/resume blocker from it.
- Traversal status: `call_forwarded_returned_async_future ->
  make_forwarded_returned_async_future` is traversable as one edge, but there
  is no path from the caller to `local_target`. This is intentional until a
  typed future-flow carrier proves both the producer future and the poll point
  across the function boundary.
- Fixture status: active fixtures were regenerated after adding this source
  oracle, and `verify-backup-dbs` passed for all registered active fixtures.
  The shared corpus snapshots were refreshed under the configured shared
  fixture directory; no tracked backup DB files changed.
- This keeps the async bucket fail-closed and gives the next implementation
  step a concrete blocker: non-local future value flow, not another same-block
  awaited async closure variant. The explicit blocker is proof-only and does
  not admit a local edge.

Forwarded returned-closure checkpoint, updated 2026-07-14:

- Selected bucket: interprocedural callable value flow.
- Source oracle:
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520`.
  `make_forwarded_returned_closure()` returns the closure produced by
  `make_target_closure()`, and `call_forwarded_returned_closure()` immediately
  invokes the producer function result.
- DB/RAG/TUI status: the caller has a resolved `Function` edge to the producer
  and a resolved `DynamicClosure` edge for the outer returned-callable
  invocation. The producer has a resolved path call to `make_target_closure`;
  RAG node context also includes the incoming caller row when the producer is a
  seed, and exact `code_item_lookup` / `code_item_edges` surface the dynamic
  closure row plus target-centered proof context.
- Traversal status: `call_forwarded_returned_closure ->
  make_forwarded_returned_closure` is traversable as one edge, and the bounded
  sync returned-callable carrier admits `call_forwarded_returned_closure ->
  returned closure owner -> local_target`.
- This is a narrow sync callable-value-flow proof, not async poll/resume
  modeling and not broad returned-closure recursion.

## Remaining Candidate Buckets

| Bucket | Current state | Entry criterion for implementation | First proof target |
| --- | --- | --- | --- |
| Broader object/field callable value flow | Axum `self.layer`, `self.tap_fn`, and router-side `self.into_route` remain targetless or candidate-only because construction/value flow is not complete. Strict `self_field_callable` binding evidence now marks those proof frontiers without admitting traversal edges. Runtime-dispatch summaries can discharge authoring needs without fabricating edges. | A source-visible construction path proves every callable value reaching a field, or the implementation introduces a typed value-flow carrier with strict incomplete-proof blockers. | DB real-corpus assertion over one field callsite, preserving no edge when proof is incomplete. |
| Callable trait-object dispatch | Fixture-backed exact `&dyn Fn`, `Box<dyn Fn>`, and `FnMut` local-binding cases are covered only when initializer or complete private-caller proof is exact. Memchr boxed `dyn FnMut` field rows remain targetless blockers; admitted runtime-dispatch summaries discharge their proof-authoring needs without creating traversal edges. | A bounded local source oracle proves a callable trait-object target without public API ambiguity or runtime vtable guessing. | Parser/DB proof for one exact trait-object row; otherwise keep `dynamic_dispatch_unbounded` or an admitted summary when the row is intentionally targetless. |
| Async poll/resume and future value flow | Immediate/same-block async closure calls, aliases, tuple/named/indexed storage, immediate awaited returned async closures, same-block local bindings of returned async-closure futures, direct forwarded returned futures, and one aggregate-stored forwarded returned future path-call producer are covered. Returned struct fields can now preserve a source-visible call-result producer when the tail struct stores a direct call result or a same-block local alias, as in axum `HandleErrorFuture { future }` sourced by `Box::pin(...)`; that axum producer is covered by DB, exact RAG, and exact TUI lookup/edges. Un-awaited returned async closures fail closed. Axum boxed dyn `Future::poll` and `BoxFuture::as_mut().poll(cx)` rows have blocker and admitted runtime-dispatch summary coverage while remaining targetless. Broader non-local future value flow, async callable trait objects, arbitrary aggregate aliases, and general poll/resume remain future work. | A typed future-flow carrier identifies the future producer and poll point without flattening async state-machine execution into ordinary source calls. | One DB traversal or one explicit blocker/summary over a reviewed source oracle. |
| Generated or macro-expanded source bodies | Bounded axum and fixture macro models cover reviewed item/local-item/generated-method cases, including the axum middleware `from_fn`, `map_request`, and `map_response` `std::mem::replace` frontiers. Axum `assert_eq!`-wrapped `try_downcast` rows now have DB proof blockers for unsupported macro expansion. Arbitrary macro expansion remains out of scope. | A specific macro template and invocation pair can be modeled narrowly through normal item/call visitors, with proof metadata explaining the boundary. | DB real-corpus traversal for one generated owner or a targetless proof row for unsupported expansion. |
| Workspace dependency-root/import families | Existing dependency-root proof rows cover the named axum `FromRef`, `Router::new`, `TestClient::new`, and direct plus re-exported `Body::empty` import families. | A new workspace-import source oracle has a resolved edge that needs an explicit proof-authority row; do not add carrier rows just to increase counts. | Target-centered DB/RAG/TUI proof row tied to exact callsite and import path. |
| External summary/source-sink policy | External frontiers, effect seeds, effect policies, runtime summaries, and proof blockers exist for current usage questions. | A concrete usage question requires a new summary or source/sink fact that cannot be answered from current frontier/effect data. | Proof-store validation plus one DB/RAG/TUI query that consumes the new fact. |
| Presentation/prompt policy | RAG prompts and context-plan overlays expose path, proof, and blocker policy for current tools. The shared matrix now distinguishes bounded prompt collection from exact lookup coverage, and the chrono `queue.is_empty` long-owner row has focused exact TUI coverage while bounded RAG remains intentionally capped. | A user-facing tool payload lacks a field already available in DB/RAG, or current copy risks treating blockers as edges. | Exact TUI test over existing DB/RAG data; no parser change. |

Object/field candidate audit, 2026-07-15:

- Rechecked the first remaining object/field bucket before adding more
  resolver breadth. DB, RAG call-context, and exact TUI coverage already
  preserve the three reviewed `axum` `self.layer` `DynamicClosure` candidates
  and keep them candidate-only with zero admitted local traversal edges.
- The only stale gap found was RAG proof-context still documenting/asserting
  two `self.layer` candidates. It now derives and asserts the same three
  source-visible candidates as DB/RAG call-context/TUI:
  `MethodRouter::layer`, `MethodRouter::route_layer`, and the transparent
  `Router::layer` source expression `|route| route.layer(layer)`.
- This is a proof-context alignment only. `self.layer` remains ambiguous,
  router-side `self.into_route` and `self.tap_fn` remain unsupported/targetless
  without callable-field value-flow proof, and no traversal edge was promoted.

Self-field proof-evidence checkpoint, 2026-07-15:

- Added the strict proof-only `binding_evidence_kind = "self_field_callable"`
  carrier for dynamic `self.<field>` callable rows. The producer is limited to
  dynamic callsites whose callee path begins with `self` and whose status is
  `Resolved`, `Ambiguous`, or `Unsupported`.
- DB real-corpus coverage now projects, persists, and queries typed
  `binding_evidence` rows for the resolved axum handler-side
  `self.into_route` row at `boxed.rs:85`, ambiguous `self.layer` at
  `boxed.rs:159,163`, router-side unsupported `self.into_route` at
  `boxed.rs:120`, and unsupported `self.tap_fn` at `serve/listener.rs:236`.
  RAG proof-context coverage asserts the same linked proof rows by callsite,
  owner, resolution state, and proof-only detail.
- This remains proof evidence over already-derived call resolution. The
  resolved handler-side row explains the admitted traversal edge, the ambiguous
  `self.layer` rows remain candidate-only, the unsupported `self.into_route`
  and `self.tap_fn` rows remain targetless, and no runtime-dispatch summary or
  proof-evidence row creates a local traversal edge.

Callable trait-object audit, 2026-07-15:

- Rechecked the callable trait-object bucket after the object/field audit.
  Parser/DB, RAG call-context, and TUI already cover the exact rows for
  `Box<dyn Fn()>`, `&dyn Fn()`, `&mut dyn FnMut()`, and bounded private-caller
  forwarding when the source oracle proves one callable target.
- The narrow drift found was RAG proof-context coverage: blockers and memchr
  targetless runtime-dispatch summaries were covered, but the exact resolved
  callable trait-object rows were not sampled at the proof-context boundary.
  Added a table-driven proof-context fixture that projects and collects the
  existing exact rows without promoting memchr boxed `dyn FnMut` dispatch or
  introducing broader trait-object resolution.

Returned async future proof-context audit, 2026-07-15:

- Rechecked the async poll/resume bucket after the returned-future query and
  tool slices. DB, exact RAG call-context, and exact TUI lookup/edges already
  expose the reviewed returned-future flow and execution-proof payloads.
- The narrow drift found was again at the proof-context boundary: sync
  returned-callable binding evidence was sampled, but the returned async
  closure rows and fail-closed forwarded returned future producer were not.
  Added a proof-context fixture that asserts resolved awaited/stored returned
  async closures project `returned_callable` binding evidence, and the
  forwarded returned future producer projects the same evidence plus the
  `dynamic_dispatch_unbounded` poll/resume blocker without a dynamic call edge.

Generated/macro proof-context audit, 2026-07-15:

- Rechecked the generated and macro-expanded source bucket. RAG proof-context
  already sampled the resolved generated `IntoServiceFuture::new`,
  `routing::post`, and `routing::get_service` macro-boundary rows with their
  admitted expansion summaries.
- The narrow drift found was on the unsupported macro-expansion side: DB and
  exact TUI covered the axum `assert_eq!`-wrapped `try_downcast` rows, but RAG
  proof-context did not sample those real-corpus blockers. Added a focused
  proof-context test for the two `axum-core/src/body.rs` and two
  `axum/src/util.rs` macro callsites, asserting
  `macro_expansion_not_available` blockers and no flattened `try_downcast`
  call edge.

Fixture regeneration and selection checkpoint, 2026-07-16:

- Active call-graph fixtures were regenerated with
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  and verified with
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`. The
  regeneration refreshed checkout-local/shared snapshot state but produced no
  tracked fixture diff.
- The remaining buckets were rechecked after regeneration. Object/field
  callable rows, callable trait-object dispatch rows, async poll/resume rows,
  generated/macro rows, workspace import rows, and runtime/effect summary rows
  already have DB/RAG/TUI coverage for the currently reviewed source oracles.
  Remaining unresolved rows are intentionally blocked, ambiguous, external, or
  summary-driven until a new proof input satisfies the entry criterion in the
  table above.
- Next implementation should therefore not reselect another same-family
  blocker or surface-only proof-context sample. It should begin with a DB-first
  assertion over a new source oracle that either proves a new typed carrier or
  names an explicit missing proof input; otherwise the correct action is
  verification and handoff, not speculative resolver breadth.

Constructor field frontier checkpoint, 2026-07-16:

- Implemented the narrow object/field source-frontier proof for axum
  `ListenerExt::tap_io`. The source oracle is
  `axum/src/serve/listener.rs:116-123`, where public generic
  `tap_io<F>(self, tap_fn: F)` returns `TapIo { listener: self, tap_fn }`.
- Parser extraction now records a constructed `return` local binding plus a
  `return.tap_fn` `FieldProjection` binding only for tail-return struct fields
  initialized directly from a visible parameter. This reuses the existing
  `LocalBinding` / `BindingProjectsField` carrier and does not widen exact
  constructed-argument target proof.
- DB real-corpus coverage asserts that constructor-side frontier in
  `axum_tap_io_constructor_records_field_parameter_frontier`, while separately
  asserting `TapIo::accept` at `axum/src/serve/listener.rs:236` still keeps
  `(self.tap_fn)(&mut io)` unsupported, targetless, and edge-free.
- Exact RAG and exact `code_item_lookup` / `code_item_edges` tests now expose
  the same constructor-side `return`, `tap_fn`, and `return.tap_fn`
  local-binding payloads without enabling proof-context or traversal for
  `tap_io` itself.
- The registered `corpus_axum_call_graph` fixture was refreshed to
  `corpus_axum_call_graph_2026-07-16.sqlite` for this proof carrier.
  Verification passed for `verify-backup-dbs --fixture corpus_axum_call_graph`
  and the broader `ploke-db --features call_graph real_target_matrix::unsupported`
  suite.

Aliased stored forwarded-future checkpoint, 2026-07-17:

- Implemented the narrow async future value-flow proof for
  `tests/fixture_crates/fixture_call_graph/src/lib.rs:2410-2413`, where
  `call_aliased_stored_forwarded_returned_async_future_tuple_field()` stores
  `make_forwarded_returned_async_future()` in `futures.0`, aliases the
  aggregate with `let alias = futures`, and awaits `alias.0`.
- Parser awaited-future tracking now propagates a simple value alias across
  known aggregate future paths, so `alias.0.await` marks the original producer
  call result as polled. This reuses the existing `CallResultAwaited`,
  `PathCallResult`, and `BindingSourceCallResult` carriers.
- DB tests assert the aliased aggregate slot binding, the returned-future proof
  flow, the contextual returned-future execution flow, and the fail-closed
  absence of an ordinary traversal path from the caller to `local_target`.
- Exact RAG and exact `code_item_lookup` / `code_item_edges` payload tests now
  include the aliased stored-forwarded owner beside the existing direct and
  stored forwarded-future owners.
- Active fixtures were regenerated with
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`;
  registry-backed backup verification passed.
- This is not general aggregate alias analysis, interprocedural future value
  flow, async trait-object dispatch, or general poll/resume traversal. It is a
  one-hop same-block alias proof for an already source-visible aggregate future
  slot.

## 2026-07-17 Typed Callable Binding Frontier

- Selected source oracle: `chrono/src/format/parse.rs:378-421`.
  `parse_internal` defines `type Setter = fn(&mut Parsed, i64) ->
  ParseResult<()>`, binds `(width, signed, set): (usize, bool, Setter)` from a
  `match *spec` tuple, then calls `set(parsed, v)?`.
- Expected result: ambiguous finite candidates, not a resolved traversal edge.
  The direct `set(...)` row records every setter candidate visible in the third
  tuple slot of the `match *spec` arms, while `local_binding` preserves a typed
  `LetBinding` proof frontier for `set`.
- Reason for stopping before traversal: the match tuple arms mix associated
  method items such as `Parsed::set_year` with free function items such as
  `set_weekday_with_num_days_from_monday`, and runtime `spec` selects which
  candidate is called.
- Verification: active fixtures were regenerated with
  `cargo run -p xtask --features call_graph -- fixtures regenerate --active`,
  the chrono committed seed was promoted, registry-backed backup verification
  passed, and
  `cargo test -p ploke-db --features call_graph real_target_matrix::fallback -- --nocapture`
  passed with the new chrono fallback candidate case included. Exact RAG
  `local_bindings_exact_expose_chrono_parse_internal_typed_setter_frontier`
  and exact `code_item_lookup` / `code_item_edges` typed-setter payload tests
  also pass, proving the typed frontier is exposed downstream without promoting
  the ambiguous call into a traversal edge.

Post-regeneration source-oracle audit, 2026-07-17:

- Rechecked the remaining buckets after active fixture regeneration and the
  aliased stored forwarded-future slice. The generated/macro-expanded bucket is
  already represented by bounded axum models for `opaque_future!`, top-level
  route helpers, method-routing impl methods, body conversion impls, rejection
  macros, handler/service tuples, middleware services, and error-handling
  service impls.
- Rechecked object/field callable rows against the pinned axum source. The
  router-side `MakeErasedRouter::into_route` row has no selected-source
  construction path for `into_route`, only field copies, so it still fails the
  source-visible proof entry criterion. The `Map.layer` rows remain
  candidate-only, and `TapIo::accept` remains a constructor-parameter frontier
  without a concrete traversal target.
- Rechecked callable argument/value flow. The fixture family already covers the
  current complete private-caller proof shapes, including one-hop and two-hop
  forwarding for function pointers, generic callable bounds, referenced and
  boxed trait objects, holder fields, candidate-only conflicting rows, and
  returned callable parameters. The real axum callable rows found in this pass
  are public/generic/runtime-dispatch shapes and should remain explicit
  blockers or summaries until a larger interprocedural value-flow model exists.
- Rechecked source/sink and build/test policy surfaces. Current DB/RAG/TUI
  coverage already handles effect seeds, caller-supplied and stored
  `effect_policy` rows, guard reports, external summaries, build domains,
  generated test-entrypoint summaries, test-selection payloads, and proof
  invariant findings for the reviewed source oracles.
- Next implementation should start only after choosing a fresh source oracle
  that satisfies one remaining bucket's entry criterion. Otherwise the correct
  action is verification, cleanup of stale notes, or handoff, not another
  same-family fixture or targetless-proof breadth slice.

## Do Not Reselect Without New Evidence

- Public callable parameters and public callable fields that lack complete
  source-visible callers.
- More same-family private callable fixture rows unless they introduce a new
  typed proof carrier.
- More awaited async closure storage variants that only exercise the existing
  same-block tracker.
- More returned async closure variants unless they introduce stored/forwarded
  future value flow or a new explicit blocker.
- More same-family `BindingSourceFunction` variants unless they introduce a
  new typed proof carrier or a real-corpus source oracle that cannot be
  explained by the existing initialized-path/projection edges.
- Axum `self.tap_fn` or router-side `self.into_route` as resolved edges unless
  the new implementation proves complete callable field value flow.
- External frontier rows as traversal edges unless a trusted external summary
  explicitly authorizes that boundary.

## Next Implementation Checklist

1. Name the selected bucket and source oracle.
2. State whether the expected result is a resolved edge, candidate-only row,
   external frontier, or explicit blocker.
3. Add or adjust the DB assertion first.
4. Extend parser/resolver/proof code only when the assertion names a missing
   typed proof input.
5. Propagate to RAG/TUI only if that row is exposed in those surfaces.
6. Update this inventory and the main coverage matrix once for the completed
   chunk.
