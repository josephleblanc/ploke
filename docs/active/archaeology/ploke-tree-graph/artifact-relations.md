# Artifact Relations

Status: `draft`

## 1. Target Entity / Visible Claim

The default artifact canvas renders artifact-to-artifact relations from the
borrowed `ploke_tree::graph::artifact_tree::Tree<'g>` with:

- visible primary edge set: `P_H ∪ P_C`
- graph-owned relation inventory/context: `P_O` and `P_B`

## 2. Competing Carriers

| carrier | location | semantic class | notes |
| --- | --- | --- | --- |
| `HistoryEdge` in `Tree.history_successors` | `ploke_tree::graph::artifact_tree::Tree` | relation | sealed History successor `P_H` |
| `ProducedChildEdge` in `Tree.produced_child_edges` | `ploke_tree::graph::artifact_tree::Tree` | relation | parent-produced child artifact relation `P_C` derived from child-plan continuity |
| `HistoryEdge` in `Tree.opened_from_edges` | `ploke_tree::graph::artifact_tree::Tree` | relation | sealed History opened-from context `P_O` |
| `AppliedPatchEdge` in `Tree.applied_patch_edges` | `ploke_tree::graph::artifact_tree::Tree` | provenance relation | observed applied-patch derivation `P_B`; kept for drilldown and diagnostics, not default visible geometry |
| `RunForest` parent/child topology | `Graph.forest` | projection-only / process provenance | not an artifact relation; rejected for default artifact canvas |
| node coloring or selection marks | `ploke-egui` render state | render projection | not relation carriers; must not replace an edge family |

## 3. Minting Sites

- `P_H`
  - persisted source: `ploke_records::history::SealedBlockRecord.state.header.active_artifact`
    and `selected_successor.artifact`
  - graph ingestion: `ploke_tree::graph::artifact_tree::Tree::from_graph`
- `P_C`
  - persisted source: `ploke_records::child_plan::ChildPlanRecord.parent_node_id`
    plus `ChildPlanChildRecord` derived artifact ids, joined through graph-owned
    parent-child continuity
  - graph ingestion: `ploke_tree::graph::artifact_tree::Tree::from_graph`
- `P_O`
  - persisted source: `ploke_records::history::SealedBlockRecord.state.header.opened_from_artifact`
    and `active_artifact`
  - graph ingestion: `ploke_tree::graph::artifact_tree::Tree::from_graph`
- `P_B`
  - persisted source: candidate/evaluation branch records carrying
    `base_artifact_id` and `derived_artifact_id`
  - graph ingestion: `ploke_tree::graph::artifact_tree::Tree::from_graph`

## 4. Chosen Primary Carrier

For default canvas rendering, the primary carriers are the typed relation
families already exposed on `Tree<'g>`:

- `history_successors`
- `produced_child_edges`

`ploke-egui` should render visible default geometry from those borrowed
relation families directly.

`opened_from_edges` and `applied_patch_edges` remain graph-owned relation
families, but they are not part of the default visible edge set.

## 5. Rejected Alternatives

- `RunForest` parent/child edges:
  process topology, not artifact relations
- node color substitution for `P_O`:
  loses the edge claim and makes context look like a node property
- promoting `P_O` into the default visible edge set:
  overstates History context as artifact-topology geometry
- promoting raw `P_B` into the default visible edge set:
  turns generation-target/base provenance into the apparent artifact-parent
  spine, which splits selected-child -> next-parent continuity into separate
  nodes
- ad hoc CLI/debug edge reconstruction:
  projection-only and not authoritative for the canvas

## 6. Upstream Types / Fields

- `ploke_records::history::SealedBlockRecord`
  - `state.header.opened_from_artifact`
  - `state.header.active_artifact`
  - `state.header.selected_successor.artifact`
- candidate/evaluation branch records with:
  - `base_artifact_id`
  - `derived_artifact_id`

## 7. Graph / Read-Model Carriers

- `ploke_tree::graph::artifact_tree::Tree<'g>`
  - `history_successors: Vec<HistoryEdge<'g>>`
  - `produced_child_edges: Vec<ProducedChildEdge<'g>>`
  - `opened_from_edges: Vec<HistoryEdge<'g>>`
  - `applied_patch_edges: Vec<AppliedPatchEdge<'g>>`

## 8. Downstream UI Consumers

- `crates/ploke-egui/src/ui/view/projection.rs::project_artifact_tree`
- default view contract / artifact edge diagnostics
- CLI artifact edge reporting

## 9. Open Gaps / Caveats

- `P_O` is a History context relation, not a successor relation.
- sealed History can legitimately produce self-loop `P_H` and `P_O` relations
  when `active_artifact`, `selected_successor.artifact`, or
  `opened_from_artifact` collapse to the same displayed Artifact. The
  graph-owned relation inventory keeps those edges, but
  `crates/ploke-egui/src/ui/view/projection.rs::project_artifact_tree`
  currently skips `parent == child`, so a node backed only by self-loop History
  relations renders as a visually disconnected singleton in the default canvas.
- selected child -> next parent continuity is a separate identity-fold problem,
  not a reason to render `P_O` as default geometry.
- `P_B` is still useful provenance. The UI should not throw it away; it should
  stop treating it as the primary visible parent-child spine.
- generation-0 child plans can currently materialize child Artifact nodes
  without materializing the base Artifact that the root Parent operated on.
  `Tree::material_keys` includes History/opened/selected artifacts plus
  candidate-after and child-derived artifacts, while `parent_artifact_key`
  resolves a displayed parent only from graph-owned candidate/current-child
  continuity. When that parent lookup fails, `P_C` is absent; if the same
  unmaterialized base also blocks `P_B`, the child Artifact appears as a true
  zero-edge singleton component.
- Branch ancestry, node/process ancestry, hydration, and selection context are
  separate relation families and should not be smuggled into the default
  artifact edge set as fake derivation edges.
