# 2026-07-01 Executable Local Owner Plan

Short description: design checkpoint for modeling calls inside closures, async
blocks, and function-local executable items without flattening those calls into
their enclosing function or method owners.

Related planning files:
- [`README.md`](README.md)
- [`2026-07-01_call-graph-larger-plan-map.md`](2026-07-01_call-graph-larger-plan-map.md)
- [`2026-07-01_call-graph-goal-coverage-matrix.md`](2026-07-01_call-graph-goal-coverage-matrix.md)
- [`../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`](../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md)

## Current Bucket

Root plan:
`.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`

Current phase: binding/type-aware semantic resolution.

Current bucket: executable-local body ownership.

Done-for-now threshold:

- Preserve the existing invariant that closure, async-block, and function-local
  item bodies are not flattened into the enclosing owner.
- Introduce nested executable owners only through typed parser facts, not by
  pretending they are `FunctionNodeId`, `ConstNodeId`, or `AnyNodeId` members.
- Project a real persisted owner row before DB/RAG/TUI APIs expose nested-owner
  calls as traversable context.
- Keep targetless unsupported rows visible until exact target proof exists.

## Existing Pattern Constraints

The existing call graph follows the typed endpoint-family style used by the
syntax and type graph layers:

- Call-site IDs live in the `CallId` universe through `PathCallSiteId`,
  `MethodCallSiteId`, `DynamicCallSiteId`, and `MacroCallSiteId`.
- `CallBodyOwnerId` currently contains only real code-graph nodes:
  `Function`, `Macro`, `Method`, `Const`, and `Static`.
- `BodyContainsCall` is a typed relation from `CallBodyOwnerId` to
  `AnyCallSiteId`.
- Transform projection flattens typed owner variants into `call_site_edge`
  rows only after the parser has proved the owner kind.
- DB owner and target queries validate owner IDs by joining them to concrete
  persisted node relations before returning context.

That means nested executable owners need their own typed owner proof and their
own persisted owner relation. Reusing the parent function/method ID would
flatten the graph. Reusing `AnyNodeId` would weaken the endpoint family.

## Current Evidence

Already covered:

- Parser extraction intentionally does not descend into `ExprClosure` or
  `ExprAsync`.
- Parser extraction intentionally skips function-local const initializers as
  ownerless for now.
- DB tests assert closure/async/local-const body calls do not leak into the
  enclosing owner.
- Real-corpus axum tests document closure-body and async-block rows as absent
  until nested owners exist.

Current gap:

- There is no stable nested executable owner identity.
- There is no relation that says a nested executable owner is contained by a
  parent function/method/macro/const/static owner.
- There is no DB metadata surface for source file, span, parent owner, or owner
  kind for nested executable bodies.

## Candidate Model

Add a new parser-side owner identity family for executable local bodies:

```text
ExecutableBodyId =
    ClosureBodyId
  ∪ AsyncBlockBodyId
  ∪ LocalItemBodyId

CallBodyOwnerId =
    FunctionNodeId
  ∪ MacroNodeId
  ∪ MethodNodeId
  ∪ ConstNodeId
  ∪ StaticNodeId
  ∪ ExecutableBodyId
```

`ExecutableBodyId` should not be added to `AnyNodeId`. It is an executable
scope owner, not a code definition node.

The parser should also emit a typed containment relation:

```text
ExecutableBodyOwner ⊆ CallBodyOwnerId(parent) × ExecutableBodyId(child)
```

The persisted DB layer should then receive a relation such as
`call_body_owner` with enough metadata to validate and display nested owners:

- `id`
- `owner_kind`
- `parent_id`
- `parent_kind`
- `span`
- `cfgs`
- optional structural label, such as `closure`, `async_block`, or `local_item`

The exact relation name can follow the transform schema convention once the
schema file is edited. The important part is that DB call-context queries
should validate nested owner rows through this persisted relation instead of
assuming every owner is a function/method/const/static/macro node.

## First Implementation Slice

Recommended first code slice:

1. Add `ClosureBodyId` and `ExecutableBodyId` typed wrappers using the same
   call-site/type-family style, backed by a new stable base identity if needed.
2. Add a parser-side closure owner record with parent owner, span, cfgs, and
   body call extraction.
3. Add a fixture-backed closure body:
   `let closure = || local_target(); closure()`.
4. Assert parser facts:
   - the outer function owns the dynamic `closure()` call;
   - the closure owner owns the inner `local_target()` path call;
   - no outer `local_target()` row is fabricated.
5. Project the closure owner relation and DB rows.
6. Assert DB facts:
   - `call_context_for_owner(outer)` still excludes the inner path call;
   - `call_context_for_owner(closure_owner)` includes the inner path call;
   - target-centered callers for `local_target` include the closure owner only
     when the DB owner metadata can describe that owner.

Do not add RAG/TUI coverage until the DB context row has a stable nested-owner
metadata contract.

## Explicit Non-Goals

- Do not resolve closure values or `Fn`/`FnMut`/`FnOnce` dispatch in this slice.
- Do not infer that `closure()` calls the closure body unless binding proof and
  invocation semantics are explicitly modeled.
- Do not model arbitrary async runtime scheduling or future polling edges.
- Do not loosen backup fixture import behavior to accommodate the new relation.
- Do not regenerate real-corpus backup fixtures until schema and fixture review
  are intentionally scheduled.

## Risks

- Adding a new owner universe may affect pruning, transform projection, DB owner
  joins, proof projection, RAG collection, and TUI payloads.
- Existing usage summaries assume call graph nodes have `CallNodeInfo` from
  function/method/macro/const/static metadata. Nested owners need a separate
  metadata path before they can participate in impact/reach summaries.
- If nested owners are inserted into traversal before metadata exists, usage
  queries will either fail or silently lose source context.

## Stop Conditions

Stop and ask before implementation if:

- the selected base identity would require changing `ploke_core` public ID
  semantics;
- the DB schema change requires regenerating registered backup fixtures;
- a proposed shortcut would flatten nested calls into their parent owner;
- a proposed shortcut would allow `AnyNodeId` or raw `Uuid` owner endpoints in
  parser-side relations.
