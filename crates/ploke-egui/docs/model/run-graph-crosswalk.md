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

`G.artifacts` may store `ArtifactKey::HistoryRef { value }` and
`ArtifactKey::PassiveId { value }` as separate source keys. The graph-owned
`ArtifactTree(G)` projection resolves the common Artifact identity case where a
History ref stores `artifact:<id>` and a passive artifact id stores `<id>`.

## Node Vocabulary

These are the graph node sets `ploke-egui` may draw. A product view chooses a
subset of these sets; the default `ArtifactTree` chooses only `A`.

| Set | Entity | Identity key | Contributing source records | Default ArtifactTree status |
|---|---|---|---|---|
| `A` | Artifact state | Canonical artifact id after wrapper normalization. `artifact:<id>` and `<id>` are the same key. | `ArtifactNode` from History artifact refs and passive `ArtifactId` observations; candidate, branch, scheduler, run-attempt, and History records may contribute evidence to the same node. | Rendered as the only default node set. Implemented by `ploke_tree::Graph::artifact_tree()`. |
| `H` | History block / admitted History entry | `BlockHash`, `EntryId` | `SealedBlockRecord`, `SealedBlockHeaderRecord`, `AdmittedEntryRecord` | Not rendered as nodes. Used as source/provenance for `P_H`, lineage marks, and diagnostics. |
| `C` | Candidate/branch participation context | `CandidateId`, branch id, membership id, selection entry id as appropriate | selection payloads, candidate sets, scheduler nodes, branch registry, evaluations | Not rendered as material nodes in default view. Used as provenance for artifact nodes, `P_B`, and candidate drilldown/debug views. |
| `R` | Runtime / Parent role instance | `RuntimeId` | History selected successor runtime refs, parent identity, handoff/ready/completion evidence | Not rendered as nodes in default view. Candidate for runtime/artifact and handoff views. |
| `O` | Operation target/action fact | `Coordinate` or `OperationTarget` when graph-owned | edit-surface, tool, operation, and evaluation records | Not rendered as nodes in default view. Candidate for operation/debug views. |
| `EVID` | Evidence attachment | `EvidenceId` plus source locator | typed passive evidence, scheduler/run-attempt files, surface evidence, evaluation/protocol/agent-turn records | Not rendered as nodes in default view. Used for detail panels and diagnostics. |
| `WARN` | Graph warning | graph warning identity/order | graph build warnings | Not rendered as nodes in default view. Used for diagnostics. |
| `SYN` | Synthetic display connector/anchor | UI-generated key | none; no semantic source record | Not rendered in default view. Must not be used to make ArtifactTree appear connected. |

Artifact node formation rule:

```text
A = quotient(A_G, artifact_key_equivalence)

artifact_key_equivalence:
  ArtifactRefRecord("artifact:<id>") == ArtifactId("<id>")
```

This rule only merges source references that denote the same artifact id. It
does not connect distinct artifacts. Distinct artifact ids require explicit
edges in one of the relation sets below.

Candidate, branch, membership, and selection records do not create a second
material entity when they carry an artifact id. They say that an existing
artifact participates in a candidate/selection context.

```text
if candidate context c refers to artifact id a:
  c contributes provenance to A(a)
  c does not create a separate artifact-like node
```

This keeps artifact identity independent of where the artifact is observed:
History, candidate evidence, branch records, scheduler records, runtime
handoff evidence, and debug/provenance records all point back to the same
`A(a)` when they refer to the same artifact id.

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

## Edge Vocabulary

These are the relation sets that may become drawn edges. A relation can be
loaded in `G` without being part of the default `ArtifactTree`.

| Set | Edge | Endpoint keys | Source records / fields | Authority | Default ArtifactTree status |
|---|---|---|---|---|---|
| `P_H` | History successor | `A -> A` | `HistoryBlockNode.active_artifact -> HistoryBlockNode.selected_successor.artifact` | Sealed History block. | Rendered. Implemented. Self-loops are allowed but do not connect components. |
| `P_B` | Candidate branch derivation | `A -> A` | `CandidateBranchNode.base_artifact_id -> CandidateBranchNode.derived_artifact_id` | Candidate/evidence relation, not History admission. | Rendered. Implemented. |
| `P_O` | History opened-from context | `A -> A` | `HistoryBlockNode.opened_from_artifact -> HistoryBlockNode.active_artifact` | Sealed History context. | Not rendered by default. May support lineage/context views if explicitly admitted into that projection. |
| `B_PARENT` | Branch ancestry | branch id -> branch id | `parent_branch_id`, `source_state_id`, branch registry/scheduler branch facts | Branch/process ancestry, not artifact derivation by itself. | Not rendered by default. Cannot connect artifacts unless a graph-owned projection maps branch ancestry to artifact endpoints. |
| `N_PARENT` | Node/process ancestry | node id -> node id | `NodeRecord.parent_node_id -> NodeRecord.node_id` | Scheduler/process ancestry, not artifact derivation by itself. | Not rendered by default. |
| `SELECTS` | Selection chooses candidate/successor | History/selection fact -> candidate/node/branch fact | selection decision payload, `output_refs`, selected candidate/member fields | Selection evidence/admission context. | Not rendered by default as artifact edge. May explain why a successor was chosen. |
| `OP_TARGETS` | Operation targets artifact/runtime | `O -> A` or `R -> O` depending projection | `Coordinate`, `OperationTarget`, tool/operation records | Operation fact, not artifact derivation. | Not rendered by default. |
| `HYDRATES` | Artifact hydrates runtime / runtime materializes artifact | `A <-> R` direction must be defined by projection | selected successor runtime refs and handoff evidence when graph-owned | Runtime/handoff relation. | Not rendered by default. |
| `EVID_FOR` | Evidence supports entity/relation | `EVID -> A/H/C/R/O` | graph evidence attachments | Evidence/provenance. | Not rendered by default. |

Default artifact edge set:

```text
E_artifact = P_H union P_B
```

Non-default relations such as `B_PARENT` can explain why two artifacts are in
the same execution family, but they do not by themselves prove that one
artifact derives from another. To use them in an artifact view, define a
separate graph-owned relation that names the projection, its endpoints, and its
authority class.

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

Current graph-owned computation:

```text
A       = artifact nodes after display-key equivalence
P_H     = HistoryBlockNode.active_artifact -> selected_successor.artifact
P_B     = CandidateBranchNode.base_artifact_id -> derived_artifact_id
lineage = loaded lineage maximizing (block_count, max_block_height)
ruler   = selected_successor artifact from max-height block in lineage
```

This fold lives in `ploke_tree::Graph::artifact_tree()` and is consumed by
`ploke-egui`.

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

## Canonical Projection Boundary

`ploke-tree` exposes a borrowed projection:

```text
ArtifactTree(G) = (A, P_H, P_B, diagnostics)
```

where `A`, `P_H`, and `P_B` refer back to graph-owned records or indices. That
projection is the object `ploke-egui` and CLI diagnostics should consume.

The projection should remain the owner of endpoint-missing diagnostics,
artifact identity collision behavior, and relation authority classes without
allocating semantic record mirrors into `ploke-egui`.
