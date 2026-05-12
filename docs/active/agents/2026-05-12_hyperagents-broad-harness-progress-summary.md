# 2026-05-12 HyperAgents Broad Harness Progress Summary

Status: progress summary after the request/publication and backend-admission orchestration wave.

Source of truth:

- `docs/workflow/evalnomicon/drafts/edit-surface/model.md`
- `docs/active/agents/2026-05-12_hyperagents-broad-harness-orchestration-handoff.md`

Stale plan to ignore:

- `docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md`

## What Changed

The BroadHarness request path is no longer a prompt that asks a harness to write
a child plan.

Implemented and reviewed:

- `PublishedBroadHarnessRequest` now owns a typed request family:
  request JSON path, prompt path, submitted-result path, isolated workspace
  path, request id, and request hash.
- Re-publication for the same parent allocates a new request family such as
  `<parent>-r2` if any prior request JSON, prompt, submitted-result path, or
  workspace exists.
- `SubmittedBroadHarnessResult` is a typed non-authority result carrier. It
  binds back to the published request and carries typed return evidence:
  change summary, guiding evidence, rationale, and suggested checks.
- `child_plan_path()` and `output_path()` aliases were removed from the
  request/result carrier surface.
- `Parent<AwaitingHarnessPlan>` now owns the request identity structurally in
  its typestate instead of storing an optional request on every `Parent<S>`.
- Optional request-hash compatibility was removed from the parent request hash
  wrapper.
- BroadHarness CLI publication now tells the harness to write a typed
  `SubmittedBroadHarnessResult`, not a `ChildPlan`.

Implemented but not accepted:

- Backend admission for `SubmittedBroadHarnessResult` was implemented in
  `backend.rs`, with tests for accept, protected-core rejection, stale-base
  rejection, and request mismatch.
- Review rejected the backend slice because the published request is not yet
  live-bound to the `EditSurfaceAdmission` authority that backend stamps onto
  the admitted result.

Partially implemented and still unsafe to rely on:

- `RequestAdmissionBinding` exists on the request/result carriers and affects
  request hash, but it is optional and is not wired into the live CLI
  publication path.
- The next fix must make request/admission authority binding mandatory for any
  backend admission path.

## Verification Run

Successful checks during the wave:

```bash
cargo check -p ploke-eval
cargo test -p ploke-eval edit_surface
cargo test -p ploke-eval broad_harness_
cargo test -p ploke-eval request_json_alone_reserves_publication_identity
cargo test -p ploke-eval broad_workspace_edit_surface_republication_uses_request_scoped_family_paths
cargo test -p ploke-eval repeated_publication_for_same_parent_gets_request_scoped_identity
cargo test -p ploke-eval submitted_broad_harness_result
```

The backend-specific tests passed before review, but the backend slice is still
not accepted because the authority model is incomplete.

## Current Blocker

The blocking issue is authority binding:

```text
PublishedBroadHarnessRequest
  -> SubmittedBroadHarnessResult
  -> EditSurfaceAdmission
  -> AdmittedBroadHarnessResult
```

The request/result bind to each other, but the backend can still be passed an
unrelated `EditSurfaceAdmission`. That can stamp an otherwise valid
request/result with the wrong coordinate, policy, and base artifact.

The correct next fix is not a CLI workaround. The request publication must carry
mandatory admission-authority identity, and backend must reject mismatches
before deriving or persisting an admitted result.

## Smoke Loop

The deterministic smoke loop was observed only through metadata-first checks.
Latest observed status during this work:

```text
12 succeeded
5 running
~270G free under ~/.ploke-eval
```

No journal or JSONL payloads were read for routine health checks.

