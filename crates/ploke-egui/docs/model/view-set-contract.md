# Graph View Set Contract

This note gives `ploke-egui` graph views a small set vocabulary so UI modes can
be implemented without drifting into unrelated record graphs.

## Universe

Let `RunGraph = ploke_tree::Graph`.

This note follows the edit-surface model vocabulary: `Artifact` is the
material/tree state, `Runtime` carries role authority, `History` is the sealed
authority substrate, and a projection is not authority. `ploke-egui` only
inspects graph facts produced elsewhere; it does not participate in selecting,
admitting, or advancing successors. To avoid cross-doc collisions, this note
does not use `G` for the loaded graph because the edit-surface model uses `G`
for surface grants.

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
         active Artifact -> selected successor Artifact
P_C    = parent-produced child Artifact edges:
         parent Artifact -> child Artifact, sourced from graph-owned child-plan continuity
P_B    = observed applied-patch Artifact provenance:
         base Artifact -> derived Artifact, sourced from branch/candidate records
P      = P_H union P_C
L      = primary-lineage subset of A and P_H
D      = non-artifact context/debug records: H, R, artifact-consideration
         records, selections, operations, source/projection attachments,
         warnings
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

For `ArtifactTree`, display identity also applies a promotion-continuity
quotient:

```text
selected child ArtifactId == later next-parent base ArtifactId
```

when that equality is proven by graph-owned child-plan continuity facts. This
quotient is for display identity in the default artifact view; it does not
erase the raw ids from inspector drilldown.

Branch, candidate, membership, and selection records that carry artifact ids
contribute consideration/evaluation/selection context to the same `A` node.
They do not create separate material-state nodes in `ArtifactTree`.

ArtifactTree filters should be stated as predicates over `A`, not as new
material node sets. For example:

```text
A_considered = { A(a) | exists branch/candidate context fact k referring to a }
```

`P_H` and `P_C` are not equivalent relation sources. `P_H` is derived from
sealed History and represents an admitted successor transition. `P_C` is a
graph-owned parent-produced-child relation sourced from child-plan continuity.
`P_B` remains observed applied-patch provenance sourced from
branch/candidate records. A selected candidate Artifact can be represented in
all three families, but `P_B` alone must not imply History admission or
artifact-parent display topology.

## Canonical Mapping

Use [run-graph-crosswalk.md](run-graph-crosswalk.md) for the record and graph
source of each set. Its Artifact Node Contract and context/edge vocabulary are
the canonical place to check whether a recorded thing is an artifact node,
a filter/mark/provenance source, an edge source, or debug/drilldown material.
In short:

```text
A   = resolved artifact identities from RunGraph.artifacts
P_H = HistoryBlockNode.active_artifact -> selected_successor.artifact
P_C = graph-owned child-plan continuity:
      parent Artifact -> produced child Artifact
P_B = applied-patch Artifact provenance observed through CandidateBranchNode.base_artifact_id
      -> derived_artifact_id
```

Relations such as `opened_from_artifact`, `parent_branch_id`,
`source_state_id`, `parent_node_id`, runtime hydration, operation targeting,
and source/projection attachments may exist in `RunGraph`, but they are not
edges in the default `ArtifactTree` unless a graph-owned projection explicitly
admits them into that view.

The graph-owned `ArtifactTree(RunGraph)` projection also computes:

```text
primary_lineage = loaded lineage maximizing (block_count, max_block_height)
current_ruler   = selected_successor artifact from max-height primary-lineage block
```

Those rules are part of the `ploke-tree` projection boundary. They should not
be reimplemented in new UI surfaces.

## Product Views

### ArtifactTree

Default product surface.

```text
N_artifact = A
E_artifact = P
Mark_artifact includes current-ruler highlight from latest primary-lineage
selected successor plus produced-child dimming/dotted marks for children that
never entered current-generation consideration.
```

Properties:

- Edge direction is parent Artifact -> child Artifact.
- `D` and `SYN` are not rendered.
- Labels are compact display handles such as `A1`, `A2`, `P1`, and `P2`;
  raw ids belong in detail/debug text.
- The primary lineage subgraph should be weakly connected when all required
  History artifacts are loaded.
- The full artifact graph may have multiple weak components. Extra components
  are diagnostics or secondary islands, not a reason to add synthetic anchors.
- Applied-patch Artifact edges observed through branch/candidate sources should
  remain visually distinguishable from admitted History transition edges once
  styling supports that distinction.

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
E_debug may include record and source/projection relations
View_debug not subset View_artifact
```

Debug views may render `D` or `SYN`, but they must be explicit modes. They must
not change the default artifact-tree contract.

Future edit-surface drilldowns may introduce explicit nodes or relations for
surface grants, edit proposals, surface checks, and History admission. Those
belong to debug or drilldown views unless they are reduced to admitted Artifact
transition edges in `P_H` or produced-child Artifact edges in `P_C`. Observed
applied-patch Artifact provenance in `P_B` may support those drilldowns without
becoming the default visible canvas spine.

## Implementation Checks

For each view mode, tests should assert:

- node set membership matches the formulas above;
- edge set membership matches the formulas above;
- hidden nodes and edges do not draw, hit-test, lay out, or appear selected;
- artifact edge direction is top-down parent to child;
- `ArtifactTree` has no `D` or `SYN` nodes;
- `P_B` applied-patch Artifact edges do not get treated as admitted History transitions;
- current-ruler highlighting marks an Artifact associated with a selected
  successor Runtime/role transition, not an Artifact that owns role authority;
- disconnected artifact components are reported as diagnostics, not connected
  with synthetic anchors.
