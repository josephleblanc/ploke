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
