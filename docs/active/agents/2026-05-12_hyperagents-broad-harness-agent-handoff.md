# 2026-05-12 HyperAgents Broad Harness Agent Handoff

Status: restart handoff for the next agent/orchestrator.

Use first:

- `semantic-architecture`
- `causal-algebra-design`
- `light-thread-orchestrator`
- `orchestrator-conveyor`
- `typed-persistence-spine`
- `structural-naming`

Primary model doc:

- `docs/workflow/evalnomicon/drafts/edit-surface/model.md`

Active orchestration doc:

- `docs/active/agents/2026-05-12_hyperagents-broad-harness-orchestration-handoff.md`

Ignore:

- `docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md`

## Current Board State

Board file:

- `.orchestrator/board.json`

No `bh-*` worker is currently active.

Important reports:

- `.orchestrator/reports/bh-request-publication-json-allocation-fix.md`
- `.orchestrator/reports/bh-review-backend-admission-v3.md`
- `.orchestrator/reports/bh-request-bind-admission-authority.md`
- `.orchestrator/reports/bh-backend-admit-broad-result-v3.md`

Accepted/reviewed slices:

- request-scoped publication family;
- submitted-result path instead of child-plan output;
- no `child_plan_path()` / `output_path()` request/result aliases;
- structural `Parent<AwaitingHarnessPlan>` request identity;
- direct non-optional parent request-hash comparison.

Rejected/not accepted:

- `bh-backend-admit-broad-result-v3`.
- `bh-request-bind-admission-authority` is complete-unreviewed and insufficient
  as implemented because the binding is optional and not live-wired.

## Files To Know

Primary changed files:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`

Durable docs added/updated:

- `docs/active/agents/2026-05-12_hyperagents-broad-harness-orchestration-handoff.md`
- `docs/active/agents/2026-05-12_hyperagents-broad-harness-progress-summary.md`
- `docs/active/agents/2026-05-12_hyperagents-broad-harness-agent-handoff.md`

## Current Invariants

Do not weaken these:

- BroadHarness must not consume raw `ChildPlan` authority.
- Request JSON, prompt, submitted-result path, and workspace are one
  request-scoped publication family.
- A submitted result is not admission, not a grant, and not child-plan
  authority.
- `Parent<AwaitingHarnessPlan>` must carry request identity structurally.
- Backend must derive Artifact identity only after request binding, admission
  authority binding, workspace/base/protected-core checks, and policy checks.
- History/CLI must not consume backend results until backend admission is
  accepted by review.

## Immediate Next Step

Fix the authority gap.

Problem:

```text
admit_submitted_broad_harness_result(
  repo_root,
  admission: EditSurfaceAdmission,
  published: PublishedBroadHarnessRequest,
  submitted: SubmittedBroadHarnessResult,
)
```

currently verifies `submitted -> published`, but does not prove `published ->
admission`.

The next correct slice should make this impossible:

```text
PublishedBroadHarnessRequest authority binding
  == EditSurfaceAdmission coordinate/policy/base artifact
```

Concrete requirements:

- Request-time admission binding must be mandatory for any live/admittable
  request path.
- The binding must cover enough identity to reject unrelated admissions:
  runtime coordinate, operation target/base artifact identity, and policy id.
- Backend must reject missing or mismatched request/admission binding before
  persisting changes or deriving `AdmittedBroadHarnessResult`.
- Add tests for mismatched admission authority, missing admission binding,
  workspace isolation, and source repository mismatch.

Suggested lane split:

- `bh-request-authority-required`: tighten request/result carrier so admittable
  published requests carry mandatory coordinate/target/policy identity.
- `bh-cli-publish-authority-bound-request`: wire live BroadHarness publication
  to construct that binding from the actual parent/runtime/artifact authority.
- `bh-backend-enforce-request-admission-binding`: reject missing/mismatched
  binding in `backend.rs`.
- `bh-review-request-backend-authority`: independent read-only review before
  CLI receiver or History work continues.

Do not assign backend and request workers to overlapping files. If the carrier
shape is uncertain, do the request carrier first, then backend.

## Commands

Use bounded output:

```bash
cargo check -p ploke-eval 2>&1 | tail -n 100
cargo test -p ploke-eval broad_harness_ 2>&1 | tail -n 100
cargo test -p ploke-eval edit_surface 2>&1 | tail -n 120
cargo test -p ploke-eval submitted_broad_harness_result 2>&1 | tail -n 100
rg -n "child_plan_path\\(|output_path\\(|PartialEq<Option<&str>>|with_admission_binding|admission_binding" crates/ploke-eval/src/cli/prototype1_state
```

For smoke-loop observation, stay metadata-first. Do not read journals or JSONL
payloads unless diagnosing a concrete anomaly.

## Stop Conditions

Stop and ask if:

- the next implementation proposes accepting unbound `ChildPlan`;
- request/admission binding remains optional in an admitted path;
- backend wants to infer authority from mutable paths instead of typed request
  and admission carriers;
- History work is proposed before backend admission is reviewed;
- two workers need the same file family.

