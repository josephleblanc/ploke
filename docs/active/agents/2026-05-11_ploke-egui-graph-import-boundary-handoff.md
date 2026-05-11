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
- `ploke-tree::graph::Graph` is now the first read-side graph/index boundary
  over `RunRecordSet`. Its first implemented layer indexes sealed History
  lineages, blocks, admitted entries, selection decisions, candidate
  memberships, artifact/runtime refs, and typed evidence attachments.
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

The graph design has started promoting the stable read-side object into
`ploke-tree` rather than continuing to make `ploke-egui` the only owner of
semantic graph structure. `ploke-egui` may keep a view graph or adapter, but it
should not be the only crate that can step through the loop graph. The read-side
graph/index should be reusable for UI, CLI-free investigation, tests, and later
research analysis.

The current `ploke-tree::browser::RunExecutionGraph` is evidence that a partial
graph spine already exists. The remaining problem is not "no graph exists"; it
is that the graph is split between browser projection, egui import/view logic,
and the new `ploke-tree::graph::Graph` boundary. The next consolidation step is
to move semantic consumers toward `ploke-tree::graph::Graph` and keep browser
or egui DTOs as projections.

## Graph Content Model

The graph is not a mirror of every persisted file family. It is a small
semantic graph with a larger typed evidence index.

The core graph object should model:

- `Lineage`: the History authority coordinate. A lineage is not a git branch,
  worktree path, process id, runtime id, or Artifact identity.
- `HistoryBlock` / authority epoch: one sealed Crown epoch for one lineage,
  keyed by block hash, block id, lineage id, lineage-local height, parent block
  hashes, opening authority, ruling authority, surface commitment, active
  artifact, and selected successor.
- `HistoryEntry`: a provenance-bearing fact admitted into one block. The entry
  preserves subject, procedure/policy, executor, observer, recorder, proposer,
  admitting authority, ruling authority, operational environment, input/output
  refs, payload ref/hash, and payload kind.
- `Artifact`: recoverable artifact identity from History refs, backend/tree
  commitments, or shared passive artifact ids. Worktree paths and branch names
  are handles, not semantic identity.
- `ArtifactSurface` / `SurfaceCommitment`: the partitioned immutable/mutated/
  ambient surface commitments that constrain successor admission and explain
  artifact transitions.
- `Runtime`: a concrete execution hydrated from an Artifact. Runtime role is a
  role occurrence (`Parent`, `Child`, `Successor`), not a permanent process or
  path label.
- `OperationCoordinate`: generator Runtime plus target Artifact or surface.
  This is the provenance coordinate for patch generation; git ancestry alone
  cannot recover the generator.
- `PatchAttempt` / `Patch` / `SurfaceDelta`: the attempted or applied edit that
  derives one Artifact from another.
- `CandidateOccurrence`: one observed candidate in a source class such as
  current generation or previously admitted History.
- `CandidateSet` / `CandidateMembership`: the authenticated universe considered
  by one selection decision. Membership and occurrence identity must not be
  collapsed into a bare child node id.
- `SelectionDecision`: the sealed decision over a candidate universe, including
  selected occurrence/membership when present, ordered considered payloads,
  candidate-set root, traversal evidence, decision outcome, and projection
  failures.
- `Handoff` / hydration: transition from selected Artifact to successor
  Runtime and then to the next `Parent<Ruling>` after startup validation.
- `Ingress`: late/backchannel observations outside a sealed epoch until they
  are imported under an explicit policy.

Record families then attach to those graph objects as typed evidence:

- `agent-turn-trace.json` and `agent-turn-summary.json` attach to Runtime turns
  or operations as behavioral evidence.
- tool calls/results attach to operations, provider attempts, evaluations, or
  runtime turns.
- provider attempts, retries, timeouts, and provider failures attach to LLM
  attempts inside a runtime turn or operation.
- protocol artifacts attach to operations, evaluations, or inspector refs.
- branch/evaluation/metrics records attach to candidate, evaluation, selection,
  or comparison relations.
- scheduler/node/request/result mirrors provide labels, paths, status, and
  recovery context; they do not define ordering or successor authority.
- logs and streams are weak operator evidence unless a typed record cites them.

When a record cannot be joined to a semantic object without guessing, the graph
should keep it as unresolved evidence with an explicit warning. It must not
invent lineage, selection, or operation authority from a weaker projection.

## Prior Docs

The 2026-05-09 egui/browser handoffs are useful historical context, but they
describe a browser-model JSON renderer boundary that is no longer the central
implementation direction for `ploke-egui`.

The still-current rule from those docs is that front-end renderers consume typed
projection objects; they do not read raw Prototype 1 files.

Relevant current review:

- `docs/active/agents/2026-05-11_063230_ploke-egui-dependency-leverage-review.md`

Useful reference docs:

- `docs/active/agents/ploke-tree-graph-ingestion/README.md`
  for the current sub-agent coordination packet, edit boundaries, retry rules,
  and module split lanes for `ploke-tree::Graph` ingestion.
- `docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md`
  for the accepted typed-persistence inventory mapped to current
  `ploke-tree::Graph` ingestion status.
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
