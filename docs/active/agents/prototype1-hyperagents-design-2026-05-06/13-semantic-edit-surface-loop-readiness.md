# Agent 13: Semantic Edit Surface Loop Readiness

Scope: `ploke-tui` semantic edit path, Prototype 1 surface-filtering concepts
already present in docs/code, and how an agent could explore/apply bounded
semantic edits as a child patch. This is a readiness report only; no source code
was modified.

## Status

`ploke-tui` already has enough reusable machinery to resolve a model-authored
canonical Rust edit into hash-checked byte splices:

- tool DTO: `GatCodeEdit`, `CodeEditParams`, `CanonicalEditBorrowed`,
  `CanonicalEditOwned` in `crates/ploke-tui/src/tools/code_edit.rs`;
- request carrier: `ApplyCodeEditRequest` and `Edit::Canonical` in
  `crates/ploke-tui/src/rag/utils.rs`;
- canonical resolver/stager: `apply_code_edit_tool`,
  `split_canon_for_semantic_target`, `SemanticCanonTarget`, and
  `stage_semantic_edit_proposal` in `crates/ploke-tui/src/rag/tools.rs`;
- graph lookup: `ploke_db::helpers::graph_resolve_exact` and
  `ploke_db::helpers::resolve_nodes_by_canon` in
  `crates/ploke-db/src/helpers.rs`;
- write substrate: `WriteSnippetData` in `crates/ploke-core/src/io_types.rs`,
  `IoManagerHandle::write_snippets_batch` in `crates/ploke-io/src/handle.rs`,
  and `process_one_write` in `crates/ploke-io/src/write.rs`.

That path is not yet ready to be the Prototype 1 child patch boundary as-is.
The reusable edit operation is embedded in TUI session state: `AppState.proposals`
stores `EditProposal` values in `crates/ploke-tui/src/app_state/core.rs`,
approval is routed through `approve_edits` and private `apply_semantic_edit` in
`crates/ploke-tui/src/rag/editing.rs`, and status/evidence is emitted through
chat/tool UI events. A Prototype 1 child patch needs an artifact-local,
parent-authorized transition record, not a TUI proposal registry.

## Reusable Edit APIs

The best reusable unit is the graph-backed conversion:

```text
(file, canon, node_type, replacement)
  -> canonical target parse
  -> path-scope validation
  -> exact DB node resolution
  -> WriteSnippetData { file_path, expected_file_hash, start_byte, end_byte, replacement, namespace }
  -> ploke-io checked splice
```

Current code that implements this lives in `apply_code_edit_tool` in
`crates/ploke-tui/src/rag/tools.rs`. It validates `NodeType` against
`NodeType::primary_and_assoc_nodes`, resolves paths via
`crate::utils::path_scoping::resolve_tool_path`, parses methods differently from
primary items via `SemanticCanonTarget`, resolves through
`graph_resolve_exact`, optionally falls back to `resolve_nodes_by_canon`, and
builds `WriteSnippetData`.

`ploke-io` is a strong substrate for bounded edits. `process_one_write` verifies
the expected tracking hash, byte range, UTF-8 boundaries, and then writes
atomically through a temp file plus rename. That gives a child patch path useful
pre/post evidence: expected hash, resolved span, replacement digest, result
hash, and per-edit failure cause.

The weaker reusable surfaces are proposal and approval. `EditProposal` carries
useful preview data and the staged `WriteSnippetData`, but it also carries TUI
request/call ids and status strings for an interactive session. `apply_semantic_edit`
marks success when `applied > 0`, which is acceptable for a UI feedback path but
too weak for an ordinary Prototype 1 child patch unless partial application is a
typed rejected outcome.

## Existing Surface Concepts

Prototype 1 already has the right abstract slot for bounded editing:

- `Surface<C>` in `crates/ploke-eval/src/intervention/algebra/mod.rs` names a
  bounded read/edit mediation layer over a `Configuration`.
- `Intervention<From, To>` in the same file consumes a source configuration,
  appends before/after records, and produces either `Outcome::Advanced` or
  `Outcome::Rejected`.
- `ArtifactId`, `PatchId`, `OperationTarget`, and `Coordinate` in
  `crates/ploke-eval/src/loop_graph.rs` provide durable vocabulary for artifact
  and patch provenance.

The concrete current intervention path is narrower and text-oriented.
`InterventionSpec` in `crates/ploke-eval/src/intervention/spec.rs` supports
`ToolGuidanceMutation` and `PolicyConfigMutation`; `ArtifactEdit` supports
`ReplaceWholeText`, `AppendText`, and `ReplaceSection`; `ValidationPolicy`
allowlists `allowed_relpaths` and checks target existence, UTF-8, nonempty
result, content change, markers, and optional cargo check. The applier
`execute_intervention_apply` in `crates/ploke-eval/src/intervention/apply.rs`
returns before/after content hashes plus optional `base_artifact_id`,
`patch_id`, and `derived_artifact_id`.

Surface filtering also exists at the History/Crown layer. `SurfaceCommitment`
in `crates/ploke-eval/src/cli/prototype1_state/history.rs` partitions the
artifact surface as `surface::Immutable`, `surface::Mutated`, and
`surface::Ambient`. `GitWorktreeBackend::surface_commitment` in
`crates/ploke-eval/src/cli/prototype1_state/backend.rs` currently treats
`crates/ploke-eval` as immutable, all `ToolName::ALL` tool-description files as
the ordinary mutated surface, and ambient as empty. It rejects an immutable
surface change.

For ordinary Prototype 1 succession, this means semantic Rust edits to
`crates/ploke-eval` are forbidden unless an explicit protocol-upgrade/fork path
is introduced. A semantic edit demo should target a non-policy fixture or
declared mutable code surface, not the runtime/Crown/History implementation.

## Missing Boundaries

1. No pure semantic edit service exists.

   The needed boundary is a non-TUI API such as:

   ```text
   resolve_semantic_edits(index, io_context, surface, request) -> Vec<WriteSnippetData>
   ```

   It should expose canonical target parsing, DB resolution, and preview/evidence
   generation without `AppState`, `EventBus`, chat messages, or user config
   proposal persistence.

2. No artifact/index identity check exists for semantic resolution.

   `apply_code_edit_tool` assumes the loaded TUI DB corresponds to the workspace
   being edited. A child patch path must prove that the DB/index snapshot was
   produced from the same child `Artifact` or from an accepted pre-apply refresh.
   Otherwise a span from one checkout can be applied to another.

3. Path allowlisting is not semantic enough.

   Current Prototype 1 `ValidationPolicy.allowed_relpaths` checks relpaths after
   a text edit. Semantic Rust edits need a pre-apply surface check over
   `(relpath, canon, node_type, resolved node id/span, expected hash)`. A relpath
   allowlist alone cannot express "this function/method is allowed, adjacent
   items and policy-bearing code are forbidden."

4. Proposal status is not authority.

   `EditProposalStatus::Applied` in `crates/ploke-tui/src/app_state/core.rs` is
   a TUI proposal status. Prototype 1 should instead record a parent-authorized
   transition with durable request id, runtime/artifact coordinate, surface
   digest, resolved targets, per-edit results, and derived artifact/patch ids.

5. Child patch materialization currently assumes full text replacement.

   `RealizeRequest` in `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
   carries one `target_relpath`, `source_content`, and `proposed_content`.
   `MaterializeBranch` in `crates/ploke-eval/src/cli/prototype1_state/c1.rs`
   uses that text branch to create the child workspace. A semantic patch path
   would either need to materialize the resolved `WriteSnippetData` edits into
   proposed file content before `RealizeRequest`, or generalize the child branch
   record to carry semantic edit specs and their materialized result.

## Evidence And Provenance To Capture

A bounded semantic child patch can capture stronger evidence than the current
TUI proposal if it records the following as a generation-local artifact:

- `runtime_id`, `ArtifactId`, `OperationTarget`, optional parent `ArtifactId`,
  and generated `PatchId`;
- source index identity: DB/index snapshot id, parse timestamp or artifact tree
  key, and workspace root used for resolution;
- surface policy identity: allowed relpaths, allowed canonical prefixes,
  allowed `NodeType`s, forbidden relpaths/prefixes, and digest of that policy;
- request: original `file`, `canon`, `node_type`, replacement digest, optional
  model/tool call id, and confidence;
- resolution: absolute/relative path, node id if available, resolved
  `WriteSnippetData` span, expected `TrackingHash`, namespace, and whether
  relaxed fallback was used;
- preview: unified diff or before/after digest, with text optionally stored as
  an artifact-local sidecar;
- apply result: all-edits-applied boolean, per-edit `WriteResult.new_file_hash`
  or error, changed relpaths, final file/content hashes, and `git diff`/commit
  witness if the backend persists the child artifact;
- transition refs: `PrototypeJournal` before/after entry ids if wired through
  `Intervention`, child node id/generation/branch id, and evaluation result path
  after the child run.

The final benchmark-style patch remains a separate projection. `runner.rs`
collects TUI proposal status through `collect_patch_artifact_with_expected` and
submission diffs through `collect_submission_fix_patch`, but those are not the
same as a semantic edit admission record. For Prototype 1 loop readiness, the
semantic edit record should be primary and `git diff`/submission output should
be derived evidence.

## Allowed And Forbidden Surfaces

A practical expression for bounded semantic edits is a structural surface policy:

```text
SemanticRustItemSurface {
  artifact: ArtifactId,
  allowed_relpaths: [...],
  forbidden_relpaths: ["crates/ploke-eval/**", ...],
  allowed_node_types: [function, method],
  allowed_canon_prefixes: ["crate::demo::"],
  forbidden_canon_prefixes: ["crate::cli::prototype1_state::", ...],
  max_files: N,
  max_edits: N,
  require_exact_resolution: true,
  require_all_edits_applied: true,
  require_surface_commitment: true,
}
```

This should not live as long helper names or status strings. The natural carrier
is a concrete `Surface<C>` implementation over an artifact configuration, with
an associated `Target` such as a semantic Rust item target. The transition that
applies it should be a typed `Intervention<Artifact<Indexed>, Artifact<Patched>>`
or equivalent role/state carrier, not a public helper that writes arbitrary
status updates.

Under the current hardcoded ordinary-succession policy:

- allowed for today's implemented loop: tool-description text files named by
  `ToolName::ALL`;
- forbidden for ordinary succession: `crates/ploke-eval` and any surface whose
  mutation changes the immutable `SurfaceCommitment` root;
- plausible demo extension: a non-policy fixture or explicitly declared mutable
  Rust surface outside `crates/ploke-eval`;
- future-only: semantic edits to `crates/ploke-eval`, because that requires an
  explicit protocol-upgrade/fork transition.

## Connection To Child Patch Path

The current child path is:

```text
InterventionCandidateSet / InterventionCandidate
  -> execute_intervention_apply
  -> branch registry treatment branch
  -> C1 MaterializeBranch / RealizeRequest
  -> C2 BuildChild
  -> C3 SpawnChild
  -> C4 ObserveChild
  -> Prototype1RunnerResult / evaluation report / successor selection
```

To connect semantic edits cleanly:

1. Add a semantic intervention spec parallel to the text `ArtifactEdit` path.

   It should carry semantic edit requests plus an explicit semantic surface
   policy. It should not reuse `ReplaceWholeText` as the durable shape unless
   the semantic resolution evidence is also persisted.

2. Resolve semantic targets before materialization.

   A resolver should produce `WriteSnippetData` plus resolution evidence from an
   artifact-matched index. It should reject zero matches, ambiguous matches, and
   relaxed fallback unless the surface policy explicitly allows fallback.

3. Materialize to child proposed content.

   Either compute proposed file content and feed the existing `RealizeRequest`
   for a one-file demo, or generalize `RealizeRequest` / branch registry records
   to carry multiple changed relpaths and semantic edit evidence. The backend
   already has `persist_workspace_files`, so multi-file persistence is closer
   than the current one-target `persist_workspace_target` path suggests.

4. Apply with all-or-rejected semantics.

   Prototype 1 should require every resolved edit to apply. Partial apply should
   produce a rejected transition with per-edit errors, not an advanced child
   artifact.

5. Persist patch/artifact identity.

   Use `PatchId` for the semantic patch record, `ArtifactId` for source/derived
   artifact identities, and `OperationTarget::Artifact` or
   `OperationTarget::PatchSet` when composing later patches. The current
   text-file fallback ids in `text_file_artifact_id` and
   `text_replacement_patch_id` are explicitly not whole-worktree artifact ids.

6. Join to child evidence.

   Include the semantic patch record path or digest in the node/branch/evaluation
   records that feed `Prototype1RunnerResult`, `Prototype1BranchEvaluationReport`,
   and `SuccessorDecision`. Report 01's proposed child-evaluation bundle is the
   right downstream join point.

## Readiness Judgment

Ready for a bounded demo after factoring, not ready for direct live loop use.

The viable first demo is a single semantic Rust item outside `crates/ploke-eval`
and outside any policy-bearing surface. The resolver can reuse the existing
`ploke-tui` logic and `ploke-io` writes, but the authoritative path must be a
Prototype 1 intervention record with an explicit semantic surface policy,
artifact/index provenance, all-or-rejected apply semantics, and patch/artifact
ids.

Directly calling `ploke-tui::apply_code_edit_tool` from Prototype 1 would be the
wrong boundary. It would import TUI chat/session semantics and proposal status
into a child artifact transition, while still leaving the important readiness
gaps unresolved: artifact-bound index identity, semantic surface authorization,
parent approval as a transition, and durable patch provenance.
