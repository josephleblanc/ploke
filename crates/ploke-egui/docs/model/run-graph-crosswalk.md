# Run Graph Crosswalk

This note collects the record, graph, and UI vocabulary needed before
`ploke-egui` defines more graph views. It is the Pass 1-3 synthesis:

- typed source inventory;
- canonical entity/relation crosswalk;
- reconciliation points for the existing `ploke-egui` model docs.

It does not define a new source of truth. `ploke_tree::Graph` owns the graph
meaning available to this UI; `ploke-egui` renders named projections over that
graph. The UI inspects completed or live run state. It does not choose,
admit, seal, or advance the next successor.

## Source Chain

```text
ploke-eval logs / records / reports
  -> typed records or typed report projections
  -> RunRecordSet
  -> ploke_tree::Graph
  -> named graph projection
  -> ploke-egui render payloads
  -> CLI/text diagnostics of the same projection
```

Owned persisted data enters through named Rust records or named typed report
projections before it becomes graph material. `ploke-egui` must not recover
graph semantics from raw JSON, rendered CLI text, copied labels, or widget
payload strings.

## Typed Source Families

| Family | Typed source | Graph treatment |
|---|---|---|
| History blocks | `SealedBlockRecord`, `SealedBlockHeaderRecord`, `AdmittedEntryRecord` | admitted History facts, block/entry facts, selected successor, artifact refs |
| Selection payloads | `SelectionDecisionEntryRecord`, `EvaluationPayloadRecord`, `CandidateSetRecord` | considered Artifact set, selected Artifact-bearing record, memberships, Artifact source refs |
| Edit-surface records | `SurfaceEvidenceRecord`, `CandidateArtifactRecord`, `SurfaceAttemptRecord` | applied-patch source refs, patch id, base/after Artifact refs |
| Branch registry | `Prototype1BranchRegistry`, `InterventionSourceNode`, `TreatmentBranchNode` | passive branch source refs and applied-patch Artifact edge hints |
| Runtime/operation ids | `RuntimeId`, `Coordinate`, `OperationTarget` | runtime-target operation facts distinct from History admission |
| Evaluation/protocol/agent-turn observations | typed passive records and report-derived projections | source/projection attachments unless promoted by a graph-owned relation |

Rendered projection/debug files are not graph sources. Typed reports or report
projections can be upstream input to `Graph`; rendered output can only be
checked against `ploke_tree::Graph` projections.

Known typed-persistence gaps:

- `CandidateArtifactRecord` does not carry every eval-side Artifact surface
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
CTX_G = artifact context facts: branches, candidates, memberships, selections
        in G.candidates/G.selections
SRC_G = source/projection attachments in G.evidence
WARN_G = graph warnings in G.warnings
```

For UI product views:

```text
A = resolved artifact identities derived from A_G
D = H_G union R_G union O_G union CTX_G union SRC_G union WARN_G
```

`A` is the default canvas node set. `D` is drilldown/debug material unless a
specific non-default graph projection says otherwise.

`G.artifacts` may store `ArtifactKey::HistoryRef { value }` and
`ArtifactKey::PassiveId { value }` as separate source keys. The graph-owned
`ArtifactTree(G)` projection resolves the common Artifact identity case where a
History ref stores `artifact:<id>` and a passive artifact id stores `<id>`.

## Artifact Node Contract

The default product graph has one material node kind:

```text
node(A(a)) = Artifact state identified by canonical artifact id a
```

Other loaded records can contribute context, filters, marks, edge provenance,
or drilldown material for `A(a)`. They do not become separate material nodes in
`ArtifactTree`.

Artifact node identity:

```text
A = quotient(A_G, artifact_key_equivalence)

artifact_key_equivalence:
  ArtifactRefRecord("artifact:<id>") == ArtifactId("<id>")
```

The default artifact view also applies a promotion-continuity quotient when the
graph can prove that a selected child Artifact later became the next Parent
checkout:

```text
selected child ArtifactId == later next-parent base ArtifactId
```

This rule only merges source references or promotion-continuity aliases that
denote the same displayed Artifact in this view. Distinct artifact ids still
require explicit edges in one of the relation sets below.

Branch, candidate, membership, and selection records do not create a second
material entity when they carry an artifact id. They say that an Artifact is
being considered, evaluated, selected, or explained in a particular context.
The Artifact remains the primary entity.

```text
if artifact context fact k refers to artifact id a:
  A(a) may be filtered as considered/evaluated/selected
  k may contribute provenance to A(a) or to an edge incident on A(a)
  k does not create a rendered ArtifactTree node
```

This keeps artifact identity independent of where the artifact is observed:
History, branch/candidate sources, scheduler records, runtime handoff
sources, and debug/provenance records all point back to the same `A(a)` when
they refer to the same artifact id.

## Artifact Context Vocabulary

These record families can annotate, filter, explain, or connect artifact nodes.
They are not additional default `ArtifactTree` material nodes.

| Context | Identity key | Artifact effect | Default ArtifactTree use |
|---|---|---|---|
| History records | `BlockHash`, `EntryId` | Mark artifacts as History-admitted or lineage-associated; supply `P_H` source refs. | Source for `P_H`, lineage marks, ruler highlight, and diagnostics. |
| Artifact context records | branch id, `CandidateId`, membership id, selection entry id | Mark Artifacts as considered, evaluated, selected, or branch-produced; may supply applied-patch source refs. | Source for context/provenance such as `P_B` when base and derived Artifact ids are present; can support filters such as “considered Artifacts” or “selected Artifacts.” |
| Runtime records | `RuntimeId` | Explain which runtime/Parent/Child/Successor source refers to an artifact. | Drilldown or non-default runtime/artifact views. |
| Operation records | `Coordinate` or `OperationTarget` when graph-owned | Explain which operation targeted or produced source facts about an artifact. | Drilldown or non-default operation views. |
| Source/projection records | source locator or graph attachment id | Support artifact facts and relation facts for inspection. | Detail panels and diagnostics. |
| Warning records | graph warning identity/order | Flag graph-build or import concerns affecting artifact interpretation. | Diagnostics. |
| Synthetic display context | UI-generated key | No source artifact fact. | Not part of `ArtifactTree`; must not be used to make artifacts appear connected. |

## Relation Vocabulary

Relations must be named by the semantic fact they carry, not by the widget edge
that happens to render them.

```text
history_successor(h, a_parent, a_child)
  h in H_G, a_parent in A, a_child in A
  source: HistoryBlockNode.active_artifact -> selected_successor.artifact
  status: admitted History successor relation

history_opened_from(h, a_opened, a_active)
  h in H_G, a_opened in A, a_active in A
  source: HistoryBlockNode.opened_from_artifact and active_artifact
  status: sealed History context relation
  use: lineage context/reveal, not automatically the same as successor edge

applied_patch_edge(k, a_base, a_child)
  k in CTX_G, a_base in A, a_child in A
  source: CandidateBranchNode.base_artifact_id -> derived_artifact_id
  status: branch/candidate-sourced applied-patch relation; not by itself a
          History successor relation

operation_targets(o, r, a)
  o in O_G, r in R_G, a in A
  source: Coordinate { runtime_id, target: OperationTarget::Artifact { artifact_id } }
  status: operation fact, not an artifact edge

artifact_hydrates(a, r)
  a in A, r in R_G
  source: selected successor/runtime handoff source when graph-owned
  status: runtime/handoff relation, not an artifact edge
```

The default artifact graph should not invent edges outside these relation
families. If a visual edge has no graph-owned relation, it is a layout aid or a
debug annotation, not a semantic edge.

A candidate Artifact can later be selected and admitted by History. In that
case the same Artifact node may have both consideration context and History
context, and the same artifact pair may be supported by both `P_B` and `P_H`.
That overlap should be represented as multiple relation labels/source refs
over Artifact nodes, not as separate Candidate nodes or as a claim that every
branch/candidate edge is already History-admitted.

## Edge Vocabulary

These are the relation sets that may become drawn edges. A relation can be
loaded in `G` without being part of the default `ArtifactTree`.

| Set | Edge | Endpoint keys | Source records / fields | Source / relation status | Default ArtifactTree status |
|---|---|---|---|---|---|
| `P_H` | History successor | `A -> A` | `HistoryBlockNode.active_artifact -> HistoryBlockNode.selected_successor.artifact` | Admitted History successor relation. | Rendered. Implemented. Self-loops are allowed but do not connect components. |
| `P_C` | Parent-produced child edge | `A -> A` | graph-owned child-plan continuity over `ChildPlanRecord.parent_node_id` and `ChildPlanChildRecord` derived artifact ids | Source fact that a displayed parent Artifact produced a displayed child Artifact in the next generation. | Rendered. Implemented. |
| `P_B` | Applied-patch edge observed through branch/candidate records | `A -> A` | `CandidateBranchNode.base_artifact_id -> CandidateBranchNode.derived_artifact_id` | Source fact that a derived Artifact was produced from a base Artifact. It may overlap a selected/admitted successor, but does not imply that by itself. | Not rendered by default. Kept as relation inventory, inspector context, and diagnostics. |
| `P_O` | History opened-from context | `A -> A` | `HistoryBlockNode.opened_from_artifact -> HistoryBlockNode.active_artifact` | Sealed History context relation. | Not rendered by default. Kept as relation inventory, inspector context, and diagnostics. |
| `B_PARENT` | Branch ancestry | branch id -> branch id | `parent_branch_id`, `source_state_id`, branch registry/scheduler branch facts | Branch/process ancestry, not an artifact edge by itself. | Not rendered by default. Cannot connect artifacts unless a graph-owned projection maps branch ancestry to artifact endpoints. |
| `N_PARENT` | Node/process ancestry | node id -> node id | `NodeRecord.parent_node_id -> NodeRecord.node_id` | Scheduler/process ancestry, not an artifact edge by itself. | Not rendered by default. |
| `SELECTS` | Selection chooses an Artifact-bearing successor | History/selection fact -> artifact-consideration fact | selection decision payload, `output_refs`, selected candidate/member fields | Selection/admission context. | Not rendered by default as artifact edge. May explain why an Artifact was chosen. |
| `OP_TARGETS` | Operation targets artifact/runtime | `O -> A` or `R -> O` depending projection | `Coordinate`, `OperationTarget`, tool/operation records | Operation fact, not an artifact edge. | Not rendered by default. |
| `HYDRATES` | Artifact hydrates runtime / runtime materializes artifact | `A <-> R` direction must be defined by projection | selected successor runtime refs and handoff sources when graph-owned | Runtime/handoff relation, not an artifact edge. | Not rendered by default. |
| `SOURCE_FOR` | Source/projection record supports entity/relation | `SRC -> A/H/C/R/O` | graph source/projection attachments | Inspection/provenance relation. | Not rendered by default. |

Default artifact edge set:

```text
E_artifact = P_H union P_C
```

Non-default relations such as `B_PARENT` can explain why two artifacts are in
the same execution family, but they do not by themselves prove that one
artifact derives from another. To use them in an artifact view, define a
separate graph-owned relation that names the projection, its endpoints, and its
source requirements.

## Existing Browser Precedent

The earlier `ploke_tree::browser` / `ploke-tree-egui` work has an owned
serializable execution graph model with runtime, artifact, operation, patch,
evaluation, selection, and handoff node/edge kinds. That model is useful prior
art for relation names such as runtime-executes-operation,
operation-produces-patch, patch-derives-artifact, and
artifact-hydrates-runtime.

It is not the source of truth for the `ploke-egui` default canvas. Treat it as
a question inventory and renderer-neutral projection precedent, not as a
replacement for a borrowed `ploke_tree::Graph` projection.

## Product Projections

### ArtifactTree

```text
N_artifact = A
E_artifact = P_H union P_C

P_H = { (a_parent, a_child) | exists h. history_successor(h, a_parent, a_child) }
P_C = { (a_parent, a_child) | exists c. produced_child_edge(c, a_parent, a_child) }
```

Properties:

- `P_H` and `P_C` both connect artifact nodes, but they come from different
  relation sources.
- `P_H` is History-admitted successor flow.
- `P_C` is graph-owned produced-child Artifact flow derived from child-plan
  continuity.
- `P_B` remains applied-patch Artifact provenance observed through
  branch/candidate records.
- `P_O` and `P_B` remain available as graph-owned context relations, but they
  are not part of the default visible canvas edge set.
- History blocks, entries, runtimes, artifact-consideration facts, operations,
  source/projection attachments, warnings, and synthetic anchors are not
  material nodes in this projection.
- Weak connectedness is not required for all future runs. It is a diagnostic
  property of the loaded relation set.

Current graph-owned computation:

```text
A       = artifact nodes after display-key equivalence
P_H     = HistoryBlockNode.active_artifact -> selected_successor.artifact
P_C     = parent Artifact -> child Artifact via child-plan continuity
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
N_debug = A_G union H_G union R_G union O_G union CTX_G union SRC_G union WARN_G
E_debug = typed record references and graph-owned source/projection attachments
```

This is an explicit debug/drilldown family. It must not leak into
`ArtifactTree`.

## Reconciliation With Existing Docs

- `view-set-contract.md` correctly states that `ArtifactTree` renders artifact
  nodes with `P_H union P_C` edges. It should treat this file as the source for
  what `P_H`, `P_C`, and contextual `P_B` mean.
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
ArtifactTree(G) = (A, P_H, P_C, diagnostics)
```

where `A`, `P_H`, and `P_C` refer back to graph-owned records or indices, and
`P_B` remains graph-owned context/provenance inventory. That projection is the
object `ploke-egui` and CLI diagnostics should consume.

The projection should remain the owner of endpoint-missing diagnostics,
artifact identity collision behavior, and relation source classes without
allocating semantic record mirrors into `ploke-egui`.
