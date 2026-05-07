# Prototype 1 Edit Surface Handoff

Date: 2026-05-07

Task title: Handoff for surface-bound edit harness design

Task description: Restart note for continuing the design-to-code translation
from the Prototype 1 edit surface framework into `ploke-eval` trait adapters
and eventual `ploke-tui` integration.

Related planning files:

- `docs/active/agents/2026-05-07-prototype1-edit-surface-model.md`
- `docs/active/todo/2026-05-05_long-horizon.md`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-tui/src/tools/code_edit.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-db/src/helpers.rs`
- `crates/ploke-core/src/io_types.rs`

## Current State

The concise framework doc is now:

```text
docs/active/agents/2026-05-07-prototype1-edit-surface-model.md
```

It captures the current agreed model:

```text
Surface(operator, substrate, mode, policy)
```

and the key candidate-create path:

```text
Runtime<Parent<Ruling>>
  -> SurfaceGrant over Artifact
  -> EditProposal
  -> Patch / ArtifactDelta
  -> derived Artifact
  -> Child Runtime self-evaluation
```

The doc also records these invariants:

- `Runtime` is an executing environment, not a normal surface substrate.
- Every live `Runtime` inhabits exactly one `Role<State>`.
- Writable candidate-generation surface exists only for `Parent<Ruling>` or an
  executor acting inside a `Parent<Ruling>`-authorized create transition.
- `Patch = ArtifactDelta`.
- The code graph is a derived semantic view over an `Artifact`, materialized in
  the database, not an independent authority substrate.
- `ploke-tui` is both an edit harness and the current access path to the code
  graph, but `ploke-eval` should consume it through a trait adapter.

## Important Design Point

Do not model `ploke-tui` as authority.

`ploke-tui` can:

- search/query code graph projections;
- resolve semantic targets to material spans;
- stage edit proposals;
- possibly host internal agents/tools;
- provide patch-to-code-graph-delta projection;
- report harness/sub-executor provenance and uncertainty.

`ploke-eval` owns:

- `Parent<Ruling>` authority;
- `SurfaceGrant`;
- surface containment checks;
- Artifact application/identity validation;
- History admission;
- child hydration/evaluation;
- attribution and selection.

## Trait Adapter Sketch

The current proposed split is:

```rust
trait CodeGraphView { ... }
trait EditHarness { ... }
```

`CodeGraphView` should cover:

- `project(Artifact) -> Projection`
- `search(Projection, query, readable surface) -> SearchHit`
- `resolve(Projection, semantic target) -> MaterialSpan`
- `delta(before, after, Patch/ArtifactDelta) -> CodeGraphDelta`

`EditHarness` should cover:

- access to the graph adapter;
- `propose(EditInput) -> Proposal + HarnessRun`;
- approval/application boundary, still unresolved;
- harness/sub-executor provenance.

The unresolved method shape is:

```rust
fn approve(...)
fn apply_checked(...)
```

These may collapse into one method. The likely distinction is:

- `approve` is only needed when human or policy approval is an explicit
  procedure step.
- `apply_checked` is the boundary that consumes a `SurfaceCheck` produced by
  `ploke-eval` and realizes the approved/checked proposal.

If the first implementation does not need human approval, prefer a smaller
shape that avoids artificial ceremony, for example:

```rust
propose(...)
check in ploke-eval
apply_checked(...)
```

or:

```rust
propose(...)
authorize_and_apply(...)
```

only if the name and type make clear that the authorization comes from
`ploke-eval`, not the harness.

## Code Graph Framing

The code graph relation should stay explicit:

```text
Parse(Artifact) = CodeGraph
Store(CodeGraph) = DatabaseIndex
Resolve(DatabaseIndex, CanonicalTarget) = MaterialSpan
```

Canonical edit admissibility requires:

```text
Projects(code_graph, target_artifact, hashes)
Resolve(code_graph, canonical_target) = material_span
material_span.expected_hash matches target_artifact content
material_span is within Write(surface_grant)
```

This preserves:

```text
Artifact -> CodeGraph -> MaterialSpan -> EditProposal/Patch
```

without letting the DB or `ploke-tui` become authority.

## Scoring And Attribution

The design intentionally scores provisionally:

```text
child score
  -> derived Artifact score
  -> Patch / ArtifactDelta score
  -> parent-runtime-as-generator score
  -> affected component / harness / sub-executor evidence
```

The score is contextual:

```text
PatchScore(p | base_artifact, generator_runtime, surface_grant, eval_policy)
```

Later probabilistic/distributional attribution is conceptual background from:

- `docs/workflow/evalnomicon/chat-history/framework-ext-01.md`
- `docs/workflow/evalnomicon/chat-history/framework-ext-02.md`
- `docs/workflow/evalnomicon/chat-history/framework-ext-03.md`
- `docs/workflow/evalnomicon/chat-history/prototype-1-probability.md`

Those docs are relevant for stochastic-kernel and posterior-update framing, but
they are not current implementation authority.

## Suggested Next Step

Start with `ploke-eval`-owned types and tests, not `ploke-tui` internals.

Concrete next slice:

1. Add a small module for edit-surface/create adapter types in `ploke-eval`.
2. Define `CodeGraphView` and a mock implementation.
3. Define the minimal `EditHarness` method set after deciding whether
   `approve` and `apply_checked` are separate.
4. Add tests:
   - graph projection must be tied to Artifact identity/hash;
   - semantic target resolves to material span;
   - proposal outside writable surface is rejected;
   - material hash mismatch is rejected;
   - valid proposal yields `ArtifactDelta` evidence ready for History.
5. Only then add a concrete `ploke-tui` adapter.

## Live Run Context

A 15-generation Prototype 1 loop is currently running separately. Do not assume
its result. Before acting on loop runtime behavior after compaction, inspect the
campaign state and monitor output first.

