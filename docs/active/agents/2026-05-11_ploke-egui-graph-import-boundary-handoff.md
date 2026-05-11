# ploke-egui Graph Import Boundary Handoff

Date: 2026-05-11

## Current Boundary

`ploke-egui` is organized around `ploke-egui::graph::Graph`.

The intended import chain is:

```text
persisted Prototype 1 run records
-> ploke-records typed persisted shapes
-> ploke-tree read-only run record/projection loaders
-> ploke-egui::graph::Graph
-> borrowed egui views, layout, diagnostics, and snapshots
```

`ploke-egui` should not parse Prototype 1 files directly, and `ploke-eval`
should not emit egui/browser-shaped DTOs for this view.

## Ordering Invariant

The read-side graph should orient around sealed Prototype 1 History as its
primary ordering spine.

This is a playback/read invariant, not active-loop authority. The graph must
not use this index to drive sealing, successor admission, or live loop
execution. It is the canonical read model for a completed or observed run:
given the persisted records available on disk, it should provide one cohesive
index over loop members, transitions, evidence, and known ordering.

Entities admitted to History must already carry enough identity to resolve to a
unique target. If the same parent appears as Ruler in multiple History blocks,
that is the same parent/artifact identity appearing at multiple positions in
the succession of authority. A later rerun may recompile or re-execute the same
underlying artifact to generate or evaluate different child candidates, but it
does not change that artifact's lineage, patch identity, or core membership in
History.

Other orderings are secondary to History block succession. Journal order,
branch-log append order, filesystem observation order, timestamps, provider
attempt order, and UI/layout order may help attach evidence or break ties, but
they must not replace the sealed History spine. When a record cannot be ordered
or resolved through History-derived identity, the graph should carry that
ambiguity explicitly instead of inventing authority from a weaker ordering.

Future work may add a shared record trait in `ploke-records` for consistent
phase/timestamp metadata across passive records. That should improve secondary
ordering and evidence attachment, not move ordering authority away from
History.

## Code State

- `ploke-tree::RunRecordSet` is the current read-only loaded-run carrier.
  It bundles:
  - `RunForestInput`
  - sealed History blocks
  - typed transition journal entries
- `ploke-tree::FsRunStore::load_record_set()` is the filesystem entry point
  for that carrier.
- `ploke-egui::import::graph_from_run_root()` now calls
  `FsRunStore::load_record_set()` and builds `Graph` from the loaded carrier.
- `ploke-egui::import::graph_from_run_records()` takes `&RunRecordSet`.

## Graph Consolidation State

There are currently two graph-like shapes that should not become competing
semantic centers:

- `ploke_tree::browser::RunExecutionGraph` is a renderer-neutral serialized
  projection attached to `PlaybackBrowserModel`. It is built in
  `crates/ploke-tree/src/browser.rs` by `build_execution_graph()`. This is
  useful compatibility/projection code, but it is still browser-shaped: nodes
  and edges carry labels, optional ids, and serialized DTO fields.
- `ploke_egui::graph::Graph` is the current egui semantic graph. It owns
  candidates, artifacts, runtimes, operations, relations, and evidence, and
  `ploke-egui::import` builds it from `ploke_tree::RunRecordSet`.

The consolidation direction is:

```text
ploke-records
  passive typed persisted record shapes

ploke-tree
  typed filesystem loading, History-spined read-side graph/index, playback,
  and renderer-neutral projections

ploke-egui
  views, layout, interaction, diagnostics, and snapshots over the ploke-tree
  graph/index
```

`ploke-tree-browser` is only a compatibility facade over `ploke_tree::browser`.
Do not add new semantics there.

The next graph design should promote the stable read-side object into
`ploke-tree` rather than continuing to make `ploke-egui` the only owner of
semantic graph structure. `ploke-egui` may keep a view graph or adapter, but it
should not be the only crate that can step through the loop graph. The read-side
graph/index should be reusable for UI, CLI-free investigation, tests, and later
research analysis.

The current `ploke-tree::browser::RunExecutionGraph` is evidence that a partial
graph spine already exists. The remaining problem is not "no graph exists"; it
is that the graph is split between browser projection and egui import/view
logic, and full History-spined joins/evidence attachment are not yet
consolidated into one read-side object.

## Prior Docs

The 2026-05-09 egui/browser handoffs are useful historical context, but they
describe a browser-model JSON renderer boundary that is no longer the central
implementation direction for `ploke-egui`.

The still-current rule from those docs is that front-end renderers consume typed
projection objects; they do not read raw Prototype 1 files.

Relevant current review:

- `docs/active/agents/2026-05-11_063230_ploke-egui-dependency-leverage-review.md`

Useful reference docs:

- `docs/active/plans/self-improvement-loop/typed-persistence-spine/traceability-matrix.md`
  for graph primitive, identity, join, replay, and evidence-strength vocabulary.
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`
  for operator-facing graph questions and acceptance criteria.

The old typed-persistence operating-console workflow is not the active
execution controller for this lane. Use this handoff plus the task-stack items
below to stay oriented. Do not edit `ploke-eval` for this work unless the user
explicitly reauthorizes it.

## Open Task Stack

Current focus:

- `ploke-egui-graph-core`: define the graph as the central UI object without
  creating copied semantic authorities in view payloads.

Current child tasks:

- `ploke-tree-history-spined-graph`: build the read-side graph/index over
  `ploke-tree::RunRecordSet`, with sealed History block succession as the
  primary ordering spine.
- `ploke-egui-view-borrows-graph`: make egui views, widget payloads, labels,
  styles, and diagnostics borrow or join back to the graph for semantic facts.
- `ploke-egui-history-edge-display-policy`: decide whether History succession
  renders as primary edges, overlay, side rail, ruler, or inspector-only
  relation.
- `ploke-egui-graph-diagnostics-feedback`: make diagnostics report the layout
  failure modes needed to tune the History-spined graph without manual visual
  guessing.

## Next Checks

- Keep parsing and filesystem record loading in `ploke-tree` / `ploke-records`.
- Keep `ploke-egui` focused on adapting `RunRecordSet` into `Graph`.
- Audit view payloads for copied semantic facts that should instead be joined
  back through `&Graph`.
- Use graph diagnostics to decide whether History succession edges remain
  primary visible edges or become a side/overlay layer.

## Verification

```bash
cargo test -p ploke-tree fs_run_store_loads_record_set
cargo test -p ploke-egui history_succession_preserves_scheduler_edge_as_distinct_evidence
cargo check -p ploke-egui --target wasm32-unknown-unknown
```
