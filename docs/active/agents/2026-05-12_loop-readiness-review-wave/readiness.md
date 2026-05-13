# Loop Readiness Review

Role: loop-readiness reviewer
Date: 2026-05-12

Scope: recent Prototype 1 structural-carrier and broad-harness changes, reviewed for readiness to run a long loop such as 15 generations and 128 max nodes.

Code changes made by this reviewer: none.

## Verdict

Not ready for an unattended long loop using the current default broad-harness path. The broad-harness request carrier work is moving in the right direction, but the live `Complete` path still hard-stops before child planning for `BroadHarnessRequest`, and the request-to-child-plan receipt/admission path is not yet wired into continuation. A long loop is only plausible after a clean-worktree gate and either:

- selecting `DeterministicTuiTools` explicitly for the run, or
- implementing the typed broad-harness submitted-result -> admitted child-plan/child-artifact transition.

## Findings

### Blocker: Default complete runs select broad harness, but complete mode rejects broad harness before child planning

`Prototype1StateCommand` defaults `candidate_generator` to `BroadHarnessRequest` in `crates/ploke-eval/src/cli.rs:634-636`. Run profiles also default generation source to broad harness in `crates/ploke-eval/src/cli/prototype1_state/profile.rs:157-161`.

The live complete path then rejects that generator: `CandidateGenerationConfig::ensure_live_complete_admitted` returns an error for `BroadHarnessRequest` with the message that the typed request-to-child-plan receipt is not implemented in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:745-753`. The complete run calls that gate at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8171-8175`.

For a 15-generation / 128-node long loop, this means the default configuration does not start an autonomous complete run. It intentionally stops at the broad-harness request boundary.

### Blocker: Current worktree is not runnable as-is for broad-harness admission

`WorkspaceBackend::admit_submitted_broad_harness_result` rejects a dirty source repository before admitting a submitted broad-harness result at `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1385-1390`.

Current `git status --short --untracked-files=all` reports dirty `ploke-eval` files, dirty unrelated `ploke-egui` files, untracked UI docs, and untracked `xtask/src/commands/check.rs`. A long loop should run from a clean parent worktree or a deliberately prepared campaign worktree. Otherwise admission can fail for operational dirt, and the run evidence is harder to interpret.

This also matters for the History invariant that ordinary descendants preserve the policy-bearing `ploke-eval` surface. The invariant is stated in `crates/ploke-eval/src/cli/prototype1_state/mod.rs:111-127` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:151-185`.

### High: Broad-harness result admission exists, but it is not connected to loop continuation

The backend can validate and admit a submitted broad-harness result, producing `AdmittedBroadHarnessResult` after request binding, clean/stale-base checks, surface checks, and commit persistence in `crates/ploke-eval/src/cli/prototype1_state/backend.rs:471-525` and `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1338-1441`.

However, the live target-selection path still emits `PendingBroadHarnessRequest` and returns an error rather than a `ChildPlan` in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6226-6235`. Existing child plans are explicitly rejected for `BroadHarnessRequest` unless a typed request receipt exists, at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:757-762`.

This is aligned with the current safety posture, but it is a long-loop blocker: writing a `SubmittedBroadHarnessResult` is not enough unless there is a follow-on transition that mints a request-bound child plan or admitted child artifact for the loop to consume.

### Medium: Request typestate exists, but crate-visible fields still allow bypassing the intended publication transition

The new request carriers are useful: `request::Request<request::Broad, request::Published>`, `request::Reference<_, request::Published>`, `request::Identity`, `request::Hash`, and `request::Binding` live in `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:35-252`. `Parent<AwaitingHarnessPlan>` stores a published request reference at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:44-52`, and the transition consumes that reference at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:981-1003`.

The remaining risk is that `request::Request<K, S>` exposes its core fields as `pub(crate)` in `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:212-221`. There is also a crate-visible `with_admission_binding` mutator that can rewrite the binding and recompute the hash after publication at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:1090-1096`.

This does not currently fail the focused tests, but it weakens the module-level invariant that live values should be built through trusted loaders/transitions rather than ad hoc literals, described in `crates/ploke-eval/src/cli/prototype1_state/mod.rs:7-9`.

### Medium: Grant typestate is structurally better, but the durable coordinate sum remains untagged

The grant carrier work is a net improvement. `grant::Grant<Checked>`, `grant::Grant<Admitted>`, and `grant::Coordinate<S>` are live carriers in `crates/ploke-eval/src/cli/prototype1_state/history.rs:2925-3225`. The checked-to-admitted transition validates policy, writable path, target artifact, and runtime identity in `crates/ploke-eval/src/cli/prototype1_state/history.rs:3090-3135`.

The durable projection still serializes `grant::AnyCoordinate` with `#[serde(untagged)]` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:2950-2955`. The current record shapes are distinguishable, but an untagged authority-state projection is fragile for long-lived history records. The History docs stress that durable records are projections of authority transitions, not loose status blobs, at `crates/ploke-eval/src/cli/prototype1_state/history.rs:203-208`.

This is not a short-run blocker, but before relying on many generations of persisted surface evidence, I would prefer an explicitly tagged record form or a documented compatibility reason.

### Low: One new invariant test uses `#[should_panic]`

The runtime-mismatch grant test uses `#[should_panic]` in `crates/ploke-eval/src/cli/prototype1_state/history.rs:9049-9051`. The project review policy prefers explicit error assertions over panic tests. The underlying behavior is important enough that the test should check the `HistoryError` path directly, so a future panic does not masquerade as the intended rejection.

## Positive Evidence

- Request binding is checked against the submitted result and the live admission before repo checks in `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1345-1364`.
- Broad-harness admission rejects non-isolated workspaces, dirty source repos, stale candidate bases, empty changes, and out-of-surface changes in `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1366-1418`.
- Complete-mode node-budget guards exist before child planning at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8176-8188`, and `reserve_complete_child_budget` caps child budget by remaining node slots at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7015-7039`.
- Continuation checks enforce generation, total-node, direct-child, and traversal-budget decisions in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6869-6941`.
- The HyperAgents prompt posture is reflected in the broad request shape: evidence roots, budget, protected-core pointer, and broad permission are favored over a deterministic target-file picker, matching `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:37-64` and `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:87-93`.

## Verification Run

- `cargo check -p ploke-eval 2>&1 | tail -n 80`
  - Passed.
  - Tail showed `ploke-eval` generated 307 warnings.
- `cargo test -p ploke-eval broad_harness_ 2>&1 | tail -n 80`
  - Passed: 13 tests.
  - Output includes fixture-local git commit messages such as `prototype1 broad harness result ...`; I saw no evidence that these touched the main repo.
- `cargo test -p ploke-eval edit_surface 2>&1 | tail -n 80`
  - Passed: 76 tests.
- `cargo run -p xtask -- check structural-naming --report-only 2>&1 | tail -n 80`
  - Passed: `Structural naming check passed: 117 Rust file(s), threshold 3`.
- `git status --short --untracked-files=all`
  - Dirty. See blocker above.
- `git check-ignore -v .orchestrator-archive/bh-placeholder`
  - Confirms `.orchestrator-archive/` is ignored by `.gitignore:98`.

## Recommended Gate Before Any Long Loop

1. Clean or isolate the run worktree. Do not run from the current dirty checkout.
2. Run a short complete loop with `--candidate-generator deterministic-tui-tools`, `--max-generations 1`, and a small node cap.
3. If that succeeds, run a 2-generation complete loop with the intended child budget and `--max-total-nodes` well below 128.
4. Only then run 15 generations / 128 nodes.
5. For broad harness specifically, do not attempt an unattended long loop until the typed submitted-result admission path mints a loop-consumable child plan or admitted child artifact.

## Open Questions

- Is the next long loop intended to exercise the broad harness, or is `DeterministicTuiTools` acceptable for the long-loop readiness run?
- Should `request::Request<K, S>` fields be private before the next run, or is the current crate-visible compatibility surface intentionally temporary?
- Should `grant::AnyCoordinate` durable JSON remain untagged for backwards compatibility, or can it move to an explicitly tagged `record` projection before records accumulate?

## Residual Risk

The structural-carrier work improves request and grant modeling, but the current implementation is still in a transition state: broad harness is a pending request protocol, not an autonomous generation source. The largest operational risk is mistaking a passing unit-test cluster for readiness to run an unattended multi-generation campaign.
