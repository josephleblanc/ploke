# Prototype 1 Edit Surface Implementation Plan

Date: 2026-05-07

Task title: Wire bounded `ploke-tui` edit surfaces into Prototype 1 candidate creation

Task description: Extend the Prototype 1 edit-surface model into a maintainable
implementation plan with validation gates, review gates, and sub-agent work
slices. The end state is a long-running Prototype 1 loop that can generate child
candidates by editing a bounded `ploke-tui` tool surface, evaluate those
children concurrently, persist the edit/surface evidence into History, and use
History-backed traversal selection.

Related planning files:

- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-model.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-handoff.md`
- `docs/active/agents/2026-05-06-prototype1-execution-surface-model.md`
- `docs/active/agents/2026-05-06_history-traversal-invariant-review.md`
- `docs/active/todo/2026-05-05_long-horizon.md`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-tui/src/tools/code_edit.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-db/src/helpers.rs`

## Target

The target loop shape is:

```text
Runtime<Parent<Ruling>>
  -> SurfaceGrant over Artifact
  -> CodeGraphView bounded by admitted rule/grant
  -> EditHarness proposal
  -> SurfaceCheck
  -> ArtifactDelta
  -> derived child Artifact
  -> concurrent Child Runtime self-evaluation
  -> History-backed traversal selection
  -> successor handoff
```

The implementation must make it possible to run:

```text
loop prototype1-state
  with candidate generation = bounded tui edit surface
  with surface = ploke-tui tool area
  with successor selection = history-score-child-prop
  with scoring metrics = operational-and-protocol
```

and have the generated data be useful after the run.

## Non-Negotiable Invariants

- `ploke-eval` owns authority. It grants surfaces, validates containment,
  applies or admits checked transitions, writes History, hydrates children, and
  selects successors.
- `ploke-tui` is an edit/code-intelligence harness. It may stage proposals,
  resolve canonical targets, apply checked proposals as an executor, and report
  harness evidence. It is not authority.
- `ploke-db` / CozoScript rules can define or derive bounded graph areas, but a
  rule is not authority by itself. Authority is the `SurfaceGrant` admitted or
  carried by `ploke-eval`.
- The code graph is a derived semantic view over an `Artifact`. A graph
  projection must be tied to the Artifact/hash it projects.
- Writable candidate-generation surface belongs to `Parent<Ruling>` or an
  executor inside a `Parent<Ruling>` authorized create transition.
- A Child can read/evaluate an Artifact and write result-channel evidence, but
  cannot hold the candidate-generation writable Artifact surface.
- `Patch = ArtifactDelta`. Patch scoring is contextual and provisional.
- Proposal storage, rendered previews, logs, and operator UI state are
  projections/telemetry unless explicitly admitted into History as evidence.
- Do not model this with flattened long names. Use modules, traits, associated
  types, state carriers, and small records.

## Architecture

Add a narrow `ploke-eval` boundary module:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/
  mod.rs
  graph.rs
  surface.rs
  harness.rs
  tests.rs
```

Local names should be short because the module carries the domain context:

```text
graph::View
graph::Projection
graph::Rule
graph::Bounds
graph::Target
graph::Span

surface::Grant
surface::Check
surface::Policy
surface::Area
surface::Touch

harness::Harness
harness::Input
harness::Proposal
harness::Run
harness::Applied
```

Exact names may change if the codebase already has a stronger local carrier,
but avoid new `Prototype1TuiCodeGraphSurfaceSomething` names.

## Phase 1: Authority-Side Boundary And Mocks

Implement the `ploke-eval` trait boundary without importing live `ploke-tui`
internals into the loop path.

Expected code:

```rust
trait View {
    type Artifact;
    type Projection;
    type Rule;
    type Bounds;
    type Query;
    type Target;
    type Hit;
    type Span;
    type Delta;
    type Error;

    fn project(&self, artifact: &Self::Artifact) -> Result<Self::Projection, Self::Error>;
    fn bounds(
        &self,
        projection: &Self::Projection,
        rules: &[Self::Rule],
    ) -> Result<Self::Bounds, Self::Error>;
    fn search(
        &self,
        projection: &Self::Projection,
        bounds: &Self::Bounds,
        query: &Self::Query,
    ) -> Result<Vec<Self::Hit>, Self::Error>;
    fn resolve(
        &self,
        projection: &Self::Projection,
        bounds: &Self::Bounds,
        target: &Self::Target,
    ) -> Result<Self::Span, Self::Error>;
    fn delta(
        &self,
        before: &Self::Projection,
        after: &Self::Projection,
        patch: &ArtifactDelta,
    ) -> Result<Self::Delta, Self::Error>;
}
```

Use associated types so later adapters can wrap `ploke-db` and `ploke-tui`
without forcing authority-side types to depend on their storage types.

For the harness:

```rust
trait Harness {
    type Graph: graph::View;
    type Proposal;
    type Applied;
    type Run;
    type Error;

    fn graph(&self) -> &Self::Graph;
    fn propose(&self, input: Input<'_>) -> Result<(Self::Proposal, Self::Run), Self::Error>;
    fn apply_checked(
        &self,
        proposal: Self::Proposal,
        check: surface::Check,
    ) -> Result<Self::Applied, Self::Error>;
}
```

Do not add a separate `approve` method in this phase. Approval can be added
later only if it becomes a real procedure step. For now, authorization is the
`ploke-eval` check.

Validation gate:

```bash
cargo fmt --all
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo test -p ploke-eval edit_surface --lib 2>&1 | tail -n 20
```

Required tests:

- graph projection is tied to one Artifact id/hash;
- rule-derived bounds include one target and exclude another;
- resolving a target outside bounds fails;
- material span outside writable surface fails;
- expected file hash mismatch fails;
- valid proposal produces an `ArtifactDelta`-shaped result;
- parent/ancestor graph rules can narrow but not widen the granted surface.

Review gate:

- Spawn a reviewer to check authority semantics, naming, and whether public
  constructors/fields weaken the intended boundary.
- Reviewer output goes under `docs/active/agents/`.

## Phase 2: Concrete `ploke-tui` / `ploke-db` Adapter

Implement the first concrete adapter without changing loop behavior yet.

Current anchors:

- `crates/ploke-tui/src/tools/code_edit.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-db/src/helpers.rs::graph_resolve_exact`

The adapter should provide:

- project target Artifact into a graph/database projection;
- evaluate rule-derived bounds;
- search within bounds;
- resolve canonical edit targets to material spans;
- stage an edit proposal through existing TUI edit machinery where possible;
- apply only after receiving a `surface::Check`;
- return touched spans, before/after hashes, proposal id, harness run id, and
  applied delta evidence.

The first bounded area should target the `ploke-tui` tool/edit surface:

```text
crates/ploke-tui/src/tools/**
crates/ploke-tui/src/rag/tools.rs
crates/ploke-tui/src/rag/editing.rs
```

The exact rule representation should be durable enough to record:

```text
rule id or digest
rule source or named rule ref
projection artifact id/hash
derived bounds digest
```

Validation gate:

```bash
cargo fmt --all
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo test -p ploke-eval edit_surface --lib 2>&1 | tail -n 20
```

Required tests:

- adapter rejects stale graph projection for a changed Artifact;
- adapter resolves a known canonical target inside the `ploke-tui` tool bounds;
- adapter rejects a canonical target outside the rule-derived bounds;
- checked application refuses a proposal not matching the granted surface;
- checked application returns enough evidence for History admission.

Review gate:

- Spawn a reviewer to check that adapter code did not import TUI proposal state
  as authority and did not let CozoScript rules bypass `SurfaceGrant`.
- Reviewer output goes under `docs/active/agents/`.

## Phase 3: Candidate Generation Integration

Wire bounded edit-surface candidate generation into the parent turn before the
child plan is finalized.

Current loop shape:

```text
Parent turn
  -> resolve_child_plan(...)
  -> materialize child Artifact
  -> build child binary
  -> spawn child runtimes concurrently
  -> collect child outcomes
  -> History-backed selection
  -> successor handoff
```

Target shape:

```text
Parent<Ruling>
  -> choose/edit bounded surface
  -> generate N checked ArtifactDeltas through EditHarness
  -> commit/materialize N child Artifacts
  -> publish ChildPlan
  -> existing concurrent child fanout
  -> collect child outcomes
  -> History-backed selection
  -> successor handoff
```

The existing concurrent child evaluation path should remain intact. The child
runner still uses invocation/channel payloads for active execution. The change
is candidate creation.

Add explicit CLI/config selection:

```text
--candidate-generator tui-edit-surface
--edit-surface ploke-tui-tools
```

Use existing or near-existing knobs for:

```text
--successor-selection history-score-child-prop
--successor-selection-metrics operational-and-protocol
```

Do not read scheduler/projection files to recover authority. If a runtime value
is needed, thread it through parent identity, bootstrap, handoff, channel, or
History-owned records.

Validation gate:

```bash
cargo fmt --all
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo test -p ploke-eval prototype1_state --lib 2>&1 | tail -n 20
```

Required tests:

- parent candidate creation produces a `ChildPlan` from checked edit-surface
  candidates;
- generated child Artifacts carry surface check and delta evidence;
- an out-of-surface proposed edit fails before materialization/build;
- a stale projection/hash mismatch fails before materialization/build;
- child fanout remains concurrent and still reads invocation/channel payloads;
- History-backed selection can select a candidate generated by this path.

Review gate:

- Spawn a reviewer focused on live-loop correctness: parent/child authority,
  channel boundaries, History handoff, and failure-before-build behavior.
- Reviewer output goes under `docs/active/agents/`.

## Phase 4: History Evidence And Replay

Persist enough edit-surface data for later scoring, replay, and attribution.

Each generated candidate should carry:

```text
generator runtime id
target artifact id
surface policy id
graph rule ids/digests
projection artifact id/hash
graph bounds digest
proposal id / harness run id
canonical targets
material spans
before hashes
after hashes
ArtifactDelta identity
SurfaceCheck result
derived artifact id
child runtime id later
child evaluation later
```

This should become History candidate/evidence material or content-addressed
evidence referenced from History. Do not hide it in mutable reports only.

Validation gate:

```bash
cargo fmt --all
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo test -p ploke-eval successor_selection --lib 2>&1 | tail -n 20
cargo test -p ploke-eval prototype1_state --lib 2>&1 | tail -n 20
```

Required tests:

- current-generation edit-surface candidates enter traversal candidate view;
- selected candidate replay includes surface/proposal/delta evidence refs;
- missing edit-surface evidence is a projection failure or fail-closed handoff,
  not a silent default;
- `selection-show --replay` can expose enough evidence to diagnose the choice.

Review gate:

- Spawn a reviewer focused on History/traversal replay and scoring data
  persistence.
- Reviewer output goes under `docs/active/agents/`.

## Phase 5: Live Run Readiness

Before a long run, perform a short run using the new path.

Short-run readiness:

```text
3 generations
candidate-generator = tui-edit-surface
edit-surface = ploke-tui-tools
successor-selection = history-score-child-prop
successor-selection-metrics = operational-and-protocol
```

Checks after the short run:

- loop reaches successor handoff;
- child fanout runs concurrently;
- selected successor is hydrated from checked Artifact evidence;
- History contains candidate, surface, proposal/delta, child evaluation, and
  selection evidence;
- replay command can show the considered set and selected candidate;
- no active path reads scheduler/projection files for authority.

Only then run the longer loop:

```text
15+ generations, then 30 generations when stable
```

The live run must be started from the active parent worktree with that
worktree's `./target/debug/ploke-eval`. Codex may set up and observe the run
but should not advance the live `prototype1-state` loop without explicit user
action.

## Orchestration Plan

The main thread should stay light. Sub-agents should read this file plus:

```text
docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-model.md
docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-handoff.md
```

before editing.

Initial sub-agent split:

1. Explorer: locate exact current candidate creation/materialization splice
   points and report minimal line ranges.
2. Worker: implement Phase 1 authority-side module and tests.
3. Explorer: locate the concrete `ploke-tui` / `ploke-db` edit and graph
   adapter surfaces and report minimal integration paths.

After Phase 1:

1. Reviewer audits Phase 1.
2. Worker fixes review findings.
3. Main thread verifies with targeted reads and bounded test commands.

Repeat the same pattern for Phases 2-4. Do not proceed to the next phase while
the current phase's validation and review gates are open.

## Done

This effort is complete when:

- the loop can generate child candidates through a bounded `ploke-tui` tool
  edit surface;
- candidate generation is checked by `ploke-eval` surface grants before build;
- child evaluations still run concurrently via the existing channel/invocation
  path;
- History persists the edit/surface/proposal/delta evidence needed for
  traversal scoring and replay;
- History-backed traversal can select candidates generated by the new path;
- a short live run succeeds;
- a longer run can be started with the new generator and scoring knobs;
- reviewers have signed off on authority, naming, History, and runtime
  invariants.
