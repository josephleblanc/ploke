# 2026-05-12 HyperAgents Broad Harness Orchestration Handoff

Status: active post-compaction plan.

This handoff is for moving Prototype 1 from the deterministic TUI edit-surface
proxy toward the HyperAgents-style broad child harness while preserving the
authority model in
`docs/workflow/evalnomicon/drafts/edit-surface/model.md`.

Ignore `docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md`
for this thread. It is stale relative to the edit-surface model and the current
mandatory grant authority implementation.

## Source Documents

- `docs/workflow/evalnomicon/drafts/edit-surface/model.md`
  Source of truth for `Runtime -> Surface(Artifact) -> derived Artifact`,
  `SurfaceGrant`, `EditHarness`, `SurfaceCheck`, and History admission.
- `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md`
  Prompt posture: expose evidence and budget, let the harness choose the
  intervention, and keep protected-core authority in `ploke-eval`.
- `docs/workflow/evalnomicon/chat-history/on-hyper-agents.md`
  Archive/traversal interpretation: do not collapse the run to one current-best
  lineage.
- `.agents/prototype1-hyperagents-handoff-2026-05-06.md`
  HyperAgents-to-Prototype-1 translation, policy-bearing surface correction, and
  staged unlock model.
- `.agents/hyper-agents.txt`
  Paper text. Appendix A.1 is especially relevant: the initial meta-agent prompt
  is small because the outer loop owns archive, evaluation, admission, and
  selection.

## Causal Frame

Surface Request:
Implement the next broad edit-harness path after the deterministic smoke run.

Causal Chain:
Parent Runtime grants an Artifact surface, harness proposes edits, `ploke-eval`
checks them, backend realizes a derived Artifact, History admits the transition,
Child Runtime evaluates the result.

Concern:
The current complete run path is deterministic and narrow. The current
`BroadHarness` path publishes a request and then fails closed, so it has the
right prompt posture but no typed result/admission path.

Evidence Surface:
Typed request, typed harness result, checked surface evidence, backend Artifact
identity, History records, child evaluation records, and later selection input.

Existing Algebra:
`SurfaceGrant`, mandatory `GrantAuthority`, `EditSurfaceAdmission`,
`CheckedSurface`, `PublishedBroadHarnessRequest`, Parent typestates, ChildPlan
messages, History surface evidence.

Missing Structure:
A request-bound broad harness result, an isolated harness workspace/candidate
Artifact, and a Parent-owned receiver that validates the result before creating
or accepting a ChildPlan.

Transformation:
Turn `BroadHarnessRequest<Published>` plus harness output into checked
Artifact transitions through `ploke-eval`, not through an unbound child-plan
file.

Projection:
CLI/status may show pending request paths and admitted/rejected harness results,
but those views are not authority.

Preservation Check:
No harness output, prompt text, TUI state, branch name, or child-plan file may
self-authorize admission. Admission must cite request binding, runtime/artifact
coordinate, grant/policy, checked patch evidence, and derived Artifact identity.

## Design Position

The next useful object is a broad child-improvement protocol:

```text
Request<Published>
  -> HarnessWorkspace
  -> Result<Submitted>
  -> SurfaceCheck<Passed | Rejected>
  -> ArtifactTransition
  -> ChildPlan
```

The tempting reduction is to let `BroadHarness` consume any existing
`ChildPlan`. That repeats the exact bug class we just removed: an output file
would stand in for the authority chain. A child plan may be the output of an
admitted broad harness result, but it should not be the input authority.

The other tempting reduction is to keep expanding deterministic TUI generation.
That is useful as a regression canary, but it is not the HyperAgents experiment.
The child harness should choose what to edit from evidence inside the allowed
surface.

## Model-Alignment Constraints

Preserve a typed `create -> check -> admit -> evaluate -> traverse` procedure
over an archive of Artifacts/Runtimes. Do not model this as a smarter file
picker.

Reject these reductions during implementation:

- protocol diagnosis becomes deterministic target-file selection;
- `SurfaceGrant` becomes prompt prose, config lists, or duplicated
  protected-core text;
- TUI proposal state, CLI text, previews, or harness telemetry become authority;
- child generation, archive admission, evaluation, and parent selection collapse
  into one successor-local decision;
- untyped evidence recovery returns through `serde_json::Value`, filenames,
  path identity, or degraded fallback;
- traversal or policy-bearing `ploke-eval` code becomes ordinary child edit
  scope before an explicit unlock/protocol-upgrade transition.

Keep archive membership, validity, evaluation, and next-parent selection as
distinct states. BroadHarness may choose targets inside the allowed Artifact
surface, but `ploke-eval` owns grant issuance, containment checks, admission,
and History evidence.

## Current Behavior

- `DeterministicTuiTools` is the only live-complete candidate generator that is
  currently admitted.
- `BroadHarness` publishes a typed `PublishedBroadHarnessRequest` with
  request-scoped `request_path`, `prompt_path`, `submitted_result_path`, and
  isolated `workspace_path`, then returns `PendingBroadHarnessRequest`.
- Re-publication for the same parent allocates a new request family such as
  `<parent-node>-r2` if any prior request JSON, prompt, result path, or
  workspace already exists.
- `BroadHarness` rejects existing child plans because there is no typed
  admitted receiver yet.
- The request still names the source repository as read/context material, but
  the mutable harness workspace is a request-bound isolated candidate workspace.
- `Parent<AwaitingHarnessPlan>` carries the published request identity
  structurally, not through an optional generic parent field.
- Backend admission for `SubmittedBroadHarnessResult` now exists in
  `backend.rs` and is awaiting independent review before CLI/History consume it.

## Implementation Lanes

### Lane 0: Smoke Observation

Purpose:
Use the current deterministic smoke run only as a regression canary for the
mandatory authority chain.

Work:
Read metadata-first health signals only. If the smoke fails in grant/check/
History authority, fix that before broad-harness implementation. If it passes,
do not start a long deterministic run merely for more samples.

Acceptance:
The smoke either passes or yields a concrete authority-chain bug with bounded
evidence.

### Lane 1: Request And Workspace Boundary

Owned files:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
- new `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs`
  if the result type is split from request publication.

Work:
Define the broad harness output contract without making it authority.

Required structure:

- `PublishedBroadHarnessRequest` remains the request publication carrier.
- Add a typed result carrier bound to `request_id`, `request_hash`,
  `parent_node_id`, and the expected output location.
- Add an isolated harness workspace/candidate artifact path to the request or
  result protocol. Do not let the active Parent worktree be the mutable child
  workspace.
- Preserve evidence roots, budget, protected-core pointer, and return-evidence
  fields from the HyperAgents prompt posture.

Acceptance:
Tests can deserialize a published request and submitted result, reject request
hash mismatch, and show that the result names a candidate workspace/output
without claiming admission.

Current status:
Complete and reviewed after corrective passes. The published request owns a
typed request JSON path, prompt path, submitted-result path, and isolated
workspace path. The request/result carrier exposes no `child_plan_path()` or
`output_path()` alias. The submitted result carries typed non-authority return
evidence and verifies request/workspace/result binding.

### Lane 2: Broad Result Admission

Owned files:

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- possibly a narrow new module under
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/`

Work:
Implement `ploke-eval`-owned validation for a submitted broad harness result.

Required checks:

- result binds to the published request;
- changed paths are under the target Artifact root;
- protected core remains unchanged;
- expected/base hashes match;
- material writes are within the granted broad surface;
- derived Artifact identity is computed by the backend;
- rejected results preserve diagnostic evidence without creating children.

Acceptance:
Splice tests cover an outside-protected-core edit accepted, a protected-core
edit rejected, a stale base rejected, and a request mismatch rejected.

Current status:
Implemented as `bh-backend-admit-broad-result-v3` and pending independent
review. Do not wire CLI/History consumers until that review accepts the backend
carrier and checks.

### Lane 3: History Evidence

Owned files:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs`

Work:
Persist the broad harness check/admission evidence as typed History-facing
records.

Required structure:

- request identity and hash;
- harness run/result identity;
- coordinate and grant/policy;
- changed path set or patch set;
- base and derived Artifact refs;
- check result and rejection reason when rejected.

Acceptance:
History evidence can represent accepted and rejected broad harness results
without `serde_json::Value` field walking and without reusing deterministic TUI
producer IDs as broad harness authority.

### Lane 4: Parent/CLI Integration

Owned files:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs` if a profile is added.

Work:
Change `BroadHarness` from request-only fail-closed to a two-stage receiver:

```text
no request/result exists -> publish request and return PendingBroadHarnessRequest
request exists, result missing -> return PendingBroadHarnessRequest
request/result exists -> validate result, admit checked children, write ChildPlan
```

The profile should remain disabled for live-complete until this receiver exists.
After it exists, add a small `broad-harness-smoke` profile rather than an
overnight broad run.

Acceptance:
`BroadHarness` no longer consumes an unbound child plan. It consumes only a
typed request-bound result and produces a ChildPlan after admission.

Current status:
Publication/pending behavior is corrected and reviewed. Receive/admit/write
ChildPlan behavior is still pending backend review and the next CLI/History
wave.

### Lane 5: Harness Driver Or Manual Bridge

Owned files:

- Prefer a small new test/mock harness module before touching live operator
  scripts.
- If an operator command is added, keep CLI as dispatch/projection only.

Work:
Provide a minimal producer for `Result<Submitted>` so the broad path is testable.
This can start as a mock/manual bridge that copies the parent Artifact to an
isolated workspace, applies a simple safe edit outside protected core, and writes
the typed result.

Acceptance:
A test or smoke command can exercise:

```text
publish request -> write submitted result -> receive/admit result -> child plan
```

without reading rendered CLI output as authority.

### Lane 6: Review

Owned files:

- none.

Work:
Run an independent review after the first full broad result admission slice.

Review questions:

- Can any path still create children from an unbound ChildPlan?
- Can the harness mutate the active Parent checkout and have that treated as
  authority?
- Does History cite request, coordinate, grant, policy, transition, and check?
- Are rejected results durable evidence without being selection authority?
- Are prompt text and CLI views still projections only?
- Are admitted-but-not-selected children, rejected children, and invalid
  proposals structurally distinct?
- Does any evidence code depend directly on traversal policy types?

Acceptance:
No high-severity authority, provenance, typed-persistence, or role/state naming
findings remain.

### Lane 7: Archive Evidence Preparation

Owned files:

- likely `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`
- likely selection-facing modules under
  `crates/ploke-eval/src/successor_selection/`

Work:
After broad result admission exists, prepare the evidence boundary for
HyperAgents-style archive traversal. This is not the first implementation wave.

Required structure:

- a named typed record that can carry sparse external outcome and dense
  tool/process evidence;
- a selection-facing projection that keeps evidence storage policy-agnostic;
- separate states for admitted archive member, rejected result, invalid
  proposal, selected successor, and explored-from candidate.

Acceptance:
Selection can consume a typed projection over admitted archive evidence without
reading mutable reports or depending on raw History internals.

## Orchestrator Setup After Compaction

Use these skills first:

- `semantic-architecture`
- `causal-algebra-design`
- `light-thread-orchestrator`
- `orchestrator-conveyor`
- `typed-persistence-spine`
- `structural-naming`

Start with the board, but do not rely on stale blocked tasks from earlier edit
surface waves. Use fresh task IDs with `bh-` prefix.

Suggested workers:

- `bh-model-retainer`: read-only model/doc alignment.
- `bh-code-scout`: read-only BroadHarness code map.
- `bh-request-worker`: request/result/workspace types.
- `bh-backend-worker`: broad result validation and backend admission.
- `bh-history-worker`: typed History evidence.
- `bh-cli-worker`: Parent/CLI receiver and profile gating.
- `bh-reviewer`: independent review.

Suggested first board tasks:

```text
bh-map-current-broad-harness
bh-result-contract
bh-isolated-workspace-boundary
bh-backend-admit-broad-result
bh-history-broad-result-evidence
bh-cli-receive-bound-result
bh-broad-harness-mock-driver
bh-final-authority-review
```

Do not assign two workers to the same file family at the same time. In
particular, `cli_facing.rs` should stay single-owner for a wave.

## Verification Commands

Use bounded output:

```bash
cargo check -p ploke-eval 2>&1 | tail -n 100
cargo test -p ploke-eval broad_harness 2>&1 | tail -n 120
cargo test -p ploke-eval edit_surface 2>&1 | tail -n 120
rg -n "grant: None|authority: None|Option<[^>]*GrantAuthority|validate_received_child_plan\\(|BroadHarness" crates/ploke-eval/src/cli/prototype1_state
```

For live loop observation, use metadata-first checks from the
`prototype1-loop-runtime` skill. Do not inspect JSONL or journals broadly.

## Stop Conditions

Pause and ask for direction if:

- broad harness admission requires ordinary descendants to edit `ploke-eval`
  authority code;
- a worker proposes accepting plain ChildPlan files as authority;
- implementation requires parsing projection text rather than typed records;
- the active deterministic smoke exposes an authority-chain regression;
- lane ownership becomes ambiguous.

## Success Definition

The next meaningful milestone is not a long deterministic run. It is a broad
harness smoke where:

```text
Parent publishes request
harness edits isolated candidate workspace outside protected core
harness writes typed request-bound result
Parent validates result under grant/policy
backend computes derived Artifact identity
History records checked transition
Parent writes ChildPlan for admitted children
```

At that point the live loop can begin testing the HyperAgents posture: broad
child agency inside a typed authority envelope, with the outer loop retaining
archive, evaluation, admission, and selection authority.
