## esm-retain-model

Read-only retainer report. No files changed.

Target guidance: `docs/workflow/evalnomicon/drafts/edit-surface/model.md`.
Explicitly ignored stale target guidance:
`docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md`.

Authority chain to retain:

- `Parent<Ruling>` grants authority over an artifact-bound coordinate.
- `SurfaceGrant` applies to `Coordinate = (generator Runtime, target Artifact)`.
- `EditProposal` is intermediate procedure state, not an Artifact transition.
- `SurfaceCheck` and checked apply are eval-owned authority gates.
- `ArtifactDelta` is the material transition from base Artifact to derived Artifact.
- History must record coordinate, grant, proposal, check, transition, hashes,
  and later child evaluation evidence.

Lossy reductions to refuse:

- Treating prompt, preview, UI approval, TUI proposal status, or child-plan file
  persistence as authority.
- Collapsing `SurfaceGrant` into `SurfaceCommitment`.
- Letting a harness output bypass eval-owned containment/hash checks.
- Treating Runtime as the writable substrate for create edits.
- Treating branch/worktree/scheduler projections as primary authority.

Structural carriers for 2-6 month stability:

- `loop_graph::Coordinate` and `OperationTarget`.
- `SurfaceGrant` with readable/writable/immutable/ambient sets and policy.
- `Proposal(q, c, g)` as proposal-state carrier.
- `SurfaceCheck` with semantic and material containment plus hash validation.
- `ArtifactDelta` and derived Artifact identity.
- Harness input/run envelopes that preserve provenance without authority.
- History records that preserve grant/proposal/check/transition/evaluation joins.

Exact ranges reported for main-thread verification:

- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:28`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:81`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:107`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:303`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:365`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:438`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:549`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:764`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:854`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:156`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:303`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:520`
- `crates/ploke-eval/src/loop_graph.rs:132`
- `crates/ploke-eval/src/intervention/spec.rs:124`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:901`
