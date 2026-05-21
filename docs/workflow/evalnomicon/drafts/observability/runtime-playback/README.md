# Runtime Playback Observability

Status: draft architecture target.

This folder describes the larger playback model for Prototype 1 runtime
observability.

- [`agent-turn.md`](agent-turn.md)
  Turn-level drilldown for model exchanges, tool execution, edit proposals,
  replay prefixes, and model-facing trace queries.
- [`inventory/`](inventory/)
  Survey track for current record owners, emitted files, graph coverage gaps,
  join keys, playback placement, egui aggregates, and benchmark/operational
  metrics.

## Goal

`RuntimePlayback` is the read-only operator over `ploke_tree::Graph` that lets
CLI, `ploke-egui`, replay probes, charts, tables, and future model-facing tools
walk a self-improvement run at different levels of granularity.

The same playback cursor should be able to drive:

- a visual graph frame in `ploke-egui`;
- a selected-step detail panel;
- a tool-call table;
- a protocol/procedure timeline;
- a selection/evaluation view;
- a running token and cost chart;
- a replay-probe breakpoint that can drill down into one agent turn.

The central UI contract is one cursor over one ordered playback of graph state.
An artifact tree, token-cost bar chart, LLM request/response pane, protocol
table, tool-call table, and selected-step detail pane should all resolve from
the same `(graph, scope, cursor)` position. They may cache their rendered
forms, but they should not maintain independent notions of "current step".

The key design constraint is that these views are projections over one loaded
graph. They must not reparse files independently or create new authority from
rendered CLI output.

## Reduction To Avoid

Do not model runtime playback as "read this run directory and list events".

That shape is too small. It loses the larger History and graph structure:

- sealed History blocks and lineage-local authority;
- Artifact ancestry;
- Runtime derivation from Artifacts;
- operations where one Runtime acts over one target Artifact;
- attempts, invocations, and process/protocol phases;
- selection decisions and candidate-set evidence;
- child self-evaluation and benchmark procedures;
- agent turns, model exchanges, tool calls, edits, and cost events.

The durable object is not a run log. It is a graph-backed playback projection
over History, Artifacts, Runtimes, operations, attempts, procedures, and
turn-level traces.

## Carrier Shape

The borrowed form should be the primary UI/query form:

```rust
pub struct RuntimePlaybackRef<'g, G = RuntimeFine> {
    graph: &'g Graph,
    scope: PlaybackScope,
    index: RuntimePlaybackIndex,
    cursor: PlaybackCursor,
    _granularity: PhantomData<G>,
}
```

The owned form is for CLI output, exports, test fixtures, worker boundaries, and
WebAssembly handoff:

```rust
pub struct RuntimePlayback<G = RuntimeFine> {
    steps: Vec<G::Step>,
    _granularity: PhantomData<G>,
}
```

The scope is explicit. Callers should not imply scope from a path, selected UI
node, or scheduler generation:

```rust
pub enum PlaybackScope {
    GraphUniverse,
    Lineage(LineageId),
    ArtifactAncestry(ArtifactId),
    Runtime(RuntimeId),
    Operation(OperationCoordinate),
    Attempt(AttemptId),
    AgentTurn(AgentTurnId),
}
```

`RuntimePlaybackRef` borrows the graph and owns only playback position/index
state. It is not a record store, not an authority layer, and not a replacement
for History.

## Granularity

Granularity should stay typed, not stringly selected at callsites.

```rust
pub trait RuntimePlaybackGranularity {
    type Step;
    type StepRef<'g>
    where
        Self: 'g;
}

pub struct RuntimeCoarse;
pub struct RuntimeFine;
pub struct HistoryBlocks;
pub struct ProtocolTransitions;
pub struct Operations;
pub struct Attempts;
pub struct AgentTurns;
pub struct ModelExchanges;
pub struct ToolCalls;
pub struct EditLifecycles;
pub struct CostEvents;
```

Callsites should prefer typed iterators over switch-heavy consumers:

```rust
let playback = RuntimePlaybackRef::<RuntimeFine>::new(graph, scope);

for step in playback.steps() { ... }
for transition in playback.protocol_transitions() { ... }
for operation in playback.operations() { ... }
for turn in playback.agent_turns() { ... }
for call in playback.tool_calls() { ... }
for cost in playback.cost_events() { ... }
```

Those methods should return named iterator types such as
`ProtocolTransitionIter<'g>`, `OperationIter<'g>`, `AgentTurnIter<'g>`,
`ToolCallIter<'g>`, and `CostEventIter<'g>`. That keeps the UI and CLI callsites
clear without pushing semantic dispatch into renderers.

## Step Families

The runtime playback stream should be able to yield these families, depending
on granularity and scope:

- History block opened/sealed;
- History entry admitted or imported;
- Parent epoch start, lock, handoff, and successor validation;
- protocol transition or box/message access;
- operation attempt by a generator Runtime over a target Artifact;
- patch generation, application, validation, and derived Artifact creation;
- child invocation, self-evaluation, benchmark execution, and result;
- selection input, candidate considered, decision, and selected successor;
- agent turn;
- model exchange;
- tool call lifecycle;
- edit proposal lifecycle;
- cost event.

The same source event can participate in more than one projection. For example,
one model exchange may appear under an agent-turn drilldown, a patch-generation
procedure, and a cost chart. The projection should preserve the source ids and
evidence strength rather than cloning the fact into unrelated records.

## Graph Inputs And Joins

`RuntimePlaybackIndex` should be a derived index over graph facts. It should
not become another source of truth.

The important joins are:

- History block and entry ids to lineage-local playback order;
- selected successor entries to candidate-set and selection evidence;
- selected Artifact ids to Artifact ancestry;
- Runtime ids to the Artifact that hydrated them;
- operation coordinates to generator Runtime and target Artifact;
- attempt ids to invocation, process, workspace, and terminal records;
- protocol transitions to boxes, messages, and admitted/imported evidence;
- evaluation procedures to child runtime, benchmark instance, oracle, and
  result evidence;
- agent-turn ids to turn traces, run records, provider sidecars, and tool/edit
  events;
- model exchanges and tool calls to procedure context and cost attribution.

If a join is missing, playback should surface a partial step with an evidence
warning rather than filling the gap from filenames or scheduler status.

## Graph Coverage And Sequenced State

`RuntimePlayback` should turn a loaded `ploke_tree::Graph` into a sequence of
graph-state projections.

At cursor position `N`, the playback frame represents the meaningful subset of
graph facts visible after applying playback steps `0..=N` for the selected
scope. The selected scope can be narrow, such as one agent turn, or broad, such
as `PlaybackScope::GraphUniverse`, which means every loaded graph fact that can
be sequenced by the current playback index.

Every graph element that a UI or model-facing query can show should have at
least one relationship to runtime playback:

- primary step subject:
  the element is introduced, changed, selected, validated, failed, or completed
  by a playback step;
- contextual frame member:
  the element existed before the current cursor and remains visible as context;
- derived projection member:
  the element contributes to a chart, table, aggregate, score, or warning
  derived from the current frame or delta;
- evidence member:
  the element is a source record, History block, sidecar, or diagnostic artifact
  supporting a visible fact.

If a graph fact cannot be placed in one of those categories, the design should
make that explicit. Either it is static reference material outside playback, or
the graph is missing a step family or join. Renderers should not invent ordering
to make an orphan fact fit a view.

This is the contract that lets multiple UI surfaces stay synchronized. Playing
the graph forward or backward moves one `PlaybackCursor`; the artifact tree
uses the frame, charts fold the same deltas, tables append or retract rows from
the same step stream, and nested panes drill into the same selected step.

## Cursor, Frame, And Delta

Playback needs more than an iterator over labels. The egui use case wants a
visual graph that can play forward while related panels update from the same
position.

The read-side model should distinguish:

- `PlaybackCursor`
  The current position in a scoped playback stream.

- `RuntimePlaybackStepRef`
  The borrowed semantic step at that position.

- `RuntimePlaybackFrameRef`
  The graph overlay and visible-state projection after applying all steps
  through the cursor.

- `RuntimePlaybackDeltaRef`
  The minimal change introduced by the current step, useful for animation,
  chart updates, and table append behavior.

Sketch:

```rust
impl<'g, G> RuntimePlaybackRef<'g, G>
where
    G: RuntimePlaybackGranularity,
{
    pub fn cursor(&self) -> PlaybackCursor;
    pub fn seek(&self, cursor: PlaybackCursor) -> Self;
    pub fn step(&self) -> Option<G::StepRef<'g>>;
    pub fn frame(&self) -> RuntimePlaybackFrameRef<'g>;
    pub fn delta(&self) -> Option<RuntimePlaybackDeltaRef<'g>>;
}
```

Frames and deltas are projections. They do not mutate the graph and they do not
define History authority.

All view state should be derived from this cursor model:

- `frame()` answers "what graph state is visible at this cursor";
- `delta()` answers "what changed at this cursor";
- typed iterators answer "which step family is relevant to this view";
- drilldowns answer "which nested timeline belongs to this selected step".

This keeps visual playback, tables, charts, and detail panes coherent. A cached
chart series or rendered table may exist, but its cache key should include the
graph identity, playback index identity, scope, granularity, and cursor range it
was derived from.

## Procedure And Cost Attribution

Cost charts and token-spend breakdowns should fold over the playback stream,
not scrape provider logs independently.

Model exchanges and tool calls should carry or resolve to a procedure context:

```rust
pub enum ProcedureKind {
    PatchGeneration,
    ProtocolAdjudication,
    ChildSelfEvaluation,
    MultiSweBench,
    SearchContext,
    PatchValidation,
    Selection,
}
```

Then a running cost chart can be expressed as a typed projection:

```rust
RuntimePlaybackRef::<CostEvents>::new(graph, PlaybackScope::ArtifactAncestry(artifact))
    .filter_procedure(ProcedureKind::PatchGeneration)
    .fold_costs();
```

The same pattern should support:

- total tokens and cost over one lineage;
- token spend by procedure family;
- cost per selected successor;
- cost per failed tool repair loop;
- cost per benchmark/self-evaluation attempt;
- request counts and latency by model, provider, route, and tool family.

## Relationship To Agent Turns

An agent turn is a drilldown inside runtime playback.

`RuntimePlaybackRef::<AgentTurns>` should yield the ordered turns relevant to a
scope. From there, callers can enter the turn-level projection described in
[`agent-turn.md`](agent-turn.md):

```rust
for turn in runtime.agent_turns() {
    let timeline = turn.agent_turn_timeline();
    for exchange in timeline.model_exchanges() { ... }
    for call in timeline.tool_executions() { ... }
}
```

The agent-turn trace remains the right level for model request/response,
tool-call, edit-lifecycle, and replay-prefix details. Runtime playback is the
outer operator that answers which turns matter for a lineage, Artifact ancestry,
operation, attempt, procedure, or chart.

## Relationship To Current `RunPlayback`

The existing `RunPlayback<G>` / `RunPlaybackRef<'a, G>` vocabulary remains
useful as the current sealed-History playback slice and as an owned/borrowed
projection pattern.

The longer-term direction is:

```text
ploke-records
  passive schemas and evidence-strength vocabulary

ploke-tree::Graph
  canonical read-side index over loaded typed records

RuntimePlaybackRef<'g, G>
  graph-borrowing playback operator with typed iterators

RuntimePlayback<G>
  owned export/snapshot form for CLI, tests, workers, and wasm

CLI / egui / model-facing tools
  render or query playback projections
```

Do not make the CLI own runtime-playback semantics. The CLI should select a
scope, choose a granularity, and render a projection produced by `ploke-tree`.

## Evidence And Authority

Playback order should be causal, not timestamp or filesystem order.

Ordering priority:

1. sealed History lineage order;
2. admitted History entry order;
3. typed transition journal or protocol records;
4. invocation/runtime-local order;
5. agent-turn event order;
6. timestamps for latency, profiling, and anomaly detection only.

Coarsening must preserve the weakest relevant evidence class. A cost chart, UI
badge, or model-facing summary must not present passive runtime evidence as if
it were sealed History authority.

## `ploke-egui` Consumers

`ploke-egui` should consume borrowed playback projections over the graph:

- graph canvas:
  play forward Artifact, Runtime, operation, and History state;
- detail panel:
  show the selected step's typed facts and source evidence;
- tables:
  show tool calls, model exchanges, protocol transitions, selections, and
  benchmark attempts;
- charts:
  fold cost, token, latency, failure, and improvement metrics over the same
  cursor;
- replay panel:
  jump from an operation/turn/tool step into historical/live replay.

The UI should not reconstruct these relationships by reparsing run files or
copying graph facts into renderer-owned DTOs. It may cache render artifacts, but
the source facts should stay borrowed from `ploke_tree::Graph` or from explicit
owned playback exports.

## First Implementation Direction

The first implementation slice should not try to build every projection.

Recommended sequence:

1. Keep the current sealed-History `RunPlayback` path working.
2. Add the `RuntimePlaybackRef<'g, G>` and `PlaybackScope` vocabulary in
   `ploke-tree` without filesystem access.
3. Add one graph-backed scope: `PlaybackScope::Lineage`.
4. Add the next graph-backed scope: `PlaybackScope::ArtifactAncestry`.
5. Add one drilldown join from a playback step to current agent-turn evidence.
6. Add `AgentTurns` and `CostEvents` typed iterators as read-only projections.
7. Teach one CLI debug command to render those projections without adding CLI
   semantics.
8. Add a synthetic graph test proving one History-selected Artifact can be
   played forward into operation, agent-turn, tool-call, and cost projections.
9. Add one bounded real-run smoke check that reports counts and warnings only.

Stop there before wiring full egui playback. The point of the first slice is to
establish the graph-backed operator and typed iterator shape.

## Reference Spine

Start here before implementing or revising this plan:

- [`self-improvement-loop/handoffs.md`](../../../../../active/plans/self-improvement-loop/handoffs.md)
  Current shared handoff index for playback, graph ingestion, and active loop
  planning.
- [`2026-05-09_run-playback-typed-observability-plan.md`](../../../../../active/agents/2026-05-09_run-playback-typed-observability-plan.md)
  Earlier typed `RunPlayback` / `RunPlaybackRef` contract for evidence strength,
  ordering, and coarse/fine projections.
- [`2026-05-12_agent-turn-record-projection-handoff.md`](../../../../../active/agents/2026-05-12_agent-turn-record-projection-handoff.md)
  Current record ownership and writer-boundary handoff for agent-turn artifacts.
- [`prototype1_state/mod.rs`](../../../../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs)
  Prototype 1 runtime model and module-level orientation.
- [`prototype1_state/history.rs`](../../../../../../crates/ploke-eval/src/cli/prototype1_state/history.rs)
  Current History/block authority model and lineage-local ordering contract.
- [`trait-first-reification.md`](../../formal/trait-first-reification.md)
  Formal-design background for representing semantic objects with trait-level
  contracts before concrete carrier proliferation.
- [`typestate-sketches.md`](../../formal/typestate-sketches.md)
  Formal-design sketches for artifacts, procedures, and typed state progress.
- [`agent-turn.md`](agent-turn.md)
  Nested turn-level timeline design for model exchanges, tools, edits, adapter
  observations, replay, and model-facing trace queries.
- [`inventory/`](inventory/)
  Record-surface and aggregate survey for the larger runtime playback model.
