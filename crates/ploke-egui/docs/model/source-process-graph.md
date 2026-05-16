# Source Process Graph

This note explains what the `ploke-egui` graph is trying to represent, where
the source facts come from, and how those facts move from `ploke-eval` through
typed records into `ploke_tree::Graph`.

It is intentionally descriptive. It should help an operator or implementer ask
"what does this graph mean?" before adding more widgets.

## Short Answer

`ploke-egui` is a read-only inspection UI for a Prototype 1 run.

The graph is not the run itself. The run happens in `ploke-eval`. The UI loads
typed persisted records, folds them into `ploke_tree::Graph`, and renders named
projections over that immutable graph.

```text
ploke-eval runtime process
  -> ploke-records typed persisted records
  -> ploke-tree RunRecordSet
  -> ploke_tree::Graph
  -> ploke-egui graph views and inspectors
```

There is no `crates/ploke-report` crate in this checkout. The passive report
and record schemas used by this path live primarily in `ploke-records`; report-
like projections become graph input only after they have a named Rust type.

## Process Being Represented

Prototype 1 is a self-improvement loop over artifacts and runtimes:

```text
Artifact A1 hydrates Runtime R1
R1 acts as Parent for one History/Crown epoch
R1 produces or admits candidate child Artifacts A2, A3, ...
child runtimes evaluate those candidate Artifacts
selection chooses one successor Artifact
the selected Artifact is installed into the active checkout
a successor Runtime hydrates from that Artifact and becomes the next Parent
History seals the epoch boundary and records the selected successor
```

The important distinction:

- `Artifact` is a recoverable checkout state.
- `Runtime` is an executing process hydrated from an Artifact.
- `Parent` is a runtime authority state, not a permanent path.
- `Candidate` is an artifact being considered in a selection context.
- `History` is sealed authority/order material, not the scheduler snapshot.
- Scheduler/run-forest records are useful process projections, not History
  authority.

The UI should explain how these pieces relate. It should not decide, admit,
seal, or advance anything.

## Source Types

`ploke-records` owns passive schemas. Deserializing one of these types means
the bytes had a known shape; it does not grant authority or validate the live
transition.

| Source family | Important types | Produced by | Meaning for the graph |
|---|---|---|---|
| Scheduler/run nodes | `scheduler::SchedulerStateRecord`, `scheduler::NodeRecord` | `ploke-eval/src/intervention/scheduler.rs` via node/scheduler projection writes | Parent/child run topology, generations, candidate ids, branch ids, base/derived artifact ids, status |
| Run identity/handoff | `identity::ParentIdentityRecord`, `invocation::SuccessorReadyRecord`, `invocation::SuccessorCompletionRecord` | `ploke-eval/src/cli/prototype1_state/parent.rs` and `invocation.rs` | Runtime handoff and successor readiness/completion facts |
| History blocks | `history::SealedBlockRecord`, `SealedBlockHeaderRecord`, `AdmittedEntryRecord` | `ploke-eval/src/cli/prototype1_state/history.rs` and sealing paths | Sealed block order, active artifact, selected successor artifact/runtime, admitted entries |
| Selection payloads | `SelectionDecisionEntryRecord`, `EvaluationPayloadRecord`, `CandidateSetRecord` | `cli_facing.rs` current-generation/traversal selection paths | Which candidates were considered, which membership was selected, selected source, decision outcome |
| Candidate artifact payload | `CandidateArtifactRecord` | `cli_facing.rs::candidate_artifact_from_outcome` and History sealing paths | The same artifact considered as a candidate, including node, resolved branch, and optional surface evidence |
| Edit surface | `SurfaceEvidenceRecord`, `SurfaceAttemptRecord` | edit-surface generation/checking paths | Patch/base/after facts and failed or applied surface attempts |
| Evaluation artifacts | `evaluation::Artifact` | evaluation/scoring paths and persisted evaluation files | Branch disposition, compared instances, evaluator/eval-set identity, metrics and reasons |
| Operation vocabulary | `ids::Coordinate`, `ids::OperationTarget`, `ArtifactId`, `RuntimeId`, `PatchId` | mirrored from `ploke-eval/src/loop_graph.rs` into passive records | Runtime-target relation: what runtime acted on which artifact/patch/artifact set |

## Source-To-Graph Fold

`ploke-tree::FsRunStore` loads a run root into `RunRecordSet`:

```text
RunRecordSet {
  forest_input: RunForestInput {
    scheduler,
    node_records,
    parent_identity,
    successor_ready,
    successor_completion,
    passive_evidence,
  },
  history_blocks,
  transition_journal,
}
```

`ploke_tree::Graph::from_records(&RunRecordSet)` then folds those records into
indexes:

| Graph index | What it holds |
|---|---|
| `Graph.forest` | scheduler/run forest assembled from scheduler and node records |
| `Graph.history` | sealed History blocks and entries |
| `Graph.authority` | lineage/block authority ordering derived from History |
| `Graph.artifacts` | recoverable artifact identities observed from History and passive evidence |
| `Graph.runtimes` | runtime identities observed from actors and passive handoff records |
| `Graph.operations` | runtime-target operation facts |
| `Graph.candidates` | candidate subjects, branches, memberships, and artifact facts |
| `Graph.selections` | sealed selection decisions and selected membership/candidate refs |
| `Graph.evidence` | typed source/projection attachments |
| `Graph.warnings` | graph-build anomalies |

This fold is the semantic boundary for the UI. `ploke-egui` should borrow from
`&Graph` or a named borrowed projection over it, not reconstruct semantics from
raw JSON, copied labels, widget-local strings, or row-shaped inspector carriers.

## Current Default Projection

The current implemented `ArtifactTree` projection has two cases. This describes
what the app is doing now; it is also the main semantic tension to resolve if
the product default is meant to be artifact-first in the stricter sense.

The intended and now restored default is:

```text
visible nodes = A = artifact identities
visible edges = P_H union P_B
```

Where:

- `P_H` is a History successor relation from active artifact to selected
  successor artifact.
- `P_B` is an applied-patch/base relation from base artifact to derived
  artifact.

The artifact tree is a borrowed projection. It may group multiple
source records into one artifact node when they resolve to the same artifact
identity, for example `ArtifactRefRecord("artifact:<id>")` and
`ArtifactId("<id>")`.

Scheduler/run facts explain how artifacts were produced or used. They are not
the primary canvas nodes unless the operator explicitly switches to a future
process/schedule view.

## What A Node Should Explain

For a selected default node, the inspector should answer:

| Question | Answer source |
|---|---|
| What did I select? | selected `TreeNode` or artifact-tree node borrowed from `Graph` |
| Which artifact facts are attached? | `base_artifact_id`, `derived_artifact_id`, `patch_id`, `Graph.artifacts` |
| Why is this connected to its neighbors? | run-forest parent/child edge, or fallback `P_H`/`P_B` edge |
| Was this artifact considered or selected? | `Graph.candidates`, `Graph.selections`, History selection payloads |
| What process produced it? | scheduler node, child outcome, candidate artifact payload, surface evidence |
| What evaluation or report data exists? | `Graph.evidence`, evaluation artifacts, sealed candidate evidence |
| What source records mention it? | evidence locators and source/projection attachments |

If a typed inspector fact cannot answer from `&Graph`, the UI should show
`missing`, `not_applicable`, or `blocked_by_missing_projection` rather than
inventing a local interpretation.

## What An Edge Should Explain

For a selected edge, the inspector should answer:

| Edge kind | Assertion | Source |
|---|---|---|
| `E_F` | child run/scheduler node was recorded with this parent node | `NodeRecord.parent_node_id` through `RunForest` |
| `P_H` | selected successor artifact followed active artifact in sealed History | `SealedBlockHeaderRecord.active_artifact` and `selected_successor.artifact` |
| `P_B` | derived artifact was produced from a base artifact by a patch/surface relation | candidate branch facts, scheduler node artifact fields, or surface evidence |

The UI should not conflate these edges. A run-forest edge explains process
topology. An artifact edge explains artifact identity lineage.

## Useful Next UI Questions

The graph becomes meaningful when it answers questions in this order:

1. What process node or artifact am I looking at?
2. What artifact identity or identities does it refer to?
3. Why is it connected to the visible parent/child nodes?
4. Was it merely considered, or was it selected/promoted?
5. What patch/surface relation produced the derived artifact?
6. What evaluation/report facts were available for the selection?
7. Which source records support each displayed fact?
8. Which expected facts are missing from the loaded graph?

These questions should drive future right-panel typed facts, edge inspection, timeline
spans, and CLI diagnostics. The graph canvas should stay simple enough to show
shape at a glance; the inspector should carry the explanation.
