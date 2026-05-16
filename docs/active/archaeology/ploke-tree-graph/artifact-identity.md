# Artifact Identity

## 1. Target Entity / Visible Claim

The visible claim is: when the inspector identifies an Artifact, it should show
the durable ids associated with that Artifact without confusing checkout
identity, recoverable handle, and provenance coordinates.

Current UI surfaces involved:

- artifact inspector `Identity` row in
  `crates/ploke-egui/src/ui/app/shell.rs`
- run-forest identity rows that show `source artifact`, `base artifact`, and
  `derived artifact`
- future progressive-discovery `Artifact Ids` inspector section

## 2. Competing IDs and Carriers

| carrier | where it appears | semantic class | notes |
| --- | --- | --- | --- |
| `ArtifactId` | `loop_graph::ArtifactId`, `ploke_records::ids::ArtifactId`, `Prototype1NodeRecord.{base_artifact_id,derived_artifact_id}` | identity / durable handle | intended backend-neutral durable artifact identity slot |
| `ArtifactRef` / `ArtifactRefRecord` | History block boundaries, selection carrier, `ploke_tree::graph::ArtifactIdentity::HistoryRef` | handle / alias | recoverable History-facing artifact handle |
| `TreeKeyHash` | `ArtifactSurface.tree_key`, sealed History artifact claim via `ClaimsRecord.artifact`, artifact claim admission via `ArtifactLocator` | identity / stronger checkout proof | strongest current checkout-state identity; now copied onto `ploke_tree::graph::ArtifactNode.ids.tree_keys` when sealed History carries an artifact claim for the active Artifact |
| `node_id` | `Prototype1NodeRecord.node_id`, parent identity and scheduler flow | provenance | identifies the generation attempt / scheduler node, not the Artifact itself |
| `branch_id` | scheduler, selection fallback for `ArtifactRef` | fallback / provenance | branch handle is explicitly not artifact identity in the docs |
| `candidate_id` | scheduler and run-forest records | provenance | selection provenance, not artifact identity |
| `runtime_id` | runtime / invocation records | provenance | runtime provenance, not artifact identity |
| artifact-tree `Node.key` / UI `selection.label` | `ploke_tree::graph::artifact_tree::Node`, `GraphSelectionDetail` | projection-only | normalized display/grouping key, not source authority |

## 3. Minting Sites

### `ArtifactId`

- declared as a backend-neutral durable artifact-state id in
  `crates/ploke-eval/src/loop_graph.rs`
- minted at least by:
  - text-file fallback identity in
    `crates/ploke-eval/src/intervention/spec.rs`
  - git-commit-backed identity in
    `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- propagated into scheduler and node records through
  `Prototype1NodeRecord.base_artifact_id` and
  `Prototype1NodeRecord.derived_artifact_id`

### `ArtifactRef`

- minted from node record fields in
  `crates/ploke-eval/src/cli/prototype1_state/selection.rs`
- current logic prefers:
  1. `derived_artifact_id`
  2. `base_artifact_id`
  3. fallback `branch:{branch_id}`

### `TreeKeyHash`

- minted from backend clean checkout state in
  `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- carried through `ArtifactSurface.tree_key`
- persisted in sealed History as `ClaimsRecord.artifact`
- used for History artifact claim admission via `ArtifactLocator` in
  `crates/ploke-eval/src/cli/prototype1_state/history.rs`

### Provenance carriers

- `node_id`, `branch_id`, `candidate_id` are persisted on
  `Prototype1NodeRecord`
- `runtime_id` is minted at runtime startup and carried through runtime and
  invocation records

## 4. Chosen Primary Carrier

Chosen primary carrier for the Artifact inspector UI: `ArtifactId`.

Why:

- the type is explicitly named and documented as the durable artifact-state id
- it already propagates across multiple records and transitions
- it is closer to what operators will recognize and reuse than raw tree-key
  commitments
- unlike `node_id` and related coordinates, it is intended to name the Artifact
  rather than the attempt that produced it

## 5. Rejected Alternatives

### `TreeKeyHash` is not primary for this UI claim

It is the strongest current checkout-state identity, but it remains secondary
to `ArtifactId` for this UI claim, and it is likely too noisy to act as the
first displayed handle in the inspector. It should be a drilldown slot, even
now that the graph can carry it on `ArtifactNode.ids.tree_keys`.

### `ArtifactRef` is not primary

It is a useful alias and often operator-facing, but it is a History-facing
recoverable handle built from nearby ids. Current minting can also fall back to
`branch:{branch_id}`, which is not artifact identity in the doc-model sense.

### `node_id`, `branch_id`, `candidate_id`, `runtime_id` are not primary

These are provenance coordinates. They are useful for explaining how an
Artifact was selected, produced, or run, but they do not identify the Artifact
as an Artifact.

### Artifact-tree keys and labels are not primary

These are projection/grouping outputs. They are downstream of the graph facts
and must not be treated as the source authority for debugger identity claims.

## 6. Upstream Record Types and Fields

### `ploke-records`

- `ploke_records::ids::ArtifactId`
- `ploke_records::history::ArtifactRefRecord.value`
- `ploke_records::history::TreeKeyHashRecord`
- `ploke_records::history::ClaimsRecord.artifact`
- `ploke_records::history::SealedBlockRecord.state.header.common.opened_from_artifact`
- `ploke_records::history::SealedBlockRecord.state.header.active_artifact`
- `ploke_records::history::SealedBlockRecord.state.header.selected_successor.artifact`

### `ploke-eval`

- `Prototype1NodeRecord.base_artifact_id`
- `Prototype1NodeRecord.derived_artifact_id`
- `Prototype1NodeRecord.node_id`
- `Prototype1NodeRecord.branch_id`
- `selection::Artifact.artifact_ref`
- `history::ArtifactSurface.tree_key`

## 7. Graph / Read-Model Carriers

Current graph carriers:

- `ploke_tree::graph::ArtifactNode`
- `ploke_tree::graph::ArtifactIds`
- `ploke_tree::graph::ArtifactIdentity`
  - `HistoryRef(ArtifactRefRecord)`
  - `PassiveId(ArtifactId)`
- `ploke_tree::graph::OpeningAuthorityNode::Genesis { tree_key_hash, .. }`
- artifact-tree grouping node sources: one display/grouping artifact may be
  backed by multiple `ArtifactNode`s

Current UI carrier path:

- `ArtifactInspection.sources`
- `ArtifactIdentityWitness<'_, 'g>` over `&[&ArtifactNode]`

Current join behavior:

- `ArtifactNode.ids` is reconciled by the same normalized entity key the
  artifact tree uses today: strip the `artifact:` prefix when present and group
  matching values
- `ArtifactId` and `ArtifactRefRecord` are accumulated into the shared
  `ArtifactIds` bundle for every source node in that entity group
- `TreeKeyHash` is accumulated into that same bundle when sealed History
  provides `ClaimsRecord.artifact`, attached through the block's
  `active_artifact`

Important caveat:

- the graph now exposes `TreeKeyHash` on the artifact identity surface when a
  sealed artifact claim is present, but the join still depends on the current
  normalized active-artifact key path rather than on a richer first-class
  artifact entity object

## 8. Downstream UI Consumers by Crate / Module

### `ploke-egui`

- `crates/ploke-egui/src/ui/app/shell.rs`
  - `render_artifact_identity`
  - `render_run_forest_identity` artifact-related rows
- `crates/ploke-egui/src/ui/inspector.rs`
  - `ArtifactInspection`
  - `ArtifactIdentityWitness`
  - diagnostics snapshot projection for artifact identity

Planned consumer:

- a collapsed `Artifact Ids` inspector section near the bottom of the inspector
  with progressive-discovery display:
  - prefix + short hash first
  - full id on click
  - copy affordances available throughout

## 9. Open Gaps, Caveats, and Fallback Status

- `TreeKeyHash` is stronger checkout identity than `ArtifactId`, but it remains
  secondary for this UI claim even though the graph now carries it on
  `ArtifactNode.ids.tree_keys`.
- the current graph join uses normalized key reconciliation and
  `active_artifact -> ClaimsRecord.artifact` attachment. That is good enough for
  this slice, but it is not yet the final first-class artifact entity model.
- `ArtifactRef` construction currently wraps `ArtifactId` textually and can
  produce inconsistent prefixes.
- grouped artifact-tree nodes may have multiple source identities; the UI must
  not silently pick a misleading primary id if the grouped sources disagree.
- missing or conflicting `ArtifactId` values should become explicit typed UI
  state, not string fallbacks.

## 10. Implementation Guidance Unblocked by This Report

This report unblocks choosing semantic slots for a future `Artifact Ids`
inspector drilldown. It does not by itself justify the final interaction
abstraction.

Expected slot order for the next UI pass:

1. primary `ArtifactId`
2. alias `ArtifactRef` values
3. secondary `TreeKeyHash` from `ArtifactNode.ids.tree_keys`
