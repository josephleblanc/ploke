# Artifact Lineage Highlight

Status: `draft`

## 1. Target Entity / Visible Claim

The default artifact canvas may visually emphasize:

- Artifacts that belong to the graph-owned primary History lineage.
- Edges that belong to that primary History lineage.
- The latest selected successor Artifact on that lineage, rendered as the
  current ruler candidate.

The glow, halo, pulse, color, and stroke treatment are render projections. They
do not choose the lineage or ruler.

## 2. Competing Carriers

| carrier | where it appears | semantic class | notes |
| --- | --- | --- | --- |
| `SealedBlockHeaderRecord.selected_successor.artifact` | `crates/ploke-records/src/history.rs::SealedBlockHeaderRecord` | authority/admission | persisted sealed History successor named by one block |
| `HistoryBlockNode.selected_successor.artifact` | `crates/ploke-tree/src/graph/types/history.rs::HistoryBlockNode` | authority/admission | graph-ingested sealed successor used by artifact-tree relation folding |
| `LineageNode.blocks` | `crates/ploke-tree/src/graph/types/history.rs::LineageNode` | authority/admission | ordered History blocks for one lineage coordinate |
| `artifact_tree::Marks::{primary_lineage,lineage_artifacts,lineage_edges,selected_ruler}` | `crates/ploke-tree/src/graph/artifact_tree.rs::Marks` | graph read-model mark | chosen graph-owned carrier for default canvas highlight membership |
| `GraphNode::Artifact.effect` | `crates/ploke-egui/src/ui/view/projection.rs::GraphNode` | render projection | UI-only visual effect flag derived from `Marks`; not source truth |
| `GraphEdgePayload.effect` | `crates/ploke-egui/src/ui/view/projection.rs::GraphEdgePayload` | render projection | UI-only visual effect flag derived from `Marks`; not source truth |
| node color / selected edge state | `crates/ploke-egui/src/ui/view/{node.rs,edge.rs}` | render projection | can emphasize an item, but must not identify the lineage or ruler by itself |

## 3. Minting Sites

- Persisted sealed History blocks store the selected successor at
  `SealedBlockHeaderRecord.selected_successor`.
- `crates/ploke-tree/src/graph/build/history.rs::Builder::ingest_history`
  observes `header.selected_successor.artifact` and stores it in
  `HistoryBlockNode.selected_successor`.
- `crates/ploke-tree/src/graph/artifact_tree.rs::primary_lineage` chooses the
  graph-owned primary lineage from loaded `LineageNode` blocks.
- `crates/ploke-tree/src/graph/artifact_tree.rs::Marks::from_graph` folds that
  lineage into `lineage_artifacts`, `lineage_edges`, and `selected_ruler`.
- `crates/ploke-egui/src/ui/view/projection.rs::project_artifact_tree` maps
  those marks to render-only node and edge effects.

## 4. Chosen Primary Carrier

The primary carrier for the visible highlight is:

`ploke_tree::graph::artifact_tree::Marks::{primary_lineage,lineage_artifacts,lineage_edges,selected_ruler}`

Those marks are already computed from graph-owned History blocks and relation
families before `ploke-egui` creates widget payloads.

## 5. Rejected Alternatives

- `GraphNode::Artifact.color`
  - useful for rendering, but not a lineage or ruler authority.
- `GraphNode::Artifact.label`
  - compact display text only.
- `egui_graphs` selected or hovered state
  - local interaction state, not the current ruler candidate.
- `RunForest` node topology
  - process topology, not sealed History lineage authority.
- Reconstructing lineage from raw persisted JSON in `ploke-egui`
  - duplicates the graph read model and bypasses typed ingestion.

## 6. Upstream Types / Fields

- `ploke_records::history::SealedBlockHeaderRecord`
  - `selected_successor`
  - `active_artifact`
  - `common.lineage_id`
  - `common.block_height`
- `ploke_records::history::SuccessorRefRecord`
  - `artifact`
  - `runtime`

## 7. Graph / Read-Model Carriers

- `ploke_tree::graph::HistoryBlockNode`
  - `lineage_id`
  - `block_height`
  - `active_artifact`
  - `selected_successor`
- `ploke_tree::graph::LineageNode`
  - `lineage_id`
  - `blocks`
- `ploke_tree::graph::artifact_tree::Marks`
  - `primary_lineage`
  - `lineage_artifacts`
  - `lineage_edges`
  - `selected_ruler`

## 8. Downstream UI Consumers

- `crates/ploke-egui/src/ui/view/projection.rs::project_artifact_tree`
- `crates/ploke-egui/src/ui/view/node.rs::GraphNodeShape`
- `crates/ploke-egui/src/ui/view/edge.rs::GraphEdgeShape`
- default view contract ruler-highlight diagnostics

## 9. Open Gaps / Caveats

- `primary_lineage` is currently a graph read-model heuristic over loaded
  lineages: highest block count, then highest block height.
- The highlight is not a successor-selection decision. It only renders marks
  already present on the borrowed artifact-tree projection.
- Render effects are intentionally not persisted and should not appear in
  `ploke_tree::Graph`.
