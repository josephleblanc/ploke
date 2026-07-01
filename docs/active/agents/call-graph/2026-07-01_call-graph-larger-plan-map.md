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
| Semantic expansion | Add binding/type-aware capabilities for remaining gaps | Started with exact local external-trait impl receiver methods and fixture-backed borrowed initialized local receivers |

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
  `Router::clone` rows across DB, RAG, and TUI.
- Borrowed initialized local receiver methods: fixture-backed
  `let value = LocalAssoc; (&value).instance_value()` now carries initializer
  proof through parser, DB/proof projection, RAG, and TUI formatting.
- Dynamic callable bindings: exact local function-item bindings now cover
  direct, alias, branch/match, and single-expression block initializers through
  parser, DB, RAG, and TUI proof where exposed.
- Unsupported receiver visibility: method calls with receiver expressions
  outside the conservative classifier now persist as targetless
  `Unsupported` receiver rows instead of being dropped before status/proof
  projection.
- Executable-local const initializer boundaries: function-local const
  initializer calls are no longer flattened into the enclosing function owner;
  true local const owner rows remain blocked on an executable-scope identity
  model.
- Dynamic/receiver/closure/import gaps: future semantic expansion buckets, not reasons to keep polishing already-proven method paths.

## Phase Transition Rule

Once the coverage matrix is complete enough that every current representative bucket has either a positive proof or a fail-closed proof, stop adding breadth to the coverage phase and switch to semantic expansion.

The current larger implementation phase is:

```text
binding/type-aware semantic resolution
```

This phase should connect existing syntax-body ownership, local binding evidence, and typed type graph facts so the resolver can prove more receiver and callable-value cases without weakening fail-closed semantics. Completed slices include exact local external-trait impl receiver methods such as axum `Router::clone` and fixture-backed borrowed initialized local receivers such as `(&value).instance_value()` where `value` is initialized from a local type path. Broader dispatch remains out of scope until the required binding/type evidence is explicit.

## Next Larger Phase

Recommended first semantic-expansion bucket:

```text
local binding tracking for callable values and typed receivers
```

Why this bucket first:

- It supports regular function-item flow, such as `let f = local_target; f()`.
- It supports method receiver flow, such as `let value: LocalType = ...; value.method()`.
- It creates the foundation for field receivers, closure ownership, function pointers, `dyn Fn`, and more precise dynamic-call resolution.
- It can use already-existing source information: function/method bodies, syntax graph ownership, imports, and type graph edges.

Do not start with broad trait dispatch or arbitrary dynamic callable execution. Those should follow after the binding/type evidence model is explicit and tested.

## Practical Next Steps After Matrix Completion

1. Create a binding/type-aware resolver plan that follows the same nearby patterns as typed type graph and existing call-resolution code.
2. Inventory existing data already available for this plan:
   - owner body text and callsite spans;
   - local binding syntax;
   - existing type graph edges and alias facts;
   - import/re-export bindings;
   - existing callsite relation/status storage.
3. Define conservative proof carriers for local binding evidence.
4. Add DB/RAG/TUI tests only after parser/transform facts exist.
5. Keep unsupported rows explicit until exact proof exists.

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
Current completed bucket: executable-local const initializer boundary
Completed proof: fixture-backed local const initializer calls are not attributed to the enclosing function owner in parser extraction or DB owner context; top-level/associated initializer owners remain covered separately
Next phase if this bucket is done: return to the coverage matrix and select the next real-corpus DB query or downstream usage-query gap
```
