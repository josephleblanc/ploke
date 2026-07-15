# 2026-07-01 Call Graph Larger Plan Map

Short description: bridge between the original typed call-graph plan, the active restart spine, and the goal coverage matrix used for current implementation pacing.

Related planning files:
- [`../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`](../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md)
- [`../2026-06-21_call-graph-fixture-nodes-orchestration-plan.md`](../2026-06-21_call-graph-fixture-nodes-orchestration-plan.md)
- [`README.md`](README.md)
- [`2026-07-01_call-graph-goal-coverage-matrix.md`](2026-07-01_call-graph-goal-coverage-matrix.md)
- [`2026-06-25_call-graph-coverage-inventory.md`](2026-06-25_call-graph-coverage-inventory.md)
- [`../2026-06-30_call-graph-usage-questions.md`](../2026-06-30_call-graph-usage-questions.md)

Status date: 2026-07-01
Baseline HEAD when created: `a14380a73`

## Root Plan

The document that began the broader call-graph implementation is:

```text
.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md
```

That plan set the main design direction: add a Ploke-native call graph that preserves the existing typed endpoint-family style used by syntactic relations, type relations, and the typed type graph. Its most important constraint is that parser-side call graph facts must be strongly typed, while DB/RAG/tool layers may only flatten them after typed parser facts exist.

The first concrete implementation entry point was:

```text
docs/active/agents/2026-06-21_call-graph-fixture-nodes-orchestration-plan.md
```

That orchestration plan narrowed the root plan into the first accepted slice: structural call-site extraction for `fixture_nodes`, exact `self.private_method()` resolution, and proof projection only after parser-side typed facts existed.

## Larger Rollout Shape

The larger implementation has been progressing through these phases:

| Phase | Purpose | Current status |
| --- | --- | --- |
| Typed parser identity/modeling | Add `CallId`, typed call-site IDs, owner families, relation/status carriers | Implemented and expanded |
| Structural extraction | Record path, method, dynamic, macro, const/static, and associated-const callsites | Implemented for current matrix rows |
| Conservative semantic resolution | Add exact local edges without weakening unknown/ambiguous cases | Broad partial implementation |
| Transform/DB projection | Persist callsite, relation, status, context, and proof rows | Implemented |
| DB query helpers | Answer caller/callee/path/impact/reach questions over persisted graph | Strong current surface |
| Real-corpus proof | Prove query behavior against real Rust targets, especially axum | Active and partially covered |
| RAG/TUI/tool surfaces | Expose exact call context, paths, impact, reach, and proof blockers downstream | Strong current surface |
| Proof/authority integration | Explain trusted edges and fail-closed blockers | Partial but substantial |
| Semantic expansion | Add binding/type-aware capabilities for remaining gaps | Active with exact local external-trait impl receiver methods, fixture-backed borrowed initialized local receivers, borrowed value-parameter receivers and method-result chains, private callable-parameter proof, selected external/frontier promotions such as impl Trait `Into` and generic-bound `Default`, and fail-closed blocker proof for unproven dynamic callable rows |

## Where The Coverage Matrix Fits

[`2026-07-01_call-graph-goal-coverage-matrix.md`](2026-07-01_call-graph-goal-coverage-matrix.md) is not the root plan. It is an execution guardrail inside the real-corpus proof and downstream-query phases.

Its job is to prevent local over-focus. A bucket is done for now once it has representative DB/RAG/TUI proof or a documented fail-closed blocker. After that, implementation should switch to the next uncovered bucket instead of continuing to polish the same call shape.

Current matrix posture:

- Method/trait-method multi-hop: met for now.
- Regular free-function one-hop: covered.
- Regular free-function multi-hop: met for now.
- Import/type-alias constructor completeness: met for the
  chrono `MappedLocalTime::Single -> LocalResult::Single` alias-constructor
  path across parser, DB, RAG, and TUI.
- Exact local external-trait impl receiver methods: met for axum
  `Router::clone` rows and axum-core
  `parts.extract_with_state(state)` imported external receiver proof across DB,
  RAG, and TUI.
- Associated-constructor initialized local receiver methods: real-corpus axum
  `CountingCloneableState::new() -> state.clone()/state.setup_done()` now uses
  the local constructor's `Self` return type as receiver proof and resolves six
  routing-test method rows through DB traversal.
- Borrowed initialized local receiver methods: fixture-backed
  `let value = LocalAssoc; (&value).instance_value()` now carries initializer
  proof through parser, DB/proof projection, RAG, and TUI formatting.
- Borrowed value-parameter receiver methods: fixture-backed
  `call_borrowed_value_param_instance_method(value: LocalAssoc)` now resolves
  `(&value).instance_value()` through the same exact parameter receiver proof
  used by direct parameter method calls across parser, DB/proof projection, and
  RAG call-context expansion.
- Borrowed value-parameter method-result chains: fixture-backed
  `call_borrowed_value_param_method_result_instance_method(value: LocalAssoc)`
  now resolves `(&value).clone_assoc().instance_value()` by reusing exact
  borrowed parameter proof for the inner method and return-type proof for the
  outer method across parser, DB/proof projection, and RAG call-context
  expansion.
- Dynamic callable bindings: exact local function-item bindings now cover
  direct, alias, branch/match including guarded same-target match arms, and
  single-expression block initializers through parser, DB, RAG, and TUI proof
  where exposed.
- External generic-bound associated frontiers: fixture-backed
  `T::default()` rows with source-visible external/prelude `Default` bounds now
  classify as targetless external frontiers across parser, DB, RAG, and TUI
  proof surfaces without claiming concrete trait dispatch.
- Real-corpus dynamic callable fallback proof: memchr function-pointer field
  rows now preserve finite ambiguous `DynamicFunction` candidates, while boxed
  callable-field rows remain fail-closed and targetless with DB proof rows
  preserving blocker reasons and source provenance.
- Tool usage summaries: `code_item_edges` now propagates the existing
  impact/reach summaries in `node_info`, matching `code_item_lookup` instead
  of exposing only lower-level path carriers.
- Unsupported receiver visibility: method calls with receiver expressions
  outside the conservative classifier now persist as targetless
  `Unsupported` receiver rows instead of being dropped before status/proof
  projection.
- Qualified trait-object qself path projection: fixture-backed and regenerated
  axum `<dyn std::any::Any>::downcast_mut::<T>(...)` calls now project as
  path-call rows with generic argument counts and remain targetless `External`
  frontier rows through DB/RAG/TUI proof surfaces.
- Constructor semantic expansion: fixture-backed method-owned `Self(value)`
  tuple-struct constructor calls and regenerated axum `BoxedIntoRoute`
  `Self(...)` constructor rows now resolve through the enclosing impl self type
  and project through DB/proof/RAG/TUI constructor surfaces.
- Executable-local item boundaries: function-local const/static initializer
  calls and local `fn` body calls are owned by executable `LocalItem` owners
  and are no longer flattened into the enclosing function owner; block-local
  `fn` item calls such as `inner()` now resolve as `LocalFunction -> LocalItem`
  edges from the enclosing function to the executable local-item owner,
  including call-before-local-item-declaration source order.
- Dynamic/receiver/closure/import gaps: future semantic expansion buckets, not reasons to keep polishing already-proven method paths.
- Workspace dependency-root imports: selected workspace fixtures can contain
  one crate importing another selected crate, for example axum-core test code
  importing `axum::{test_helpers::*, Router}`. The current resolver now has
  bounded workspace proof for selected type/trait imports such as axum-core
  `TestClient::new`, `Router::new`, axum `FromRef::from_ref`, and axum
  re-exported `Body::empty` rows, including nested closure/local-item owners
  and a nested local impl where-bound owner. The proof graph now has strict
  `dependency_root` carriers for the real axum `FromRef::from_ref`
  dependency-root rows, the axum-core `request_parts.rs:193` `Router::new`
  workspace-import row, the axum-core `TestClient::new` workspace-glob row,
  and the direct axum `form.rs:158` `Body::empty` workspace import row, so
  downstream context can explain those target admissions. Broader
  dependency-root imports still need their own exact source oracle and proof
  carrier instances before they can be promoted; the per-crate `ModuleTree`
  alone is not enough authority.

## Phase Transition Rule

Once the coverage matrix is complete enough that every current representative bucket has either a positive proof or a fail-closed proof, stop adding breadth to the coverage phase and switch to semantic expansion.

The current larger implementation phase is:

```text
binding/type-aware semantic resolution, with the persistent local-binding
proof carrier now established for exact source oracles
```

This phase should connect existing syntax-body ownership, local binding evidence, and typed type graph facts so the resolver can prove more receiver and callable-value cases without weakening fail-closed semantics. Completed slices include exact local external-trait impl receiver methods such as axum `Router::clone`, axum-core `parts.extract_with_state(state)` where `parts: &mut http::request::Parts`, fixture-backed borrowed initialized local receivers such as `(&value).instance_value()` where `value` is initialized from a local type path, borrowed value-parameter receivers where `(&value).instance_value()` reuses exact parameter receiver proof, borrowed value-parameter method-result chains where `(&value).clone_assoc().instance_value()` reuses exact parameter proof plus return-type proof, and private complete-single-caller callable parameter proofs including direct array parameters such as `call_single_indexed_function_pointer_param(funcs: [fn() -> i32; 1]) { funcs[0]() }`. Broader dispatch remains out of scope until the required binding/type evidence is explicit.

2026-07-15 checkpoint:

- The local-binding proof carrier has moved from planned to implemented for
  the current exact source-oracle set. Parser, transform, DB, exact RAG, and
  exact TUI/tool payloads now preserve typed `local_binding` and
  `local_binding_edge` evidence for returned-callable bindings, returned future
  proof rows, same-block future let/aggregate storage, parameter bindings,
  argument-to-parameter edges, initialized callable path bindings, value
  aliases, constructed field/index projections, field-projection source
  function edges, and exact private method callable arguments.
- The implemented carrier remains explanatory proof over already-admitted
  traversal. It deliberately does not infer public API caller values, arbitrary
  aggregate value flow, broad callable-field construction flow, trait-object
  vtable dispatch, or general async poll/resume traversal.
- Active call-graph fixtures were regenerated and verified after the latest
  carrier slices. The current restart rule is to select the next larger proof
  model only when a reviewed source oracle can name the missing typed input and
  a DB-first assertion can prove either a new exact edge or a stricter
  fail-closed blocker.

## Next Larger Phase

The original recommended first semantic-expansion bucket was:

```text
local binding tracking for callable values and typed receivers
```

That bucket is now the foundation for the current implementation, not the next
unstarted phase.

Why this bucket first:

- It supports regular function-item flow, such as `let f = local_target; f()`.
- It supports method receiver flow, such as `let value: LocalType = ...; value.method()`.
- It creates the foundation for field receivers, closure ownership, function pointers, `dyn Fn`, and more precise dynamic-call resolution.
- It can use already-existing source information: function/method bodies, syntax graph ownership, imports, and type graph edges.

Do not start with broad trait dispatch or arbitrary dynamic callable execution. Those should follow after the binding/type evidence model is explicit and tested.

Remaining larger proof models:

| Bucket | Entry criterion | Current boundary |
| --- | --- | --- |
| Broader object/field callable value flow | A source-visible construction path proves every callable value reaching a field, or a typed value-flow carrier records the incomplete proof. | Axum `self.layer`, router-side `self.into_route`, and `self.tap_fn` stay ambiguous or targetless; existing runtime-dispatch summaries do not create local edges. |
| Callable trait-object dispatch | A bounded local source oracle proves a concrete callable target without public caller ambiguity or vtable guessing. | Exact initializer and private complete-caller `dyn Fn` cases are covered; unbounded trait-object dispatch stays fail-closed. |
| Async future value flow / poll-resume | A typed future-flow carrier identifies both producer future and poll point without flattening async state-machine execution into ordinary source calls. | Returned-future proof/execution rows explain reviewed cases; ordinary traversal to the async closure body remains blocked unless polling is source-visible and bounded. |
| Generated or macro-expanded source bodies | A specific macro template and invocation pair can be modeled narrowly through normal item/call visitors. | Bounded axum generated-item templates are covered; arbitrary macro expansion stays out of scope and projects blockers. |
| External/source-sink policy summaries | A concrete usage question needs a new closed-vocabulary effect/summary fact that current `effect_seed`, `effect_policy`, guard, boundary, or summary helpers cannot express. | Existing effect, guard, boundary, build/test, and external-summary surfaces are strong; do not add policy vocabulary speculatively. |

## Practical Next Steps After Matrix Completion

1. Select exactly one remaining larger proof model and source oracle from the
   table above.
2. Inventory existing data already available for that oracle:
   - owner body text and callsite spans;
   - local binding syntax;
   - existing type graph edges and alias facts;
   - import/re-export bindings;
   - existing callsite relation/status storage.
3. Add the DB assertion first, naming whether the expected result is an exact
   traversal edge, candidate-only row, external frontier, or explicit blocker.
4. Add parser/transform/proof carriers only when that assertion names missing
   typed input; otherwise keep unsupported rows explicit.
5. Propagate to RAG/TUI only after the DB proof surface is stable.

## Workflow Guardrail

Before resuming implementation after this plan map, the agent should state:

```text
Root plan:
Current phase:
Current bucket:
Exit criteria:
Next phase if this bucket is done:
```

For the current state, that should be:

```text
Root plan: .hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md
Current phase: binding/type-aware semantic resolution
Current completed checkpoint: persistent local-binding proof carrier slices now cover returned-callable evidence, returned-future proof rows, exact let/parameter/alias/field-projection rows, argument-to-parameter edges, method callable arguments, and exact DB/RAG/TUI payload propagation; active call-graph fixtures have been regenerated and verified
Completed proof: the current representative buckets are either exact local edges with typed proof or explicit fail-closed frontier rows with blocker/proof payloads; no remaining audited real-corpus candidate is safe to promote without broader macro/generated-body, interprocedural value-flow, callable-field construction, trait-object dispatch, external-summary/source-sink, or async poll/resume evidence
Next phase if this bucket is done: choose one remaining larger proof model with a source oracle and DB-first assertion; do not add more same-family binding, import, receiver, or targetless breadth unless the source oracle introduces a new typed proof input
```
