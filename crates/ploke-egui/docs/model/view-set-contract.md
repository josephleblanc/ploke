# Graph View Set Contract

This note gives `ploke-egui` graph views a small set vocabulary so UI modes can
be implemented without drifting into unrelated record graphs.

## Universe

Let `RunGraph = ploke_tree::Graph`.

This note follows the edit-surface model vocabulary: `Artifact` is the
material/tree state, `Runtime` carries role authority, `History` is the sealed
authority/evidence substrate, and a projection is not authority. To avoid
cross-doc collisions, this note does not use `G` for the loaded graph because
the edit-surface model uses `G` for surface grants.

Every rendered view is a projection:

```text
View_m(RunGraph) = (N_m, E_m, Mark_m)
```

where `N_m` is the rendered node set, `E_m` is the rendered edge set, and
`Mark_m` is render-only highlighting, dimming, labels, selection, and layout
state. `Mark_m` must not add semantic facts.

Base sets:

```text
A      = resolved Artifact nodes observed in RunGraph
H      = History records / blocks observed in RunGraph
R      = Runtime records observed in RunGraph
P_H    = admitted History transition edges:
         active/opened Artifact -> selected successor Artifact
P_B    = observed candidate derivation edges:
         candidate branch base Artifact -> derived Artifact
P      = P_H union P_B
L      = primary-lineage subset of A and P_H
D      = non-artifact debug records: H, R, candidates, selections,
         operations, evidence, warnings
SYN    = synthetic connector or anchor nodes
```

Required disjointness:

```text
A disjoint D
A disjoint SYN
D disjoint SYN
```

`P` may only connect nodes in `A`. If an edge endpoint is missing from `A`, the
edge is absent from the rendered view and may be counted as hidden diagnostics.

For `ArtifactTree`, node identity is the resolved Artifact id, not the source
reference wrapper. A History `ArtifactRef("artifact:<id>")` and a passive
`ArtifactId("<id>")` denote the same node in `A` for display.

`P_H` and `P_B` are not equivalent authority classes. `P_H` is derived from
sealed History and represents an admitted transition. `P_B` is an observed
candidate derivation edge and must not imply `Admit(t)` or
`ArtifactTransition(q)` by itself.

## Canonical Mapping

Use [run-graph-crosswalk.md](run-graph-crosswalk.md) for the record and graph
source of each set. In short:

```text
A   = resolved artifact identities from RunGraph.artifacts
P_H = HistoryBlockNode.active_artifact -> selected_successor.artifact
P_B = CandidateBranchNode.base_artifact_id -> derived_artifact_id
```

The current `ploke-egui` implementation also computes:

```text
primary_lineage = loaded lineage maximizing (block_count, max_block_height)
current_ruler   = selected_successor artifact from max-height primary-lineage block
```

Those rules are implementation facts until `ploke-tree` exposes a borrowed
`ArtifactTree(RunGraph)` projection. They should not be reimplemented in new UI
surfaces.

## Product Views

### ArtifactTree

Default product surface.

```text
N_artifact = A
E_artifact = P
Mark_artifact includes current-ruler highlight from latest primary-lineage
selected successor.
```

Properties:

- Edge direction is parent Artifact -> child Artifact.
- `D` and `SYN` are not rendered.
- Labels are compact display handles; raw ids belong in detail/debug text.
- The primary lineage subgraph should be weakly connected when all required
  History artifacts are loaded.
- The full artifact graph may have multiple weak components. Extra components
  are diagnostics or secondary islands, not a reason to add synthetic anchors.
- Candidate derivation edges should remain visually distinguishable from
  admitted History transition edges once styling supports that distinction.

### Lineage

Primary-lineage focus.

```text
N_lineage = A intersect L
E_lineage = P_H intersect (L x L)
View_lineage subset View_artifact
```

Properties:

- Empty if no loaded primary lineage exists.
- Shows the selected History spine as artifact-to-artifact patch flow, not as
  HistoryBlock nodes.
- May reuse the current-ruler highlight from `ArtifactTree`.

### ArtifactAndLineage

Overlay mode.

```text
N_both = N_artifact union N_lineage = N_artifact
E_both = E_artifact union E_lineage = E_artifact
Mark_both = Mark_artifact union lineage emphasis
```

Because `Lineage` is a subset of `ArtifactTree`, this mode should change visual
emphasis, not introduce new semantic nodes.

### Empty

Control mode for visibility and diagnostics.

```text
N_empty = empty set
E_empty = empty set
```

No selected hidden node or edge may remain interactable.

## Debug Views

Any all-record or drilldown graph is separate from the product views:

```text
N_debug subset A union D union SYN
E_debug may include record and evidence relations
View_debug not subset View_artifact
```

Debug views may render `D` or `SYN`, but they must be explicit modes. They must
not change the default artifact-tree contract.

Future edit-surface drilldowns may introduce explicit nodes or relations for
surface grants, edit proposals, surface checks, and History admission. Those
belong to debug or drilldown views unless they are reduced to admitted Artifact
transition edges in `P_H` or observed candidate derivation edges in `P_B`.

## Implementation Checks

For each view mode, tests should assert:

- node set membership matches the formulas above;
- edge set membership matches the formulas above;
- hidden nodes and edges do not draw, hit-test, lay out, or appear selected;
- artifact edge direction is top-down parent to child;
- `ArtifactTree` has no `D` or `SYN` nodes;
- `P_B` edges do not get treated as admitted History transitions;
- current-ruler highlighting marks an Artifact associated with a selected
  successor Runtime/role transition, not an Artifact that owns role authority;
- disconnected artifact components are reported as diagnostics, not connected
  with synthetic anchors.
