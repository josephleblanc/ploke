# Artifact Relations

Status: `draft`

## 1. Target Entity / Visible Claim

The default artifact canvas renders artifact-to-artifact relations from the
borrowed `ploke_tree::graph::artifact_tree::Tree<'g>` without hiding admitted
History context edges that the same projection already exposes.

## 2. Competing Carriers

| carrier | location | semantic class | notes |
| --- | --- | --- | --- |
| `HistoryEdge` in `Tree.history_successors` | `ploke_tree::graph::artifact_tree::Tree` | relation | sealed History successor `P_H` |
| `HistoryEdge` in `Tree.opened_from_edges` | `ploke_tree::graph::artifact_tree::Tree` | relation | sealed History opened-from context `P_O` |
| `AppliedPatchEdge` in `Tree.applied_patch_edges` | `ploke_tree::graph::artifact_tree::Tree` | relation | observed applied-patch derivation `P_B` |
| `RunForest` parent/child topology | `Graph.forest` | projection-only / process provenance | not an artifact relation; rejected for default artifact canvas |
| node coloring or selection marks | `ploke-egui` render state | render projection | not relation carriers; must not replace an edge family |

## 3. Minting Sites

- `P_H`
  - persisted source: `ploke_records::history::SealedBlockRecord.state.header.active_artifact`
    and `selected_successor.artifact`
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
- `opened_from_edges`
- `applied_patch_edges`

`ploke-egui` should render from those borrowed relation families directly.

## 5. Rejected Alternatives

- `RunForest` parent/child edges:
  process topology, not artifact relations
- node color substitution for `P_O`:
  loses the edge claim and makes context look like a node property
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
  - `opened_from_edges: Vec<HistoryEdge<'g>>`
  - `applied_patch_edges: Vec<AppliedPatchEdge<'g>>`

## 8. Downstream UI Consumers

- `crates/ploke-egui/src/ui/view/projection.rs::project_artifact_tree`
- default view contract / artifact edge diagnostics
- CLI artifact edge reporting

## 9. Open Gaps / Caveats

- `P_O` is a History context relation, not a successor relation.
- Branch ancestry, node/process ancestry, hydration, and selection context are
  separate relation families and should not be smuggled into the default
  artifact edge set as fake derivation edges.
