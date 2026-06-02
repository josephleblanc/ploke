# Prototype 1 Broad Harness Request/Plan Erasure

Status: partially resolved by `bd00056b`; residual proof/durability gaps remain
Discovered: 2026-05-12

## 2026-05-13 Resolution Note

Commit `bd00056b Add broad harness request fanout` fixes the immediate live
complete-run failure shape:

- `BroadHarnessRequest` is admitted for complete runs;
- one published broad request slot is allocated per child-budget slot;
- each slot is bound to the live request/admission context;
- headless `ploke-tui` can produce the submitted result;
- backend admission rechecks request binding, live admission binding, source
  cleanliness, base staleness, path policy, non-empty diff, and unexpected dirty
  paths before deriving a child artifact transaction;
- broad child plans are constructed from admitted transactions, not from an
  unbound existing child-plan file.

The original bug remains useful as a structural warning. The still-open parts
are not the old hard stop, but the longer-term proof boundary: verified loading
of published request records, durable grant projection shape, and replacing the
current broad workspace-diff admission path with the full formal
`SurfaceGrant`/`CheckedProposal` spine where appropriate.

## Summary

Original pre-`bd00056b` failure: `BroadHarness` was modeled as a normal
candidate generator even though the implementation only published an external
harness request and then stopped with `PendingBroadHarnessRequest`. On resume,
an existing child plan could be accepted without being bound to the
`BroadHarnessRequest` that was published.

This collapses the semantic distinction between:

- a deterministic TUI child plan generated inside `ploke-eval`;
- a broad external harness request waiting for a response;
- a future Router-backed/ploke-tui harness response with request-policy receipt.

The result is a "trust me" boundary in the edit-surface path: child-plan
validation is keyed by flat generator names and booleans instead of by a typed
request/response/provenance carrier.

## Affected Files

- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`

## Failure Shape

The collapsed conversion chain is:

```text
GenerationSource::BroadHarness
  -> Prototype1CandidateGenerator::BroadHarness
  -> CandidateGenerationConfig::BroadHarness
  -> requires_surface_evidence() == true
  -> validate_requested_tui_surface_child(...)
```

That erases whether the child plan was produced by deterministic TUI machinery,
by a published broad request, or by a future Router-backed harness.

Separately, `publish_broad_harness_child_plan_request` writes request/prompt
files and then returns a `PrepareError`. That means the pending request state is
represented as an error string rather than a typed transition such as:

```text
Parent<Ready> -> Parent<AwaitingHarnessPlan<Broad>>
```

## Current Mitigation / Remaining Boundary

The live complete-run path now admits `BroadHarnessRequest` only through the
request-batch continuation implemented in `bd00056b`. It must continue to reject
any path that tries to satisfy broad harness with an unbound existing child
plan, raw submitted JSON, or prompt output.

Existing child plans must not satisfy `BroadHarness` without a receipt that binds
at least:

- parent identity;
- request identity/hash;
- edit policy;
- objective;
- granted surface/protected core;
- proposal producer/provenance;
- child plan identity and candidate evidence.

Deterministic TUI child plans must reject Router-backed proposal provenance
unless that path is modeled as a distinct typed harness/provenance carrier.

## Correct-By-Construction Direction

Do not continue expanding flat `Broad*` names. The missing structure should be
modeled directly:

```text
HarnessRequest<Broad, Published>
Parent<AwaitingHarnessPlan<Broad>>
ChildPlan<For<HarnessRequest<Broad, Published>>>
SurfaceEvidence<DeterministicTuiTools>
SurfaceEvidence<RouterBackedHarness>
Proposal<Checked, Grant>
```

The broad harness route should be a projection from admitted `EditObjective`,
`SurfaceGrant`, request-policy, and artifact identity, not a standalone prompt
DTO that later relies on caller discipline.

## Verification Targets

- A complete live run with `source = "broad-harness-request"` publishes request
  slots, admits request-bound transactions, and seals children only after the
  configured minimum admission count.
- `BroadHarness` rejects an existing unbound child plan.
- Deterministic TUI validation rejects Router-backed proposal provenance.
- Future broad harness child plans cannot be constructed without
  request/admission-bound evidence.
