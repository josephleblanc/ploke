# Agent 16: Artifact Tree Backend Locator Census

Scope inspected: `crates/ploke-eval/src/loop_graph.rs`, `crates/ploke-eval/src/intervention/algebra`, `intervention/apply.rs`, `intervention/spec.rs`, `prototype1_state/backend.rs`, and directly adjacent filesystem/git/backend locator adapters.

## Summary

The current code has three overlapping but distinct concepts:

- graph vocabulary for naming runtime/artifact/patch operation inputs
- intervention and text-file adapters that can read or edit one bounded file surface
- git/history backend authority carriers that can materialize or verify artifact trees

The closest finished shape is the backend-mediated workspace path: `WorkspaceBackend` realizes child workspaces, persists bounded files, verifies selected branch content, installs selected artifacts into the active checkout, computes clean tree keys, and computes `SurfaceCommitment`s. The closest authority sketch is `history::Locator<T>` plus `ArtifactLocator`, but that locator currently proves/digests an opaque `Artifact` marker from a `TreeKeyHash`; it does not yet provide document/file access inside an artifact.

For a future child evaluation/evidence access operator, the code already says "operate over `Surface(Artifact)`" but the live access path is still `repo_root + relpath -> fs::read_to_string` or `git show branch:path`. A real operator should probably sit between those: authority comes from the artifact/tree/backend locator boundary, while readable evidence is a bounded surface projection, not arbitrary filesystem access.

## Causal Shape

The intended model in `prototype1_state/mod.rs` is:

```text
Runtime -> Surface(Artifact) -> PatchAttempt
PatchAttempt + base Artifact -> derived Artifact
derived Artifact -> hydrated Runtime
```

The same module defines an `Artifact` as a checkout state able to hydrate a runtime, not a path or branch name by itself. It also states that a runtime may operate over the surface of an artifact to produce a patch and apply that patch to create a new artifact.

Current implementation status:

- Finished enough for live Prototype 1: bounded tool-description file mutation and git-worktree child realization.
- Partially wired: artifact/patch operation-target provenance in registry, scheduler, apply output.
- Sketch: backend-agnostic semantic edit surfaces, artifact-local provenance manifests, generic artifact document access, and History-backed admission of evidence refs.

## Graph Vocabulary

Source: `crates/ploke-eval/src/loop_graph.rs`.

Types:

- `RuntimeId` is a durable UUID for one concrete runtime instance.
- `ArtifactId` is a backend-neutral string identity for a recoverable artifact state.
- `PatchId` is a durable identity for a generated or composed patch record, not a branch name.
- `OperationTarget` names what a runtime operates over:
  - `Artifact { artifact_id }`
  - `PatchSet { base_artifact_id, patch_ids }`
  - `ArtifactSet { base_artifact_id, artifact_ids }`
- `Coordinate { runtime_id, target }` binds one runtime to one operation target.

Authority/provenance notes:

- `ArtifactId` explicitly allows git commit ids, git tree ids, content digests, artifact-manifest ids, or another stable backend reference.
- Dirty worktrees are explicitly not supposed to receive an `ArtifactId` until recoverable.
- `PatchId` identifies the patch record; composed or LLM-resolved merge patches need their own ids.
- This module is vocabulary only. It has no filesystem access, no unpacking, and no authority to decide whether an id is valid.

Finished vs sketch:

- Finished shape: the enum shape correctly separates artifact, patch-set, and artifact-set targets.
- Sketch: `Coordinate` is mostly a provenance slot; live records do not yet uniformly carry runtime coordinates.

## Intervention Algebra

Source: `crates/ploke-eval/src/intervention/algebra/mod.rs`.

Types and traits:

- `Configuration` separates one joint artifact/binary world-state into `ArtifactState` and `BinaryState`.
- `Surface<C>` is the bounded read mediation layer:
  - associated `Target`
  - associated `ReadView`
  - associated `Error`
  - `read_view(&self, config, target)`.
- `RecordStore` is the append-only journal abstraction.
- `Intervention<From, To>` consumes a source configuration, writes before/after records, and returns `Outcome<To, Rejected>`.
- `CommitPhase`, `CommitError`, and `Outcome` frame journal-backed transitions.

Authority/provenance notes:

- The trait docs put writes behind interventions rather than exposing a general mutation API on `Surface`.
- This is the correct semantic boundary for a future child evidence access operator: read access is mediated by a surface over a configuration, while write access remains a transition.
- The trait does not define artifact tree location, authorization, snapshot identity, or document unpacking.

Finished vs sketch:

- Finished shape: the read/write separation is the right long-term direction.
- Sketch: there is no generic implementation that reads from an artifact locator or backend tree; live surfaces still read from paths.

## Text-File Edit Surface

Sources: `crates/ploke-eval/src/intervention/spec.rs`, `apply.rs`, `execute.rs`, `synthesize.rs`, and `tests.rs`.

Types and functions:

- `ArtifactEdit` supports `ReplaceWholeText`, `AppendText`, and marker-based `ReplaceSection`.
- `ValidationPolicy` restricts `allowed_relpaths` and checks target existence, non-empty result, UTF-8, content change, marker presence, and cargo-check intent.
- `InterventionSpec` currently supports `ToolGuidanceMutation` and `PolicyConfigMutation`, but the concrete adapter only supports tool guidance.
- `InterventionSynthesisInput` carries `issue`, `source_state_id`, `source_content`, and optional `operation_target`.
- `InterventionCandidate` carries `candidate_id`, `branch_label`, `proposed_content`, `spec`, and optional `patch_id`.
- `InterventionCandidateSet` carries the target relpath, source content, candidates, and optional operation target.
- `InterventionApplyInput` carries `source_state_id`, selected candidate, target relpath, expected source content, `repo_root`, optional `base_artifact_id`, and optional `patch_id`.
- `InterventionApplyOutput` records target path, absolute path, content hashes, validation, optional base artifact, patch, and derived artifact ids.
- `operation_target_artifact_id` extracts the base artifact from an `OperationTarget`.
- `text_file_artifact_id(target_relpath, content)` creates a narrow content-derived identity for the current text-file prototype surface.
- `text_replacement_patch_id(target_relpath, source_content, proposed_content)` creates a narrow patch/proposal identity.
- `execute_intervention_apply` reads the target, checks exact source-content equality, runs the text adapter, then records fallback artifact/patch identities.
- Private adapter traits in `execute.rs` split materialize, stage, apply, and validate. `execute_tool_text_intervention` wires them.

Filesystem/document access:

- `execute_intervention_apply` uses `repo_root.join(target_relpath)` and `fs::read_to_string`.
- `ToolTextInterventionAdapter` reads and writes the same absolute target path.
- `ensure_treatment_branch_materialized` replays a stored branch by reading the current file, checking source/proposed content, then calling `execute_intervention_apply`.
- Tests exercise synthesis and apply against `crates/ploke-core/tool_text/non_semantic_patch.md`.

Authority/provenance notes:

- `text_file_artifact_id` is explicitly not a whole-worktree `ArtifactId`; it names only target relpath plus text content.
- `text_replacement_patch_id` is explicitly not a branch id or durable artifact id.
- Source-content equality is the main drift guard.
- `ValidationPolicy::allowed_relpaths` is the current bounded-surface guard.
- The adapter can mutate arbitrary files only if the spec and validation policy admit the relpath; it is not tied to History authority.

Finished vs sketch:

- Finished live adapter: bounded full-text tool-description rewrites can be synthesized, applied, validated, and recorded.
- Sketch: `PolicyConfigMutation`, cargo-check validation, semantic/code-aware edits, multi-file patches, and backend-provided artifact identities are not implemented.

## Branch Registry And Scheduler Provenance

Sources: `crates/ploke-eval/src/intervention/branch_registry.rs` and `scheduler.rs`.

Types and functions:

- `TreatmentBranchNode` carries `patch_id`, `generation_target`, optional `generation_coordinate`, `status`, `apply_id`, `applied_content_hash`, and `derived_artifact_id`.
- `InterventionSourceNode` carries `source_artifact_id`, `operation_target`, target relpath, source content, and generated branches.
- `ActiveInterventionTarget` tracks the active branch/patch/apply/derived-artifact target for a relpath.
- `record_synthesized_branches` preserves supplied operation-target provenance or falls back to `text_file_artifact_id`.
- `mark_treatment_branch_applied` propagates base artifact, patch id, and derived artifact id into branch registry state.
- `select_treatment_branch`, `active_branch_selection_for_target`, `resolve_treatment_branch`, and `restore_treatment_branch` operate over stored branch registry records.
- `Prototype1NodeRecord` and `Prototype1RunnerRequest` carry optional `operation_target`, `base_artifact_id`, `patch_id`, and `derived_artifact_id`.

Authority/provenance notes:

- These are durable projections over synthesis/apply/evaluation state; they are not material locators.
- `branch_id` remains a registry/display handle; `patch_id` is the provenance handle.
- `generation_coordinate` exists but is not populated in the path I inspected.
- Fallback artifact ids preserve single-file provenance without pretending to identify a whole artifact.

Finished vs sketch:

- Finished enough: branch registry preserves source, candidate, patch, and derived-file identity across synthesis and apply.
- Sketch: registry and scheduler do not yet recover or unpack artifacts; they rely on backend paths/workspaces elsewhere.

## Workspace Backend

Source: `crates/ploke-eval/src/cli/prototype1_state/backend.rs`.

Types:

- `GitBranch` is the backend branch identity for a child lineage.
- `GitBranchRef` is the fully qualified branch ref used for verification.
- `GitCommit` is a checked-out workspace `HEAD`.
- `GitObjectId`, `GitTreeKey`, and `TreeKey` associated type represent backend-owned clean tree keys.
- `SurfaceRoots` contains private immutable/mutated/ambient before/after roots used to construct a History `SurfaceCommitment`.
- `RealizeRequest` names parent `repo_root`, scheduler `node_id`, `node_dir`, bounded `target_relpath`, expected source content, and proposed content.
- `Workspace<Branch, Head, Root>` records `parent_root`, `parent_head`, child branch, child root, and child head.
- `WorkspaceBackend` is the main backend trait.
- `GitWorktreeBackend` is the concrete git worktree implementation.

Important `WorkspaceBackend` methods:

- `realize` creates or verifies a child workspace and writes proposed target content.
- `remove` removes a managed child workspace after verification.
- `workspace_for_node` reconstructs a backend handle from persisted node state.
- `persist_workspace_target` and `persist_workspace_files` commit bounded files in the child workspace.
- `verify_artifact_target` checks that a durable artifact branch carries expected target content.
- `install_artifact_in_active_checkout` switches the active parent checkout to a selected artifact branch.
- `checkout_fresh_parent_branch` initializes a gen0 parent branch.
- `persist_active_checkout_files` commits bounded files in the active checkout.
- `validate_parent_checkout` checks that the active checkout can act as a given parent.
- `clean_tree_key` derives a backend-owned clean artifact tree key.
- `surface_commitment` computes the current Prototype 1 surface commitment from two checked-out artifacts.

Filesystem/git access:

- `realize` uses git worktree creation or reuse, then reads and writes `root.join(target_relpath)`.
- `ensure_reusable` verifies git worktree metadata, branch, path, dirty paths, target existence, and target content.
- `verify_artifact_target` uses `git show <branch>:<target_relpath>` to inspect a file inside an artifact branch without checking it out.
- `persist_files` stages and commits only declared relpaths after rejecting unexpected dirty paths.
- `clean_tree_key` shells out to `git rev-parse HEAD^{tree}`.
- `surface_commitment` calls `tracked_paths`, `tool_description_paths`, and `surface_hash`; `surface_hash` reads file bytes from checked-out roots.

Authority/provenance notes:

- `Workspace` warns that `parent_head` and `head` are git commit identities, not full content witnesses. A child target may diverge in the worktree before `head` changes.
- `SurfaceRoots` fields and constructor are private; sibling modules cannot fabricate roots for History.
- `clean_tree_key` is documented as the operation History admission should rely on, and callers should not construct tree keys from strings.
- `validate_parent_checkout` refuses dirty active checkouts and checks branch identity, commit message, parent identity path, and gen0 branch freshness.
- The backend owns operational authority over git worktrees; History owns admission authority over tree/surface commitments.

Finished vs sketch:

- Finished live shape: git worktree realization, reuse checks, bounded target persistence, selected artifact installation, parent checkout validation, and surface commitment hashing.
- Sketch/not final: the module TODO says shelling out to git and parsing output is acceptable for the prototype but not the long-term shape. `clean_tree_key` and tree-key admission are staged for History wiring. There is no generic "open document from artifact surface" trait.

## Typed C1 Surface And Workspace Lifecycle Sketch

Sources: `prototype1_state/c1.rs` and `workspace.rs`.

Types and functions:

- `Artifact<L>` carries `repo_root`, `target_relpath`, source/current/proposed content hashes, and lineage marker.
- `Binary<L, ChildState, AckState>` carries parent/child runtime state.
- `Prototype<Running, ArtifactWorld, ChildState, AckState>` implements `Configuration`.
- `ToolDescriptionSurface` implements `Surface<Prototype<Parent, Parent, Absent, Unacknowledged>>` with `Target = PathBuf` and `ReadView = String`.
- `MaterializeBranch<B>` implements the `C1 -> C2` transition using `WorkspaceBackend::realize`.
- `workspace.rs` sketches `SharedPaths`, `ActiveCheckout`, `ChildWorktree`, `World`, and `Action::{Create, Select, Update, Build, Cleanup, Exit}`.

Filesystem/document access:

- `Prototype<Parent, Parent, Absent, Unacknowledged>::load` reads the target from `repo_root.join(resolved.target_relpath)` and checks it matches stored source content.
- `ToolDescriptionSurface::read_view` reads the requested target path from the configuration artifact root.
- `MaterializeBranch::transition` rereads the surface, then asks the backend to realize the child workspace.

Authority/provenance notes:

- The type parameters preserve parent/child artifact and runtime state better than flattened status events.
- The current `Artifact<L>` is path/hash-bearing working state, not a durable artifact identity.
- `workspace.rs` explicitly says logical nodes are durable while realized workspaces are cache-like.

Finished vs sketch:

- Finished direction: move-only transition shape and backend-mediated child realization.
- Sketch: `workspace.rs` is explicitly not wired into the full loop; C1-C4 are typed-transition scaffolding next to older live controller paths.

## History Locator And Surface Commitments

Sources: `prototype1_state/history.rs` and `parent.rs`.

Types:

- `SurfaceRoot`, `Surface<P>`, `SurfaceDelta<P>`, and `SurfaceCommitment` model immutable, mutated, and ambient surface partitions.
- `surface::{Immutable, Mutated, Ambient, Bounded}` are partition/state markers.
- `Locator<T>` defines `Key`, `Digest`, `Error`, `locate`, and `digest`.
- `Artifact` is an opaque recoverable artifact state target for verifiable claims.
- `ArtifactLocator` implements `Locator<Artifact>` with `Key = TreeKeyHash`.
- `Digest<T>` is stored evidence, not recovery capability.
- `Verifiable<T, L>` stores key plus digest for an object checked through locator `L`.
- `ArtifactPath` and `Manifest` exist as initial artifact-local provenance manifest vocabulary.
- `TreeKeyHash` commits a backend-owned clean tree key.
- `TreeKeyCommitment` lets a backend key produce a `TreeKeyHash`; `GitTreeKey` implements it.

Authority/provenance notes:

- `Locator<T>` is a capability, not stored block data.
- `ArtifactLocator` is explicitly a bridge, not the final artifact identity model.
- `ArtifactLocator::locate` currently returns an opaque `Artifact` marker and does not load files.
- `ArtifactLocator::digest` hashes the `TreeKeyHash` under the `prototype1.history.artifact.digest.v1` domain.
- `TreeKeyHash::from_serialized_key` is private, so caller-authored strings cannot become artifact identity witnesses.
- `Parent<Unchecked>::check` calls `WorkspaceBackend::validate_parent_checkout`.
- `Startup<Predecessor>::from_history` uses `GitWorktreeBackend.clean_tree_key`, `TreeKeyCommitment::tree_key_hash`, `sealed.verify_current_artifact_tree(&current_artifact, &ArtifactLocator)`, `GitWorktreeBackend.surface_commitment`, and `sealed.verify_current_surface`.

Finished vs sketch:

- Finished authority shape: predecessor startup checks current artifact tree and surface against sealed History using backend-derived keys and locator verification.
- Sketch: artifact-local manifests, `ArtifactPath`, and `Manifest` are vocabulary only. The locator cannot enumerate, read, or unpack documents inside an artifact.

## Existing Access Paths Inside An Artifact Or Surface

Today there are four concrete access patterns:

- Checked-out filesystem read/write: `repo_root.join(target_relpath)` with `fs::read_to_string` or `fs::write`.
- Git branch file read: `verify_artifact_target` uses `git show branch:path`.
- Checked-out surface hashing: `surface_commitment` reads declared files from before/after roots and hashes bytes.
- History locator verification: `ArtifactLocator` verifies a tree-key commitment but returns only an opaque marker.

Only the first three can access document/file bytes. Only the last two participate in History-style authority, and only `surface_commitment` currently bridges byte access into a typed History commitment.

## Relation To Child Evaluation And Evidence Access

A future child evaluation/evidence access operator needs two separate capabilities:

- Authority capability: prove the child is reading from the artifact/surface it claims.
- Read capability: project bounded evidence from that artifact/surface into documents, files, records, or semantic nodes.

Existing reusable pieces:

- `OperationTarget` can name the artifact, patch set, or artifact set the runtime operates over.
- `WorkspaceBackend::clean_tree_key` and `TreeKeyCommitment` can produce History-checkable artifact commitments.
- `WorkspaceBackend::surface_commitment` can hash policy-defined surface partitions without executing the candidate.
- `Surface<C>::read_view` is the right algebraic slot for bounded read projections.
- `verify_artifact_target` demonstrates direct file lookup inside a durable git branch.

Missing structure:

- There is no trait that takes an admitted `Artifact` or `TreeKeyHash` plus a bounded target and returns file/document bytes.
- There is no artifact-local manifest committed by both tree and History block.
- There is no semantic target type beyond relpaths and tool-description files.
- There is no evidence-access record tying child evaluation inputs to artifact locator, surface partition, target, digest, and policy.
- There is no uniform child operator that can read evidence without inheriting arbitrary filesystem authority from `repo_root`.

Design implication:

The future operator should not treat `repo_root` or `branch_id` as authority. It should take an artifact/tree locator or checked History claim, then project a bounded surface view. For git, that implementation can initially use checked-out roots or `git show`; semantically, the operator should be shaped like an artifact-surface read, not a path read.

## Relation To Semantic Edit Surface

The current edit surface is a text-file adapter:

- target identity is `PathBuf`
- view is `String`
- patch is a full/append/marker text replacement
- fallback artifact identity is `text-file-sha256`
- fallback patch identity is `text-replace-sha256`

The finished part worth preserving is the mediation rule: reads are bounded by `Surface`, writes happen through `Intervention`, and durable records preserve base/patch/derived identity.

The sketch part is everything semantic:

- no node/span-aware target
- no multi-file patch set
- no structured diff or AST edit
- no semantic validation beyond text/marker checks
- no backend-agnostic document retrieval from an artifact
- no History-admitted edit/evidence manifest

For HyperAgents, the semantic edit surface should extend the same algebra rather than bypass it. A child should read evidence through a bounded artifact surface, propose a patch with a durable `PatchId`, apply it against an explicit base artifact, and produce a derived artifact whose tree/surface commitments can be admitted by History.

## Bottom Line

Already solid:

- graph vocabulary separates runtime, artifact, patch, and operation target
- intervention algebra separates read surfaces from write transitions
- git backend realizes/verifies/persists child workspaces without treating git as the semantic model
- History has a typed locator/digest envelope for artifact tree commitments
- parent startup can verify current artifact tree and surface against sealed History

Still only a sketch:

- generic artifact/file/document locator
- semantic edit surface
- artifact-local provenance manifest
- uniform child evidence-access operator
- full use of `Coordinate`
- native git object validation instead of shell-command parsing

The main actionable design boundary is: child evidence access should be modeled as an authorized `Surface(Artifact)` projection. Current filesystem and `git show` reads are implementation techniques, not the durable authority model.
