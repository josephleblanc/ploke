## ploke-egui Artifact View Handoff

Date: 2026-05-12

Related:

- [`crates/ploke-egui/README.md`](../../../crates/ploke-egui/README.md)
- [`crates/ploke-eval/src/cli/prototype1_state/mod.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs)
- [`docs/active/plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`](../plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md)
- [`docs/active/plans/self-improvement-loop/frontend-questions.md`](../plans/self-improvement-loop/frontend-questions.md)
- [`2026-05-11_ploke-egui-graph-import-boundary-handoff.md`](2026-05-11_ploke-egui-graph-import-boundary-handoff.md)

## Thread Goal

Land the first good-looking, honest `ploke-egui` graph view by starting with
artifact-first tree geometry and deferring deeper runtime/tool/agent-turn detail
to drilldown surfaces.

This thread is not about adding a second semantic graph in `ploke-egui`. It is
about making the existing `ploke-tree::graph::Graph` visible in a clean first
projection.

## Hard Constraints

- `ploke-tree::graph::Graph` is the semantic authority.
- `ploke-egui` must consume only references to that graph or derived view
  projections over it.
- No duplicate semantic graph may exist in `ploke-egui`.
- If a fact belongs in `ploke-tree::graph::Graph`, do not re-home it in egui.
- Do not revert or work around other in-flight edits in the repo. This thread
  must tolerate concurrent work.

## Underlying Object

The object we are modeling is the execution/archive graph described by
`prototype1_state::mod`, not a git viewer and not a log timeline.

The important base relations are:

- Artifact: checkout state that can hydrate a Runtime.
- Runtime: executing process hydrated from an Artifact.
- Patch / patch attempt: edit operation producing a derived Artifact.
- History: sealed authority order for one or more lineages.

The core causal shape remains:

```text
Runtime -> Surface(Artifact) -> PatchAttempt
PatchAttempt + base Artifact -> derived Artifact
derived Artifact -> hydrated Runtime
```

The first UI does not need to display that full richness at once. It needs a
useful first projection of it.

## Initial Visual Goal

Start with the artifact view.

Primary geometry:

- nodes: artifact states
- edges: applied patch / derivation edges
- edge labels: minimal patch labels such as `P1`, `P2`, `P3`
- curves: clean bezier-like branch geometry that reads as a tree first

History is still crucial, but not as the primary geometry:

- use History as reveal order
- use History as dimming/stepping/highlight state
- use History to highlight the current `Parent<Ruler>`
- do not collapse the display into a sealed-History chain

This matches the current `crates/ploke-egui/README.md` direction: the first
view should be simpler than the full hypergraph-like object, while preserving
the sense of branching artifact evolution.

## What Not To Do

- Do not rebuild a separate semantic `Graph` in `ploke-egui`.
- Do not make runtime/tool/agent-turn detail part of the initial node geometry.
- Do not let tool calls, logs, or protocol artifacts become the organizing
  structure of the canvas.
- Do not optimize the first landing around hundreds of tool calls or deep
  runtime evidence on-screen.

Those surfaces belong later as typed drilldown, not as the first visual layer.

## Drilldown Direction

The later expansion path is already constrained by the UI drilldown contract:

- graph primitives are projections over typed records, not authority
- deeper detail must remain reachable through typed joins
- runtime/tool/provider/agent-turn facts are evidence attached to graph objects
- selection, hover, and inspector panels should open typed drilldown views
  later rather than overloading the initial canvas

So the intended progression is:

1. artifact-first tree view with minimal patch labeling
2. reveal/highlight and score/eval overlays
3. typed inspector/drilldown for runtime, tool, provider, and agent-turn detail
4. additional alternate views once the first projection is legible

## Current Problem Statement

The recent `ploke-egui` reorientation toward `ploke-tree::graph::Graph` was the
right semantic move, but the current projection is visually worse because it is
too thin:

- artifact chain when History is present
- flat candidate inventory otherwise

That is why the display regressed. The renderer is being fed a chain or a pile
instead of the readable artifact/branch tree the user wants.

## Next Implementation Direction

Keep the semantic boundary where it is and fix the projection.

Near-term work should:

1. Review the existing artifact/candidate/selection/History relations already
   present in `ploke-tree::graph::Graph`.
2. Use those existing relations to recover artifact-first tree geometry for
   egui, rather than introducing new egui-owned semantic carriers.
3. Label patch edges minimally (`P1`, `P2`, ...) and keep the first pass
   visually quiet.
4. Leave deeper runtime/tool/agent-turn presentation to later drilldown/table
   work.

If a missing relation is truly required for honest projection, add it to
`ploke-tree::graph::Graph` deliberately as semantic authority, not as a
`ploke-egui`-local mirror.

## Read Order For Restart

If restarting this thread cold, read in this order:

1. [`crates/ploke-egui/README.md`](../../../crates/ploke-egui/README.md)
2. [`crates/ploke-eval/src/cli/prototype1_state/mod.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs)
3. [`2026-05-11_ploke-egui-graph-import-boundary-handoff.md`](2026-05-11_ploke-egui-graph-import-boundary-handoff.md)
4. [`docs/active/plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`](../plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md)
5. [`docs/active/plans/self-improvement-loop/frontend-questions.md`](../plans/self-improvement-loop/frontend-questions.md)

Then inspect the current `ploke-egui` projection/layout code and compare it
against this handoff before editing.
