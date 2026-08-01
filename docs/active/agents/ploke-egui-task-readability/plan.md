# ploke-egui Task-Readability Plan

Date: 2026-05-12

## Objective

Make the first `ploke-egui` graph view useful by measuring and improving task
readability of the artifact-first projection.

The immediate UI question is:

> How quickly can a viewer recover the current/promoted artifact lineage and
> its local alternatives from the image?

This is not a screenshot-beautification pass. The screenshot is only evidence
for humans. The implementation should diagnose the actual layout from:

- tree/projection structure,
- assigned node positions,
- rendered label bounds,
- edge paths,
- selected/promoted lineage membership,
- rank and subtree geometry.

## Semantic Boundary

Hard rule:

`ploke-tree::graph::Graph` is the semantic authority. `ploke-egui` must not
build or own a second semantic graph.

Allowed in `ploke-egui`:

- view state,
- interaction state,
- layout caches,
- node/label/edge geometry,
- diagnostic findings,
- rendering style,
- colors,
- render-only text artifacts that cannot be used to recover semantic identity.

Not allowed in `ploke-egui`:

- egui-owned domain `Artifact`, `Runtime`, `Patch`, `Candidate`, `Selection`,
  or `History` carriers that become authority,
- direct parsing of Prototype 1 files or owned persisted JSON,
- allocated values cloned out of `ploke_tree::Graph` for later semantic use,
- copied semantic facts, ids, refs, records, or wrapper reports that drift from
  `ploke-tree::Graph`,
- row-shaped semantic carriers such as `InspectorRow` for Artifact, Runtime,
  role, History, candidate, evidence, or source-status facts,
- mirror types that reconstruct graph meaning to avoid borrowing from
  `ploke-tree::Graph`,
- screenshot-first heuristics as the primary diagnostic source.

If a needed fact belongs to the execution/archive model, add it deliberately to
`ploke-tree::graph::Graph` or its typed loaders. If it is only geometry,
legibility, layout, or inspector presentation, keep it in `ploke-egui`.

## Reference-Only Rule

The reference rule is strict and phase-blocking:

`ploke-egui` must use references into `ploke_tree::Graph` for semantic access.

Any value allocated or cloned out of `ploke_tree::Graph` and stored for later
semantic use is a hard blocker. A phase that completes with such a clone is
invalid and must be repaired immediately before any downstream work continues.

This includes copied ids, copied refs, copied records, copied partial records,
and wrapper carriers such as `Option<SomeReport>` that merely repackage graph
meaning. Renaming the copy as a "view model", "report", "status", "summary",
"payload", "info", or "projection" does not make it acceptable.

Allowed derived values are values whose semantics are newly computed and cannot
act as substitutes for graph facts, for example:

- positions,
- rectangles,
- edge curves,
- collision counts,
- rank spacing statistics,
- average token counts,
- summed durations,
- visibility scores,
- color choices,
- render-only strings that are never used as join keys or semantic evidence.

When in doubt, treat the value as semantic until proven otherwise. If a third
party widget requires owned values, isolate them as render-only artifacts and
audit that they cannot be used to recover, compare, or join graph identity.
That exception must be reviewed in the same phase and is not a license to store
semantic ids or records in egui.

Every implementation review must include this rule. At minimum, run one
reference-boundary audit per phase. Prefer folding the audit into every review
step. Any violation is a hard failure, not deferred cleanup.

## Larger Object

The object being built is a task-readability feedback loop over a typed archive
graph:

```text
ploke-records typed records
  -> ploke-tree RunRecordSet
  -> ploke-tree::graph::Graph
  -> borrowed egui artifact projection
  -> layout geometry
  -> diagnostics and ranked findings
  -> layout/projection tuning
```

The tempting reduction to refuse is:

```text
screenshot looks bad -> tweak layout constants or invent egui graph facts
```

The correct reduction is:

```text
layout diagnostics explain task-readability failures -> tune projection/layout
without moving semantic authority out of ploke-tree
```

## Current Code State

Known current surfaces:

- `crates/ploke-egui/src/import/mod.rs`
  delegates to `ploke_tree::Graph::from_records`.
- `crates/ploke-egui/src/ui/view/projection.rs`
  derives an artifact tree, History artifact fallback, or candidate inventory
  from `&ploke_tree::Graph`.
- `crates/ploke-egui/src/ui/view/layout.rs`
  assigns tree-like node positions.
- `crates/ploke-egui/src/ui/view/diagnostics.rs`
  computes graph fill, label diagnostics, edge crossings, long edges,
  backtracking, and selected-path crossings.
- `crates/ploke-egui/src/diagnostics/mod.rs`
  persists native diagnostic snapshots.

The visible problem is that the current projection can still feed the renderer
a thin chain or flat candidate inventory. The next work should make that failure
measurable before tuning layout.

## Diagnostic Families

Phase 1 should extend diagnostics in this order:

1. **Occlusion**
   - label overlap count,
   - edge-label intersections,
   - node-label or edge-node interference if geometry is available.
2. **Primary lineage salience**
   - selected/promoted path visibility,
   - selected/promoted path straightness,
   - crossings involving selected/promoted path.
3. **Hierarchy readability**
   - rank spacing mean/min/coefficient of variation,
   - parent-child rank monotonicity,
   - parent centroid deviation.
4. **Subtree separation**
   - sibling subtree span overlap,
   - middle-rank density or interpenetration.
5. **Composition and edge clutter**
   - raw and informative fill,
   - weighted edge crossings,
   - long-edge share,
   - local edge density only after simpler metrics prove insufficient.

The first target vector is:

```text
fill_x
fill_y
label_overlap_count
edge_label_intersections
weighted_edge_crossings
generation_spacing_cv
subtree_overlap_score
primary_path_visibility
primary_path_straightness
```

## Findings

Diagnostics should become ranked findings, not just counters.

Example finding shape:

```text
Severity: High
Primary lineage is not visually dominant.
- selected path crossings: 7
- occluded selected labels: 4
- straightness score: 0.41
```

Findings are view diagnostics. They are not persisted loop facts and must not
be used as active loop authority.

## Phases

### Phase 0: Board And Boundary Setup

- Create a clean orchestration lane set or an explicitly isolated new wave in
  `.orchestrator`.
- Validate lane-owned edit surfaces before spawning workers.
- Dispatch one retainer first if context needs refreshing.
- Run an initial reference-boundary audit. Existing cloned semantic values in
  the active path are blockers to resolve before Phase 1 can be accepted.

### Phase 1: Measurement Before Tuning

- Add only borrowed projection structure needed for diagnostics. Do not add
  owned semantic join handles, copied ids, wrapper report types, or row-shaped
  semantic carriers.
- Extend geometry diagnostics for ranks, selected path, and subtree spans.
- Persist snapshot fields and ranked findings.
- Add focused unit tests over synthetic artifact trees.

### Phase 2: Layout Improvements

- Tune tree layout using diagnostic feedback.
- Prefer changes that improve selected lineage salience and subtree separation
  without overfitting to one screenshot.
- Keep edge labels minimal (`P1`, `P2`, `P3`) and visually quiet.

### Phase 3: Coverage Inventory Refresh

- Refresh the stale graph ingestion inventory for only facts needed by the
  artifact view and near-term drilldown.
- Classify gaps as:
  - graph semantic gap,
  - loader/typed-record gap,
  - egui projection gap,
  - view-only diagnostic gap.

### Phase 4: Review And Handoff

- Run graph-boundary review after Phase 1.
- Run readability review after Phase 2.
- Write a short handoff before closing or refreshing workers.

## Dependency Policy

Projection identity blocks rank/subtree/primary-lineage diagnostics. If the
projection lane is blocked:

- diagnostics workers may continue on label/edge/fill metrics,
- layout workers may add tests or inspect current algorithm behavior,
- inventory workers may refresh coverage docs,
- no worker may invent semantic keys, mirror carriers, or copied ids in egui to
  bypass the block.

If diagnostics are blocked by missing geometry:

- add geometry capture in `ploke-egui`,
- do not parse screenshots as the primary source,
- keep screenshot checks as later regression evidence only.

If a semantic relation is missing from `ploke-tree::Graph`:

- record the missing relation in the inventory,
- open a graph lane task,
- keep egui changes blocked or degraded with an explicit warning.

## Done Criteria

The first task-readability wave is done when:

- `ploke-egui` still imports only through `ploke_tree::Graph`,
- no egui-owned duplicate semantic graph has appeared,
- reference-boundary audit finds no allocated semantic values cloned out of
  `ploke_tree::Graph` in the accepted path,
- diagnostics report at least rank spacing, selected-path crossings,
  selected-path straightness, and subtree overlap for artifact-tree views,
- native snapshots persist the new diagnostic vector and ranked findings,
- at least one synthetic tree test catches a known bad layout condition,
- a boundary review and readability review have been recorded.
