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
- The latest proof-authority slice keeps memchr
  `memchr/src/tests/substring/mod.rs:94,110` boxed `dyn FnMut` path rows
  targetless, but admits runtime-dispatch summaries that discharge their
  owner-scoped authoring needs without fabricating call edges.
- The latest async-frontier proof-authority slice keeps axum
  `error_handling/mod.rs:251` boxed dyn `Future::poll` unsupported and
  targetless, but admits a runtime-dispatch summary that discharges its
  owner-scoped authoring need without modeling poll/resume traversal.
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

Local-binding projection checkpoint, 2026-07-14:

- Source oracles:
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
- Fixtures: active call-graph fixtures regenerated and verified; shared corpus
  snapshots now report 66 relations.
- Boundary: this is not general let-binding flow, callable-field value flow,
  trait-object dispatch, or async poll/resume traversal.

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
| Broader object/field callable value flow | Axum `self.layer`, `self.tap_fn`, and router-side `self.into_route` remain targetless or candidate-only because construction/value flow is not complete. Runtime-dispatch summaries can discharge authoring needs without fabricating edges. | A source-visible construction path proves every callable value reaching a field, or the implementation introduces a typed value-flow carrier with strict incomplete-proof blockers. | DB real-corpus assertion over one field callsite, preserving no edge when proof is incomplete. |
| Callable trait-object dispatch | Fixture-backed exact `&dyn Fn`, `Box<dyn Fn>`, and `FnMut` local-binding cases are covered only when initializer or complete private-caller proof is exact. Memchr boxed `dyn FnMut` field rows remain targetless blockers; admitted runtime-dispatch summaries discharge their proof-authoring needs without creating traversal edges. | A bounded local source oracle proves a callable trait-object target without public API ambiguity or runtime vtable guessing. | Parser/DB proof for one exact trait-object row; otherwise keep `dynamic_dispatch_unbounded` or an admitted summary when the row is intentionally targetless. |
| Async poll/resume and future value flow | Immediate/same-block async closure calls, aliases, tuple/named/indexed storage, immediate awaited returned async closures, and same-block local bindings of returned async-closure futures are covered. Un-awaited returned async closures fail closed. Axum boxed dyn `Future::poll` has blocker and admitted runtime-dispatch summary coverage while remaining targetless. Non-local flow, async callable trait objects, returned futures that are forwarded across functions or stored in aggregates before polling, and general poll/resume remain future work. | A typed future-flow carrier identifies the future producer and poll point without flattening async state-machine execution into ordinary source calls. | One DB traversal or one explicit blocker/summary over a reviewed source oracle. |
| Generated or macro-expanded source bodies | Bounded axum and fixture macro models cover reviewed item/local-item/generated-method cases, including the axum middleware `from_fn`, `map_request`, and `map_response` `std::mem::replace` frontiers. Axum `assert_eq!`-wrapped `try_downcast` rows now have DB proof blockers for unsupported macro expansion. Arbitrary macro expansion remains out of scope. | A specific macro template and invocation pair can be modeled narrowly through normal item/call visitors, with proof metadata explaining the boundary. | DB real-corpus traversal for one generated owner or a targetless proof row for unsupported expansion. |
| Workspace dependency-root/import families | Existing dependency-root proof rows cover the named axum `FromRef`, `Router::new`, `TestClient::new`, and direct `Body::empty` import families. | A new workspace-import source oracle has a resolved edge that needs an explicit proof-authority row; do not add carrier rows just to increase counts. | Target-centered DB/RAG/TUI proof row tied to exact callsite and import path. |
| External summary/source-sink policy | External frontiers, effect seeds, effect policies, runtime summaries, and proof blockers exist for current usage questions. | A concrete usage question requires a new summary or source/sink fact that cannot be answered from current frontier/effect data. | Proof-store validation plus one DB/RAG/TUI query that consumes the new fact. |
| Presentation/prompt policy | RAG prompts and context-plan overlays expose path, proof, and blocker policy for current tools. The shared matrix now distinguishes bounded prompt collection from exact lookup coverage, and the chrono `queue.is_empty` long-owner row has focused exact TUI coverage while bounded RAG remains intentionally capped. | A user-facing tool payload lacks a field already available in DB/RAG, or current copy risks treating blockers as edges. | Exact TUI test over existing DB/RAG data; no parser change. |

## Do Not Reselect Without New Evidence

- Public callable parameters and public callable fields that lack complete
  source-visible callers.
- More same-family private callable fixture rows unless they introduce a new
  typed proof carrier.
- More awaited async closure storage variants that only exercise the existing
  same-block tracker.
- More returned async closure variants unless they introduce stored/forwarded
  future value flow or a new explicit blocker.
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
