# Prompt, Evidence, And Oracle Visibility Review

Date: 2026-05-12

Scope: current `ploke-eval` to broad-harness / `ploke-tui` boundary, with emphasis on what prompt is wired, what evidence is visible, how oracle/test values are exposed, how patch/application validation works, and what the bounded edit adapter still needs.

No code changes were made.

## Short Answer

If blocker 1 is handled only by converting `BroadHarnessRequest` into "some child plan", that is not enough. The correct continuation must bind a harness submission back to the published request, live admission, protected-core policy, actual candidate workspace diff, checked surface evidence, child validation, and History context.

The current implementation publishes a broad request and a markdown prompt. It does not yet run the broad request through a live `ploke-tui` adapter in complete mode.

Implemented path:

- `BroadHarnessRequest` contains the request payload: parent, workspace, edit policy, child budget, protected core pointer, evaluation brief, evidence roots, return-evidence contract, and instructions. See `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:16-26`.
- The prompt is rendered by `BroadHarnessRequest::render_prompt`, which writes parent node, source repo, mutable workspace, edit policy, child budget, evaluation, protected core, evidence roots, return-evidence fields, and instructions. See `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:930-991`.
- Publication writes both JSON and markdown prompt files under `messages/edit-harness-request`. See `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1238-1283`.
- Complete mode still stops when the broad request is produced. `run_child_fanout` receives `ParentTargetSelection::AwaitingHarnessPlan` and returns `PendingBroadHarnessRequest`. See `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6225-6235`.
- The live complete gate also rejects `BroadHarnessRequest` until a typed request-to-child-plan receipt exists. See `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:745-753`.

## Prompt Currently Wired

The actual generated prompt is:

```text
# Prototype 1 broad edit harness request

Parent node: ...
Source repository snapshot: ...
Mutable candidate workspace: ...
Edit policy: workspace except ploke-eval
Child budget: min to max

## Evaluation
- Scope: Prototype 1 descendant performance
- Selection: History-backed successor selection
- Guidance: protocol diagnoses are guidance, not hard file targets

## Protected Core
- Anchor: crates/ploke-eval/src/cli/prototype1_state/backend.rs:EVAL_CORE_SURFACE_ROOT
- Policy: workspace-except-ploke-eval authority prefixes and filenames
- Consequence: edits to protected authority are rejected before admission or prevent a valid descendant from starting

## Evidence
- submitted result output: ...
- History blocks: ...
- evaluations: ...
- node records: ...
- protocol artifacts: ...
- oracle reports: final_report.json when present

## Return Evidence
- Authority boundary: admission=not claimed, grant=not claimed, child_plan=not claimed
- what files you changed
- what evidence guided the choice
- why the change should help future evaluations
- how the change should be checked

## Instructions
- Inspect the repository and the listed evidence before choosing edits.
- Use protocol diagnoses as context about possible tool or workflow issues, not as hard file targets.
- You may edit any useful part of the allowed workspace surface outside the protected ploke-eval authority core.
- Choose edits that you judge most likely to improve descendant performance under the evaluation and successor-selection loop.
- Write the typed submitted-result evidence to <path>. This submission is evidence only; ploke-eval later checks it and may mint a ChildPlan after admission.
```

The source for that text is the renderer at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:930-991`.

This aligns with the HyperAgents posture: broad edit permission, evidence roots over large summaries, and no over-specified target. The design doc explicitly says to expose evidence and budget, describe evaluation, and let the harness choose the intervention. See `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:37-39`, `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:43-51`, and `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:116-126`.

## What Information Is Included

Included in the request/prompt:

- Parent node id: `harness_request.rs:933`.
- Source repository snapshot and mutable candidate workspace: `harness_request.rs:934-941`.
- Broad edit policy and child budget: `harness_request.rs:942-945`.
- Evaluation scope, selection authority, and guidance policy: `harness_request.rs:948-957`.
- Protected-core anchor, policy, and consequence: `harness_request.rs:959-1002`.
- Evidence roots for submitted output, History blocks, evaluations, nodes, protocol artifacts, and oracle reports: `harness_request.rs:866-910`.
- Return-evidence fields and explicit non-authority boundary: `harness_request.rs:796-819` and `harness_request.rs:911-918`.
- Instructions to inspect evidence, treat protocol diagnostics as guidance, edit outside protected core, choose likely descendant improvements, and write a typed result: `harness_request.rs:920-925` and `harness_request.rs:747-768`.

Not included yet:

- A compact latest-evaluation summary. The agent is pointed at directories, not given a specific latest evaluation report.
- Explicit "latest oracle values" or "current best oracle/test values" as structured prompt fields.
- A required validation command set. The return-evidence contract asks the harness to suggest checks, but it does not tell the harness which checks must be run.
- A typed timeout/retry policy for the harness turn.
- A typed child-validation oracle surface the harness can query before returning.
- A complete outbound `ploke-tui` request-policy receipt. The live test notes that full outbound request capture is still missing and uses unknown payload hashes. See `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1898-1917`.

## Oracle And Test Visibility

There is visibility, but it is indirect.

The broad prompt lists:

- evaluations directory;
- node records directory;
- protocol-artifacts node-scoped directory;
- oracle reports as `final_report.json when present`.

Those roots are created in `BroadHarnessRequest::prototype1_workspace`: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:866-910`.

The evaluation records do carry metrics and oracle eligibility. `Prototype1BranchEvaluationReport` stores compared instances with baseline metrics, treatment metrics, and optional evaluation result. See `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:9289-9307` and `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:9331-9343`. The operator report prints `oracle/converged/nonempty/applied` counts at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:9525-9554`.

But there is no clear "latest oracle values" carrier in the broad prompt. The harness must discover the newest useful evidence by reading the exposed roots. That matches the HyperAgents evidence-root posture, but it is weak for reliability because the agent may miss the latest decisive record.

Recommended addition:

- Add a small typed `EvidenceDigest` or `RunContext` projection, generated by `ploke-eval`, with paths and short facts only:
  - latest parent baseline report path;
  - latest compared child reports;
  - current selection metric names;
  - latest oracle/converged/nonempty/applied counts;
  - known rejected surface-attempt reasons;
  - exact validation commands expected before submission.

This should be a filesystem evidence root or request field, not prose authority.

## Child Validation Visibility

`ploke-tui` itself has a cargo tool with command timeout handling. It can run cargo check/test style commands and kills timed-out cargo processes. See `crates/ploke-tui/src/tools/cargo.rs:596-619`.

The current broad harness prompt does not explicitly instruct the agent to run child validations. It only asks the agent to include "how the change should be checked" in the submitted evidence. See `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:778-791`.

Downstream, `ploke-eval` validates children after a child plan exists. Deterministic TUI child plans are checked by `validate_requested_tui_surface_child`; broad request plans are rejected until they have a typed request receipt. See `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:757-768`.

Recommendation:

- The adapter request should include a validation contract, not just suggested checks.
- The harness may run checks for feedback, but `ploke-eval` must rerun or verify validation after candidate admission.
- The child should be able to inspect child validation outcomes through an evidence root, but not self-promote based on them.

This follows the runtime loop boundary: protocol output answers what to try, mechanized metrics and oracle signals decide keep/reject/continue, and the child cannot self-promote with prose. See `docs/workflow/evalnomicon/drafts/runtime/loop.md:95-108`.

## Patch Application And Multiple Patch Handling

Current `ploke-eval` checked surface path is conservative:

- `GitWorktreeBackend::validate_edit_surface_candidate` rejects empty proposals: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1092-1096`.
- It currently rejects multi-file edit proposals by requiring all touches to dedupe to exactly one path: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1098-1106`.
- It validates path policy, target file existence, source hash, touch spans, stale base hash, proposal producer binding, generator surface, graph bounds, staging, grant check, applied writes, and after-artifact validation: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1107-1288`.
- It mints a checked grant and `CheckedSurfaceEdit` only after those checks: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1290-1335`.

Current broad harness admission is broader:

- It verifies the submitted result is bound to the published request: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:129-188`.
- It checks the live admission binding against the published request: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1351-1364`.
- It requires the source repo to match, the candidate workspace to be isolated, the source worktree to be clean, and the candidate workspace to have the same base head: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1366-1401`.
- It diffs changed paths and rejects no-change submissions or paths outside the broad policy: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1403-1418`.
- It persists the changed files into a derived Artifact commit and records the derived artifact id and artifact surface: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1420-1441`.

Important difference:

- The checked TUI path is currently single-file.
- The broad harness admission allows multiple changed paths as long as they are inside the broad policy.
- There is not yet a typed "multi-patch bundle" carrier that proves all patches were applied together with per-file before/after hashes and validation results.

In `ploke-tui`, the semantic apply path treats semantic batch application as successful if `applied > 0`, not if all edits applied. See `crates/ploke-tui/src/rag/editing.rs:360-393`. The non-semantic path is stricter and distinguishes partial apply as failed. See `crates/ploke-tui/src/rag/editing.rs:184-203`.

Recommendation:

- For the bounded adapter, require all-or-rejected semantics for an admitted candidate bundle.
- If `ploke-tui` stages multiple edits, `ploke-eval` should lower them into a typed bundle with:
  - per-file base hash;
  - per-touch span;
  - per-file after hash;
  - one candidate Artifact id over the whole workspace result;
  - a validation record tying all touched files to the same request/run.
- Partial apply should produce rejected `surface_attempt::Evidence`, not an admitted child plan.

## Harness Failure And Return Detection

Current `ploke-tui` event machinery can detect tool return:

- Tool calls are dispatched with a per-call waiter and timeout: `crates/ploke-tui/src/llm/manager/session.rs:1502-1565`.
- The dispatcher routes `ToolCallCompleted` and `ToolCallFailed` by `request_id` and `call_id`: `crates/ploke-tui/src/llm/manager/session.rs:1568-1602`.
- `apply_code_edit` reports a structured staged result and includes `request_id`, `call_id`, `proposal_id`, staged count, applied count, files count, and preview mode in UI payload: `crates/ploke-tui/src/tools/code_edit.rs:131-203`.
- Low-level apply-code-edit failures emit `ToolCallFailed` with the same request and call id: `crates/ploke-tui/src/rag/utils.rs:227-242`.

Network/provider timeout behavior is not yet a clean adapter-level record:

- The runtime child timeout doc says parent-side post-ready timeout is missing or unclear, and recommends a shared timeout policy object. See `docs/workflow/evalnomicon/drafts/runtime/child.md:317-346`.
- `ploke-tui` has tool-call timeout handling, but the broad request does not currently carry a typed timeout budget or retry policy.

If the sub-agent cannot make an edit but does not cause a `ploke-tui` error:

- `apply_code_edit` returns an error when no proposal is staged. See `crates/ploke-tui/src/tools/code_edit.rs:203-208`.
- A broad harness submission with no actual workspace diff is rejected by `BroadHarnessNoChanges`. See `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1403-1408`.
- But a harness can still write a well-shaped submitted-result file that claims checks or rationale while producing bad/no useful changes; current authority only hardens request binding and workspace diff/admission, not semantic quality. Selection/evaluation decides usefulness later.

Recommendation:

- Make the adapter return a typed terminal state:
  - `Returned<AppliedBundle>`;
  - `Returned<RejectedAttempt>`;
  - `Returned<NoEdit>`;
  - `Returned<TimedOut>`;
  - `Returned<ToolFailed>`;
  - `Returned<InvalidSubmission>`.
- Parent continuation should consume only an admitted candidate carrier, not raw TUI event text or submitted prose.

## Implemented Behavior vs Implementation Plan

Implemented:

- Broad request JSON + markdown prompt publication.
- Request identity/hash and request-bound submitted-result path.
- Submitted result schema with request binding, changed files, guiding evidence, rationale, and suggested checks.
- Backend admission for submitted broad harness result into an `AdmittedBroadHarnessResult`.
- Deterministic checked TUI surface path producing child plans.
- Low-level TUI tool/event machinery that can stage semantic edits and report tool completion/failure.
- Live API test demonstrating a Router prompt can stage an `apply_code_edit` proposal and `ploke-eval` can lower the staged write into checked surface evidence. See `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1642-1711`, `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1727-1839`, and `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1846-1977`.

Planned / missing:

- Complete-mode broad request continuation from `Parent<AwaitingHarnessPlan>` to request-bound admitted candidate(s).
- A real `ploke-tui` adapter implementation behind the eval-owned `Harness` boundary. The current trait is marked phase-1 deferred: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:1-21`.
- Full outbound request/response capture for request-policy receipts. The live test explicitly records unknown hashes for the live path: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1898-1917`.
- Typed latest-evidence digest for oracle/test visibility.
- Typed all-or-rejected multi-patch bundle validation.
- Typed timeout/retry/terminal state across the adapter.

This is consistent with the orientation doc, which says `ploke-eval` owns grants/checks/History/admission/selection, `ploke-tui` is a harness behind a trait boundary, and TUI proposal state/logs/projections are not source truth. See `docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md:130-153`. The task queue also says the real `ploke-tui` adapter comes after authority-side contract fixtures: `docs/active/agents/2026-05-08_bounded-edit-surface-implementation-orientation.md:236-244`.

## Wireframe Plan

1. Define an eval-owned adapter request carrier.

   Carrier shape: `adapter::Request<Broad, Published>` or equivalent, minted from `request::Request<Broad, Published>` plus live `EditSurfaceAdmission`, evidence digest, timeout policy, and validation contract.

   It should preserve:

   - request identity/hash;
   - parent identity;
   - candidate workspace;
   - broad policy;
   - protected-core pointer;
   - evidence roots plus compact latest-evidence digest;
   - allowed tool set or harness capability;
   - return schema;
   - timeout/retry policy.

2. Add `EvidenceDigest` as a typed projection, not authority.

   This should summarize latest useful facts and point at source records:

   - latest baseline/evaluation paths;
   - current metric/oracle fields;
   - latest rejected surface attempts;
   - protocol artifact roots;
   - child validation commands expected by policy;
   - current generation/node budget.

3. Implement the `ploke-tui` harness adapter behind the eval-owned trait boundary.

   It should:

   - feed the prompt/request into `ploke-tui`;
   - listen for terminal `ChatTurnFinished`, `ToolCallCompleted`, and `ToolCallFailed`;
   - collect staged proposal ids and tool-call ids;
   - refuse terminal "success" unless an edit proposal or explicit no-edit result is returned;
   - capture request-policy payload hashes where possible.

4. Lower TUI proposal output into eval-owned checked evidence.

   Do not admit TUI proposal state directly. Convert to:

   - resolved touches;
   - generator-surface provenance;
   - request-policy receipt;
   - all-or-rejected apply bundle;
   - `surface_attempt::Evidence`.

5. Add multi-patch bundle semantics.

   The first admitted broad continuation should either:

   - support one changed file and reject the rest explicitly, or
   - introduce a bundle carrier that validates all touched files under one candidate Artifact transition.

   Do not rely on `ploke-tui` semantic apply's `applied > 0` success rule for admission.

6. Add adapter terminal states and timeout policy.

   Parent should observe:

   - returned applied bundle;
   - returned rejected attempt;
   - no edit;
   - tool failed;
   - model/provider timeout;
   - adapter timeout;
   - invalid submission.

   These should become durable attempt evidence where relevant, not only logs.

7. Continue complete mode only from admitted carriers.

   The continuation should be:

   ```text
   request::Request<Broad, Published>
     + adapter::Returned<...>
     + backend admission/check
     -> Parent<ChildPlanReady> or admitted candidate Artifact(s)
   ```

   Complete mode should never consume raw prompt prose, TUI CLI text, or unvalidated submitted-result claims as child authority.

8. Splice tests before live run.

   Required tests:

   - published request + evidence digest renders prompt with latest oracle/test paths;
   - TUI adapter returns staged proposal and terminal state;
   - no-edit terminal becomes rejected attempt evidence;
   - tool failure/timeout becomes rejected attempt evidence;
   - partial multi-edit apply is rejected;
   - all-patch bundle admits only when all touched files validate;
   - broad request continuation mints child plan/admitted candidate only from request-bound evidence.

## Bottom Line

Handling blocker 1 correctly will make the live run plausible, but only if the fix is the request-bound adapter continuation described above. The current prompt is already broadly aligned with HyperAgents. The missing part is not prompt wording; it is the typed continuation that makes `ploke-tui` output into checked, request-bound, validation-aware evidence before the parent runtime treats it as runnable child material.
