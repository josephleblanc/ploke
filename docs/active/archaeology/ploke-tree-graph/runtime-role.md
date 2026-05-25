# Runtime Role

## 1. Target Entity / Visible Claim

The visible claim is: the inspector may show an Artifact as having been invoked
as a child runtime or as the selected successor/parent runtime.

Current UI surfaces involved:

- role badges in `crates/ploke-egui/src/ui/app/shell.rs`
- borrowed role witnesses in `crates/ploke-egui/src/ui/text/decor.rs`
- inspector projections in `crates/ploke-egui/src/ui/inspector.rs`

## 2. Competing Carriers

| carrier | where it appears | semantic class | notes |
| --- | --- | --- | --- |
| `InvocationRecord.role` plus invocation `derived_artifact_id` | `ploke_records::invocation::InvocationRecord.{role,node,request}` | primary evidence | classifies the runtime attempt and binds it to the produced Artifact id. |
| `TreeNode.derived_artifact_id` | `ploke_tree::TreeNode.derived_artifact_id` | handle / projection context | identifies the Artifact associated with a run-forest node, but does not by itself say the runtime role. |
| `ArtifactNode.ids.artifact_ids` | `ploke_tree::graph::ArtifactNode.ids.artifact_ids` | identity / handle | identifies graph Artifacts that may be matched to invocation-derived Artifact ids. |
| `runtime_id` | `InvocationRecord.runtime_id` | provenance | identifies one runtime process, not the Artifact role claim. |
| `node_id` | `InvocationRecord.node_id`, `NodeRecord.node_id`, `RunnerRequestRecord.node_id` | provenance / join coordinate | ties passive invocation evidence to run-forest nodes, but is not an Artifact identity. |
| `branch_id` / `candidate_id` | `NodeRecord.{branch_id,candidate_id}`, `RunnerRequestRecord.branch_id` | provenance | useful for drilldown, not the role badge source fact. |

## 3. Minting Sites

### Invocation role

- `ploke_records::invocation::Role` defines `Child` and `Successor`.
- `ploke_records::invocation::InvocationRecord.role` records the role for one
  runtime invocation attempt.

### Invocation Artifact binding

- `InvocationRecord.node.derived_artifact_id` can carry the produced Artifact
  id through embedded `NodeRecord`.
- `InvocationRecord.request.derived_artifact_id` can carry the same binding
  through embedded `RunnerRequestRecord`.
- `Badge::from_invocation` currently chooses `node.derived_artifact_id` first,
  then `request.derived_artifact_id`.

### Graph read model

- `Graph::invocations()` exposes typed invocation records from
  `PassiveEvidence.run_attempts.invocations`.
- `GraphBuilder::observe_runner_request_metadata` observes and attaches
  `RunnerRequestRecord.derived_artifact_id`.
- `GraphBuilder::observe_node_artifacts` observes `NodeRecord.derived_artifact_id`.

## 4. Chosen Primary Carrier

Chosen primary UI witness: `Badge<'g>`.

Why:

- `Badge::Child(&ArtifactId)` preserves both role and Artifact binding.
- `Badge::Parent(&ArtifactId)` preserves the current UI convention that a
  successor invocation is shown as the parent/continuation role.
- The borrowed `ArtifactId` keeps the witness tied to graph-owned invocation
  evidence until the egui render boundary.

## 5. Rejected Alternatives

### `runtime_id` is not primary

It names a process attempt. It can explain provenance for the badge, but it
does not identify which Artifact the role badge should be attached to.

### `node_id` is not primary

It is a useful run-forest join coordinate and appears in invocation records, but
role badges are displayed on Artifact-bearing inspector selections. The badge
must bind to the Artifact id.

### `TreeNode.derived_artifact_id` is not sufficient

It can identify the selected run-forest node's produced Artifact, but it does
not prove whether a runtime was invoked as `Child` or `Successor`.

### `branch_id` and `candidate_id` are not primary

They are selection/evaluation provenance coordinates and do not classify
runtime role.

## 6. Upstream Record Types and Fields

### `ploke-records`

- `ploke_records::invocation::Role`
- `ploke_records::invocation::InvocationRecord.role`
- `ploke_records::invocation::InvocationRecord.node`
- `ploke_records::invocation::InvocationRecord.request`
- `ploke_records::scheduler::NodeRecord.derived_artifact_id`
- `ploke_records::scheduler::RunnerRequestRecord.derived_artifact_id`

### `ploke-tree`

- `ploke_tree::graph::Graph::invocations`
- `ploke_tree::PassiveEvidence.run_attempts`
- `ploke_tree::graph::ArtifactNode.ids.artifact_ids`
- `ploke_tree::TreeNode.derived_artifact_id`

## 7. Graph / Read-Model Carriers

Current graph carrier path:

- typed invocation records are loaded into `PassiveEvidence.run_attempts`
- `Graph::invocations()` exposes them without reparsing JSON in `ploke-egui`
- run-forest selections match invocation evidence by `InvocationRecord.node_id`
  or by the selected node's `derived_artifact_id`
- Artifact selections match invocation evidence by comparing the badge
  Artifact id to `ArtifactNode.artifact_ids()`

Graph invariant:

When a typed `InvocationRecord` carries role `Child` or `Successor` and carries
a `derived_artifact_id`, the UI may render a role badge for an inspector
selection whose graph Artifact ids contain that same `ArtifactId`.

## 8. Downstream UI Consumers

### `ploke-egui`

- `crates/ploke-egui/src/ui/text/decor.rs`
  - `Badge<'a>`
  - `BadgeText<'a>`
- `crates/ploke-egui/src/ui/inspector.rs`
  - `RoleBadgeSet<'g>`
  - `RunForestNodeInspection.role_badges`
  - `ArtifactInspection.role_badges`
  - `SelectionInspectorSnapshot.roles`
  - `InspectorSections::role_badges`
- `crates/ploke-egui/src/ui/app/shell.rs`
  - `render_badges`

## 9. Open Gaps and Caveats

- `Role::Successor` is currently rendered as `Badge::Parent` because the UI
  convention is "selected continuation runtime". If the product language needs
  separate `Successor` and `Parent` badges later, this report should be split
  or revised.
- The role claim is passive read-model evidence. It does not grant runtime
  authority or prove that the runtime is currently alive.
- Artifact matching still depends on the current graph Artifact id bundle. If
  grouped Artifact identities disagree, the role badge should remain tied to
  the concrete borrowed `ArtifactId` rather than to a projection label.

## 10. Implementation Guidance

Keep role badges as borrowed witnesses:

```rust
Badge::Child(&ArtifactId)
Badge::Parent(&ArtifactId)
```

Do not cache cloned `ArtifactId` role slots in `InspectorSections`. Cached
inspector sections may store stable selection handles, but role facts should be
resolved from `Graph` into `Badge<'_>` values during the render/snapshot pass.
