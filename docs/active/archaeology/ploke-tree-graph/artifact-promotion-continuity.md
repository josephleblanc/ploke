# `artifact-promotion-continuity`

## Claim

The default artifact tree should preserve two separate continuity rules without
inventing synthetic nodes or string-only UI heuristics:

1. A selected child Artifact and the later next-parent base Artifact may be
   shown as one displayed Artifact node when graph-owned child-plan continuity
   proves they are the same checkout for this view.
2. Sibling child Artifacts from the same parent must remain attached to that
   parent Artifact even when they never join the selected History lineage.

The dimmed/dotted sibling treatment in `ploke-egui` is a view mark over this
second rule, not a new graph relation class.

## Chosen Primary Carrier

`ploke_tree::graph::artifact_tree::Tree<'g>`

This borrowed projection owns the default artifact-view node set, visible
relation families, and mark set. It is the right place to decide:

- which Artifact identities materialize into the default view,
- which parent Artifact endpoint a `P_C` relation should attach to,
- which produced children are outside the primary lineage and therefore receive
  a dimmed/dotted/filterable mark in the UI.

## Carrier Table

| question | chosen carrier | where it appears |
| --- | --- | --- |
| Which Artifact ids belong to a parent-published child plan? | `ploke_records::child_plan::ChildPlanRecord` / `ChildPlanChildRecord` | `crates/ploke-tree/src/graph/build/passive.rs`, `crates/ploke-tree/src/graph/artifact_tree.rs` |
| Which Artifact was materialized for a parent node before children were created? | `ploke_tree::graph::CandidateNode.artifact_after` | `crates/ploke-tree/src/graph/types/selection.rs`, `crates/ploke-tree/src/graph/artifact_tree.rs` |
| Which selected child continues as the next parent base Artifact? | `ploke_tree::graph::ParentCreateAttempt::{derived_artifact_id,next_parent_base_artifact_id}` | `crates/ploke-tree/src/graph/types/parent_create.rs`, `crates/ploke-tree/src/graph/artifact_tree.rs` |
| Which Artifact edges are primary visible default-view relations? | `ploke_tree::graph::artifact_tree::{HistoryEdge,ProducedChildEdge}` | `crates/ploke-tree/src/graph/artifact_tree.rs`, `crates/ploke-egui/src/ui/view/projection.rs` |
| Which produced children are outside the primary selected lineage? | `ploke_tree::graph::artifact_tree::Marks::{non_lineage_children,non_lineage_child_edges}` | `crates/ploke-tree/src/graph/artifact_tree.rs`, `crates/ploke-egui/src/ui/view/projection.rs` |

## Read Path

1. Passive evidence loading places typed child-plan records into
   `Graph.child_plans.plans`.
2. Candidate payloads and runner/invocation evidence place passive Artifact ids
   into `Graph.artifacts` and parent-materialized Artifact ids into
   `CandidateNode.artifact_after`.
3. `artifact_tree::promotion_aliases` collapses selected-child ->
   next-parent-base continuity only when a later child plan proves the same
   checkout continues as the next parent.
4. `artifact_tree::material_keys` materializes:
   - History-opened, active, and selected successor Artifacts,
   - candidate `artifact_after` Artifacts,
   - child-plan derived Artifacts,
   - parent Artifact endpoints recoverable from the producing node.
5. `artifact_tree::parent_artifact_key` resolves the displayed parent Artifact
   for each `ChildPlanRecord.parent_node_id`.
6. `artifact_tree::Marks::from_graph` classifies any visible `ProducedChildEdge`
   whose child Artifact is absent from the primary lineage as a non-lineage
   child mark.

## Why The Example Siblings Were Singleton Nodes Before

In the saved run that motivated this change, the two sibling Artifacts
`95e4ee12...` and `389091dd...` had:

- child-plan records,
- treatment runner requests/results,
- completed child runtime observations,
- persisted evaluation artifacts,

but their shared parent Artifact endpoint was not materialized into the default
artifact tree. Without that parent endpoint, the `P_C` edges could not be
emitted, so the children appeared as true zero-edge singleton components.

They were not “unevaluated children”. They were evaluated child Artifacts that
did not enter the selected History lineage.

## UI Consequence

`ploke-egui` should treat `Marks.non_lineage_children` and
`Marks.non_lineage_child_edges` as render-only emphasis/filter controls:

- child nodes are dimmed,
- their `P_C` edges are dotted,
- a left-panel filter may hide them,

while the underlying graph relation remains `ProducedChildEdge`.
