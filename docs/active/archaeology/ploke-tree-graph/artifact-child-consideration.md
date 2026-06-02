# `artifact-child-consideration`

## Claim

The default artifact tree should dim, dot, and optionally hide only those
produced child Artifacts that never entered the current-generation selection
process.

Children that were evaluated and rejected remain ordinary visible siblings:

- solid `P_C` edges,
- undimmed nodes,
- eligible for the same patch/evidence drilldown as the selected child.

## Chosen Primary Carrier

`ploke_tree::graph::artifact_tree::Marks::{unconsidered_children, unconsidered_child_edges}`

Those marks are computed by joining:

- `ProducedChildEdge.sources: &ChildPlanChildRecord`
- current-generation `CandidateNode` records

The primary predicate is whether a produced child appears in the sealed
selection entry as a `CandidateNode` whose `source == CurrentGeneration`.

## Carrier Table

| question | chosen carrier | where it appears |
| --- | --- | --- |
| Which child Artifacts exist in a parent-owned child plan? | `ploke_records::child_plan::ChildPlanChildRecord` | `crates/ploke-records/src/child_plan.rs`, `crates/ploke-tree/src/graph/artifact_tree.rs` |
| Which artifacts entered the current-generation selection process? | `ploke_tree::graph::CandidateNode { source: Some(CurrentGeneration), node_id, artifact_after }` | `crates/ploke-tree/src/graph/types/selection.rs`, `crates/ploke-tree/src/graph/build/selection.rs`, `crates/ploke-tree/src/graph/artifact_tree.rs` |
| Which relation is rendered as the child edge in the default view? | `ploke_tree::graph::artifact_tree::ProducedChildEdge` | `crates/ploke-tree/src/graph/artifact_tree.rs`, `crates/ploke-egui/src/ui/view/projection.rs` |
| Which UI control hides excluded children? | `ploke_egui::ui::view::ArtifactTreeFilters.hide_unconsidered_children` | `crates/ploke-egui/src/ui/view/mod.rs`, `crates/ploke-egui/src/ui/app/mod.rs` |

## Read Path

1. `ChildPlanRecord` is loaded into `Graph.child_plans.plans`.
2. Sealed selection entries are loaded into `Graph.candidates.candidates`.
3. `artifact_tree::considered_children` collects current-generation candidate
   node ids and artifact ids from those candidate records.
4. `artifact_tree::Marks::from_graph` classifies each visible
   `ProducedChildEdge` as unconsidered only when none of its child-plan sources
   match the current-generation candidate set.
5. `ploke-egui` renders those marks as dimmed nodes, dotted `P_C` edges, and a
   left-panel hide filter.

## Why Rejected Evaluated Children Must Stay Solid

The motivating floating artifacts `95e4ee12...` and `389091dd...` were:

- patch-generated,
- built,
- committed,
- evaluated,
- rejected during selection.

They did enter the selection process, so they are not “unconsidered” children.
They were previously disconnected only because the root parent artifact endpoint
was not materialized into the default artifact tree.

## Caveats

- These marks do not encode acceptance vs rejection. They encode only whether a
  produced child entered current-generation consideration.
- Historical traversal candidates are not primary for this claim. The filter is
  about a parent’s own child-production cycle, not later cross-generation
  reconsideration.
