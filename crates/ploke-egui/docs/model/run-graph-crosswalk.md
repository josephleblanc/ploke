# Run Graph Crosswalk

This note collects the record, graph, and UI vocabulary needed before
`ploke-egui` defines more graph views. It is the Pass 1-3 synthesis:

- typed source inventory;
- canonical entity/relation crosswalk;
- reconciliation points for the existing `ploke-egui` model docs.

It does not define a new source of truth. `ploke_tree::Graph` owns semantic
meaning; `ploke-egui` renders named projections over that graph.

## Source Chain

```text
typed records
  -> RunRecordSet
  -> ploke_tree::Graph
  -> named graph projection
  -> ploke-egui render payloads
  -> CLI/text diagnostics of the same projection
```

Owned persisted data enters through named Rust records. `ploke-egui` must not
recover graph semantics from raw JSON, rendered CLI text, copied labels, or
widget payload strings.

## Typed Source Families

| Family | Typed source | Graph treatment |
|---|---|---|
| History blocks | `SealedBlockRecord`, `SealedBlockHeaderRecord`, `AdmittedEntryRecord` | authority spine, block/entry facts, selected successor, artifact refs |
| Selection payloads | `SelectionDecisionEntryRecord`, `EvaluationPayloadRecord`, `CandidateSetRecord` | candidate universe, selected candidate, memberships, candidate artifact evidence |
| Edit-surface evidence | `SurfaceEvidenceRecord`, `CandidateArtifactRecord`, `SurfaceAttemptRecord` | candidate artifact derivation evidence, patch id, base/after artifact refs |
| Branch registry | `Prototype1BranchRegistry`, `InterventionSourceNode`, `TreatmentBranchNode` | passive branch evidence and candidate derivation hints |
| Runtime/operation ids | `RuntimeId`, `Coordinate`, `OperationTarget` | runtime-target operation facts distinct from History admission |
| Evaluation/protocol/agent-turn evidence | typed passive records in `ploke-records` | evidence attachments unless promoted by a graph-owned relation |

Projection/debug files are not graph sources. They may only be checked against
`ploke_tree::Graph` projections.

Known typed-persistence gaps:

- `CandidateArtifactRecord` does not carry every eval-side artifact surface
  detail currently available during History construction.
- `SurfaceEvidenceRecord` is the passive checked-surface evidence shape; live
  grant/check carriers remain in `ploke-eval`.
- `history_preview::Document` has a degraded `serde_json::Value` catalog path.
  That path is not a source for graph semantics.

## Entity Sets

Let `G = ploke_tree::Graph`.

```text
A_G = artifacts observed in G.artifacts
H_G = sealed History blocks and admitted entries in G.history
R_G = runtime identities in G.runtimes
O_G = operations in G.operations
C_G = candidates, branches, memberships, and selections in G.candidates/G.selections
EVID_G = evidence attachments in G.evidence
WARN_G = graph warnings in G.warnings
```

For UI product views:

```text
A = resolved artifact identities derived from A_G
D = H_G union R_G union O_G union C_G union EVID_G union WARN_G
```

`A` is the product artifact node set. `D` is drilldown/debug material unless a
specific non-default graph projection says otherwise.

Current gap: `G.artifacts` stores `ArtifactKey::HistoryRef { value }` and
`ArtifactKey::PassiveId { value }` as distinct keys. The UI needs artifact
identity equivalence, but that equivalence should be exposed by `ploke-tree`,
not rediscovered by `ploke-egui` string handling.

Current implementation note: `ploke-egui` resolves the common observed case
where a History ref stores `artifact:<id>` and a passive artifact id stores
`<id>`. That is a presentation-side repair until `ploke-tree` exposes the
canonical artifact identity relation.

## Relation Vocabulary

Relations must be named by the semantic fact they carry, not by the widget edge
that happens to render them.

```text
history_successor(h, a_parent, a_child)
  h in H_G, a_parent in A, a_child in A
  source: HistoryBlockNode.active_artifact -> selected_successor.artifact
  authority: sealed History block

history_opened_from(h, a_opened, a_active)
  h in H_G, a_opened in A, a_active in A
  source: HistoryBlockNode.opened_from_artifact and active_artifact
  authority: sealed History block
  use: lineage context/reveal, not automatically the same as successor edge

candidate_derivation(c, a_base, a_child)
  c in C_G, a_base in A, a_child in A
  source: CandidateBranchNode.base_artifact_id -> derived_artifact_id
  authority: candidate/evidence, not History admission

operation_targets(o, r, a)
  o in O_G, r in R_G, a in A
  source: Coordinate { runtime_id, target: OperationTarget::Artifact { artifact_id } }
  authority: operation fact, not artifact derivation

artifact_hydrates(a, r)
  a in A, r in R_G
  source: selected successor/runtime handoff evidence when graph-owned
  authority: runtime/handoff relation, not patch derivation
```

The default artifact graph should not invent edges outside these relation
families. If a visual edge has no graph-owned relation, it is a layout aid or a
debug annotation, not a semantic edge.

## Existing Browser Precedent

`ploke_tree::browser` already has an owned serializable execution graph model
with runtime, artifact, operation, patch, evaluation, selection, and handoff
node/edge kinds. That model is useful prior art for relation names such as
runtime-executes-operation, operation-produces-patch, patch-derives-artifact,
and artifact-hydrates-runtime.

It is not the source of truth for the `ploke-egui` default canvas. Treat it as
a renderer-neutral projection precedent, not as a replacement for a borrowed
`ploke_tree::Graph` projection.

## Product Projections

### ArtifactTree

```text
N_artifact = A
E_artifact = P_H union P_B

P_H = { (a_parent, a_child) | exists h. history_successor(h, a_parent, a_child) }
P_B = { (a_base, a_child) | exists c. candidate_derivation(c, a_base, a_child) }
```

Properties:

- `P_H` and `P_B` both connect artifact nodes, but they have different
  authority classes.
- `P_H` is History-admitted successor flow.
- `P_B` is candidate/evidence derivation flow.
- History blocks, entries, runtimes, candidates, selections, operations,
  evidence, warnings, and synthetic anchors are not nodes in this projection.
- Weak connectedness is not required for all future runs. It is a diagnostic
  property of the loaded relation set.

Implementation gap: `ploke-egui` currently constructs the artifact projection
from `G` directly. The canonical relation fold should live in `ploke-tree` and
be borrowed by `ploke-egui`.

Current egui-local computation:

```text
A       = artifact nodes after display-key equivalence
P_H     = HistoryBlockNode.active_artifact -> selected_successor.artifact
P_B     = CandidateBranchNode.base_artifact_id -> derived_artifact_id
lineage = loaded lineage maximizing (block_count, max_block_height)
ruler   = selected_successor artifact from max-height block in lineage
```

This describes the current implementation, not the desired ownership boundary.

### RuntimeArtifactGraph

```text
N_runtime_artifact = A union R_G union O_G
E_runtime_artifact =
  operation_targets
  union artifact_hydrates
  union P_H
  union P_B
```

This graph answers runtime/operation questions. It is not the default canvas.

### DebugRecordGraph

```text
N_debug = A_G union H_G union R_G union O_G union C_G union EVID_G union WARN_G
E_debug = typed record references and graph-owned evidence attachments
```

This is an explicit debug/drilldown family. It must not leak into
`ArtifactTree`.

## Reconciliation With Existing Docs

- `view-set-contract.md` correctly states that `ArtifactTree` renders artifact
  nodes with `P_H union P_B` edges. It should treat this file as the source for
  what `P_H` and `P_B` mean.
- `default-view-contract.md` correctly requires the center canvas to default to
  `ArtifactTree`, but its CLI-testable signals should eventually report
  relation classes from a `ploke-tree` projection, not egui-local inference.
- `graph-pipeline.md` correctly says `ploke_tree::Graph` is semantic source of
  truth. The missing layer is a graph-owned projection between `Graph` and
  egui `RawGraph`.
- `artifact-tree-default/README.md` correctly rejects full record/debug graph
  rendering in the default view. This crosswalk names the allowed artifact-edge
  relation families.

## Next Canonicalization Step

Add a borrowed projection in `ploke-tree` that exposes:

```text
ArtifactTree(G) = (A, P_H, P_B, diagnostics)
```

where `A`, `P_H`, and `P_B` refer back to graph-owned records or indices. That
projection is the object `ploke-egui` and CLI diagnostics should consume.

The projection should define endpoint-missing diagnostics, artifact identity
collision behavior, and relation authority classes without allocating semantic
record mirrors into `ploke-egui`.
