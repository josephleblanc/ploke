# 2026-05-12 Broad Bounded Surface Transition Plan

## Purpose

Concrete handoff for implementing the broad bounded-surface path without
collapsing Parent authority into a prompt-and-file-drop workflow.

This is the next planning packet for the current bounded edit-surface thread.
It sits on top of:

- [`2026-05-08_bounded-edit-surface-handoff.md`](2026-05-08_bounded-edit-surface-handoff.md)
- [`2026-05-08_bounded-edit-surface-implementation-orientation.md`](2026-05-08_bounded-edit-surface-implementation-orientation.md)
- [`../bugs/2026-05-12-prototype1-broad-harness-request-plan-erasure.md`](../bugs/2026-05-12-prototype1-broad-harness-request-plan-erasure.md)

## Status

Current implementation state after the first broad-surface slice:

- Slice 1 is landed.
- Broad request publication now crosses a typed parent transition instead of
  existing only as a `PrepareError` string.
- The broad path still stops before proposal admission or child materialization.
- Complete live runs still reject `BroadHarness`.

Code landed in:

- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs)
  Added `PublishedBroadHarnessRequest` with persisted `request_id` and
  `request_hash`.
- [`crates/ploke-eval/src/cli/prototype1_state/parent.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs)
  Added `Parent<AwaitingHarnessPlan>` and the
  `Parent<Ready> -> Parent<AwaitingHarnessPlan>` transition.
- [`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs)
  Split broad request publication from normal child-plan generation and
  preserved the current fail-closed stop after the typed publication step.

Verified with:

- `cargo test -p ploke-eval broad_harness`
- `cargo test -p ploke-eval broad_workspace_edit_surface_publishes_harness_request_not_fake_children`

## Reset Note

Use the bounded-surface model as the semantic center of gravity for follow-on
work.

- `SurfaceGrant` is the authority-bearing object: the Parent saying "this is
  allowed."
- Any persisted broad-harness request is a derived projection of granted
  authority, not a new authority object created by writing a file.
- `Parent <-> Child` runtime channels remain a separate cross-runtime protocol
  concern. Broad-harness request persistence is not a runtime channel in that
  sense.
- Logs, prompts, mailbox files, and similar persistence surfaces remain
  non-authoritative until they are checked back through `Grant::check` and
  admitted by the Parent.

## Larger Object

Do not implement this as "let `BroadHarness` make children."

The object to preserve is:

```text
Parent authority
  -> bounded proposal procedure over an Artifact
  -> admitted ChildPlan
  -> derived Artifact
  -> hydrated Child runtime
```

HyperAgents-style archive traversal changes how Parents are chosen. It does not
remove the need for an explicit create/admit boundary.

## Incorrect Reduction To Refuse

Do not continue the current reduction:

```text
GenerationSource::BroadHarness
  -> publish prompt
  -> read child-plan file
  -> trust that it came from the same objective/grant/policy
```

That reduction loses:

- published-request identity;
- base Artifact identity;
- grant identity;
- projection identity;
- proposal provenance;
- checked containment over `R/W/F`;
- proof that the resulting `ChildPlan` came from the published request.

## Target Transition

The correct broad path is:

```text
Parent<Ruling>
  -> EditObjective
  -> SurfaceGrant<Broad>
  -> HarnessRequest<Broad, Published>
  -> Proposal<For<Request>>
  -> CheckedProposal<Broad>
  -> ChildPlan<For<Request>>
  -> derived Artifact
  -> Child runtime
```

The key design point is that the broad path is not "another generator." It is a
request-bound proposal-and-admission transition.

## Structural Carriers And Module Homes

### 1. `edit_surface::surface`

File:

- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs)

Keep this module as the home of artifact-bound grant/check structure.

Recommended additions:

```rust
pub(crate) struct SurfaceGrant<K> {
    coordinate: Coordinate,
    readable: ReadSet,
    writable: WriteSet,
    forbidden: ForbiddenSet,
    ambient: AmbientReadSet,
    grantor: ParentRef,
    policy: SurfacePolicyId,
    kind: K,
}

pub(crate) enum Broad {}
```

Pragmatic v1 alternative if a generic `SurfaceGrant<K>` is too disruptive:

- keep the existing `Grant` carrier;
- add a `BroadSurfacePolicy` or equivalent constructor path that makes `R/W/F`
  explicit;
- do not hide broad-surface policy in prompt text alone.

Required invariant:

```text
Q_w ⊆ W
Q_w ∩ F = ∅
```

Recommended first constructor:

```rust
pub(crate) fn prototype1_workspace_broad(
    artifact: Ref,
    graph: graph::Bounds,
    protected_core: ForbiddenSet,
    grantor: ParentRef,
    policy: SurfacePolicyId,
) -> Result<SurfaceGrant<Broad>, Error>
```

For v1 broad surface:

- `R`: most or all of the parent Artifact plus admitted evidence refs;
- `W`: workspace surface outside protected authority core;
- `F`: `crates/ploke-eval` policy/authority core and similar non-ordinary-edit
  surfaces.

### 2. `edit_surface::harness_request`

File:

- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs)

This module should own the request object, not only the prompt DTO.

Recommended shape:

```rust
pub(crate) struct HarnessRequest<K, S> {
    id: RequestId,
    base_artifact: surface::Ref,
    objective: surface::EditObjective,
    grant: SurfaceGrant<K>,
    evidence_roots: Vec<EvidenceRoot>,
    child_budget: HarnessChildBudget,
    state: S,
}

pub(crate) enum Published {}
```

Minimum persisted identity:

- request id;
- canonical request hash;
- base Artifact id;
- objective binding;
- grant policy id;
- protected-core / forbidden identity.

The current `BroadHarnessRequest::prototype1_workspace(...)` is useful seed
material, but it must become a projection from `EditObjective + SurfaceGrant`,
not a standalone authority object.

### 3. `edit_surface::request_policy`

File:

- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/request_policy.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/request_policy.rs)

This module already contains the strongest existing provenance carrier.
Keep it, but stop treating "Router-backed" as the whole abstraction.

Recommended direction:

```rust
pub(crate) enum ProposalProducer {
    DeterministicTuiTools,
    Router { request_policy: Receipt },
    HumanCurated,
}
```

If the current `NonRouter` variant remains for compatibility, it should not be
enough to satisfy the broad request path by itself.

Add a request-bound receipt carrier:

```rust
pub(crate) struct ProposalReceipt {
    request_id: String,
    request_hash: String,
    proposal_id: String,
    run_id: String,
    base_artifact_id: ArtifactId,
    objective: ObjectiveBinding,
    producer: ProposalProducer,
}
```

Required verification:

- receipt request id/hash matches the published request;
- receipt base Artifact matches checked base Artifact;
- proposal/run binding is stable;
- producer-specific policy receipts validate if present.

### 4. `edit_surface::harness`

File:

- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs)

This module should carry the request-bound proposal and checked-proposal
structure.

Recommended shape:

```rust
pub(crate) struct Proposal<R> {
    request: R,
    proposal_id: String,
    run_id: String,
    projection: ProjectionIdentity,
    touches: ResolvedTouches,
    receipt: request_policy::ProposalReceipt,
}

pub(crate) struct CheckedProposal<K> {
    request: HarnessRequest<K, Published>,
    proposal_id: String,
    run_id: String,
    projection: ProjectionIdentity,
    check: surface::Check,
    receipt: request_policy::ProposalReceipt,
}
```

The key rule is:

```text
CheckedProposal is the only normal path from broad harness output to ChildPlan admission.
```

The existing `ArtifactDelta` remains the downstream patch-shaped evidence after
check/apply.

### 5. `prototype1_state::parent`

File:

- [`crates/ploke-eval/src/cli/prototype1_state/parent.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs)

`ChildPlanFiles` is already the right parent-owned boundary, but broad surface
needs more than the current optional `surface` attachment.

Recommended addition:

```rust
pub(crate) struct BroadSurfaceEvidence {
    request_id: String,
    request_hash: String,
    proposal_id: String,
    run_id: String,
    base_artifact_id: ArtifactId,
    producer: request_policy::ProposalProducer,
    projection: ProjectionIdentity,
    resolved_touches: ResolvedTouches,
}
```

Then either:

- add a broad-specific evidence variant under the existing child surface
  evidence carrier; or
- add an adjacent request-bound evidence field on `ChildFiles`.

The important constraint is structural, not cosmetic:

```text
ChildPlanFiles must be able to prove "this child came from this published request over this checked broad grant."
```

### 6. `prototype1_state::history`

File:

- [`crates/ploke-eval/src/cli/prototype1_state/history.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs)

History should admit broad checked-proposal and child-plan provenance as typed
evidence, not as prompt text or monitor-level metadata.

At minimum, selection-facing evidence should be able to recover:

- which objective and broad surface produced the candidate;
- whether the producer was deterministic, router-backed, or other;
- which protected-core policy was in effect.

## CLI / State Transition Changes

### `cli_facing.rs`

File:

- [`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs)

Do not grow more flat `Broad*` checks here. The CLI should project the typed
transition.

Recommended transition split:

1. Publish request:
   - `Parent<Ready>` -> `Parent<AwaitingHarnessPlan<Broad>>`
   - write request artifacts
   - stop with typed pending state, not a semantic error string
2. Admit response:
   - load response as `Proposal<For<Request>>`
   - verify request binding and proposal provenance
   - produce `CheckedProposal<Broad>`
3. Build child plan:
   - project checked proposal into `ChildPlanFiles`
   - continue into normal materialize/build/spawn flow

Until these carriers exist, the current complete-run rejection of
`BroadHarness` should remain in place.

## First Vertical Slices

### Slice 1: Published Request State

Goal:

```text
EditObjective + SurfaceGrant<Broad>
  -> HarnessRequest<Broad, Published>
  -> Parent<AwaitingHarnessPlan<Broad>>
```

Status:

- Implemented as an initial typed publication slice.
- Current code uses `PublishedBroadHarnessRequest` plus
  `Parent<AwaitingHarnessPlan>` as the concrete carrier names.
- The request still does not carry full `EditObjective + SurfaceGrant<Broad>`
  structure yet; that remains follow-up work for tightening the request object
  around the broader bounded-surface algebra.

Done when:

- the broad path no longer claims to be a normal runnable candidate generator;
- request identity/hash is persisted;
- complete live runs still fail closed;
- pending broad request is represented structurally, not only as `PrepareError`.

What is still missing in Slice 1:

- the published request is not yet a generic `HarnessRequest<Broad, Published>`
  carrier;
- the request is still projected from the existing broad prompt DTO rather than
  a full `EditObjective + SurfaceGrant<Broad>` object;
- the stop path is still surfaced to the operator as a pending `PrepareError`
  after the typed state transition, because the response-admission path does not
  exist yet.

Suggested tests:

- `broad_request_publication_produces_published_request_identity`
- `broad_request_complete_run_still_rejects_without_response_support`

Likely test homes:

- [`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs)
- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs)

### Slice 2: Request-Bound Checked Proposal

Goal:

```text
HarnessRequest<Broad, Published> + Proposal<For<Request>>
  -> CheckedProposal<Broad>
```

Current next task:

- consume the persisted published-request identity from Slice 1;
- add a request-bound proposal receipt carrier;
- check base Artifact identity, request id/hash, and surface containment before
  any child-plan projection.

Done when:

- request id/hash must match;
- base Artifact must match;
- `ResolvedTouches` must stay within `W`;
- forbidden/protected writes fail before child-plan creation;
- producer provenance is validated through `ProposalReceipt`.

Suggested tests:

- `checked_broad_proposal_accepts_matching_request_and_base_artifact`
- `checked_broad_proposal_rejects_wrong_request_hash`
- `checked_broad_proposal_rejects_forbidden_write`
- `checked_broad_proposal_rejects_router_receipt_with_wrong_artifact`

Likely test homes:

- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs)
- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/request_policy.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/request_policy.rs)
- [`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs)

### Slice 3: ChildPlan Admission

Goal:

```text
CheckedProposal<Broad>
  -> ChildPlan<For<Request>>
  -> ChildPlanFiles
```

Done when:

- parent-owned child plan carries request-bound broad evidence;
- an unbound child-plan file cannot satisfy the broad route;
- deterministic TUI evidence cannot masquerade as broad request-bound evidence.

Suggested tests:

- `broad_child_plan_requires_request_bound_surface_evidence`
- `deterministic_surface_evidence_does_not_satisfy_broad_request_route`
- `bound_checked_proposal_projects_to_child_plan_files`

Likely test homes:

- [`crates/ploke-eval/src/cli/prototype1_state/parent.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs)
- [`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs)

### Slice 4: Normal Materialization

Goal:

```text
ChildPlanFiles<bound broad evidence>
  -> derived Artifact
  -> child runtime
```

Done when the broad route uses the same downstream materialize/build/spawn path
as other admitted children, with no special trust-based shortcut.

## Invariants To Check On Every Slice

- Base Artifact identity stays explicit from request publication through child
  plan admission.
- Search breadth does not enlarge write authority.
- Protected core remains excluded from ordinary broad edit surfaces.
- Objective binding survives into selection-facing evidence.
- Proposal provenance survives into admitted child evidence.
- Parent remains the only authority that grants, checks, admits, and
  materializes.
- Traversal breadth remains orthogonal to edit authority.

## Stop Conditions

Do not claim the broad path is implemented until all of the following are true:

- `BroadHarness` no longer means both "published request" and "child-plan
  provenance";
- a published request has a stable typed identity;
- a returned proposal is checked against the same request/grant/base Artifact;
- `ChildPlanFiles` can prove request-bound broad provenance;
- complete live runs can only continue from typed admitted broad evidence.

## Immediate Next Task

Implement Slice 2 next.

Slice 1 is done. The next smallest structurally correct step is:

```text
PublishedBroadHarnessRequest + returned proposal
  -> request-bound receipt verification
  -> CheckedProposal<Broad>
```

Do not skip ahead to child-plan projection before this checked-proposal carrier
exists. That would reintroduce the same trust gap under a new name.
