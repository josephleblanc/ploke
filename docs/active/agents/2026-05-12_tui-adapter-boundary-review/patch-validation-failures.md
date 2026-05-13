# Patch Application, Validation, And Failure Handling Review

Date: 2026-05-12

Scope: bounded discovery only. No code edits were made.

Question: how patches are applied together, how multiple touched files are represented and validated together, how clean/dirty/stale-base/protected-core/surface checks work, how `ploke-tui` harness failures are represented, and how `ploke-eval` knows when a sub-agent returns.

## Executive Finding

The current implementation has two different paths:

- **Implemented deterministic/eval-side checked edit path:** `ploke-eval` can validate an already-produced proposal, derive checked touches, run grant/check/apply evidence in memory, and materialize a single-file checked child candidate.
- **Pending broad/hyper-agent path:** `ploke-eval` publishes a broad harness request and prompt, then stops with a pending submitted-result path. It does not yet invoke `ploke-tui` as the live sub-agent, wait for its return, turn a submitted result into a request-bound child plan, or convert the submitted broad result into checked bounded edit evidence.

So: handling the current blocker is necessary, but not sufficient by itself unless the implementation also adds the adapter/continuation pieces below.

## Implemented Behavior

### 1. Eval-Side Patch Application Shape

`ploke-eval` has a trait-shaped harness boundary in `edit_surface/harness.rs`:

- `Harness::propose(input) -> (Proposal, Run)` and `Harness::apply_checked(proposal, check)` are the intended boundary ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:7](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs#L7)).
- `ArtifactDelta` records base, after, and touched spans after a surface check ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:23](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs#L23)).
- The mock adapter only accepts a `surface::Check` whose proposal/base/after/touches match the proposal ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:142](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs#L142)).

The concrete backend path is not a live proposal generator. It validates an already-produced `EditProposal`:

- The comment explicitly says `validate_edit_surface_candidate` is not a generator and live CLI must fail closed until a real producer supplies spans/replacements ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:1068](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1068)).
- It rejects empty touches and currently rejects multi-file proposals (`paths.len() != 1`) ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:1092](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1092), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:1104](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1104)).
- It computes source/proposed content, patch id, base and derived artifact ids, and derives checked touch evidence before building `CheckedSurfaceEdit` ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:1140](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1140), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:1297](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1297)).

### 2. Multiple Patches And Touched Files

Current support is mixed:

- In `ploke-eval`, the checked deterministic path handles **multiple touches in one file**. It sorts touches, validates non-overlap, folds all replacements together from the end of the file, then validates all checked writes ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:1127](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1127), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:2352](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L2352), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:2388](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L2388)).
- In `ploke-eval`, that same path rejects **multiple files** before checked apply ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:1098](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1098)).
- In `ploke-tui`, semantic `apply_code_edit` staging can represent edits across multiple files; the proposal stores `edits`, `files`, preview, and pending status ([crates/ploke-tui/src/rag/tools.rs:114](../../../../crates/ploke-tui/src/rag/tools.rs#L114), [crates/ploke-tui/src/rag/tools.rs:216](../../../../crates/ploke-tui/src/rag/tools.rs#L216)).
- In `ploke-tui`, non-semantic patch staging rejects multiple patch entries for the same file and asks the model to combine them into one unified diff per file ([crates/ploke-tui/src/rag/tools.rs:1000](../../../../crates/ploke-tui/src/rag/tools.rs#L1000)).
- In `ploke-tui`, bulk approval orders proposals newest-first, marks overlapping older proposals stale, and treats non-semantic proposals as whole-file touches for overlap detection ([crates/ploke-tui/src/rag/editing.rs:633](../../../../crates/ploke-tui/src/rag/editing.rs#L633), [crates/ploke-tui/src/rag/editing.rs:744](../../../../crates/ploke-tui/src/rag/editing.rs#L744)).

This means `ploke-tui` can stage multi-file proposals, but the current `ploke-eval` checked candidate bridge is single-file.

### 3. All-Or-Rejected Apply Evidence

`ploke-eval` has the stronger all-or-rejected shape in `tui::Apply`:

- `Apply::from_results` rejects if returned write count differs from touched count ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:784](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs#L784)).
- It rejects if any write span differs from the checked touch or if any write failed ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:799](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs#L799)).
- `validate` then requires the after artifact id and every touched file hash to match the reported write result ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:829](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs#L829)).

`ploke-tui` itself is looser:

- Semantic apply marks a proposal `Applied` when `applied > 0`, and `Failed("No semantic edits were applied")` only when zero edits applied ([crates/ploke-tui/src/rag/editing.rs:381](../../../../crates/ploke-tui/src/rag/editing.rs#L381), [crates/ploke-tui/src/rag/editing.rs:388](../../../../crates/ploke-tui/src/rag/editing.rs#L388)).
- The success event includes `"ok": applied > 0`, `applied`, and per-file results, so partial apply can be observable but is not rejected by `ploke-tui` itself ([crates/ploke-tui/src/rag/editing.rs:363](../../../../crates/ploke-tui/src/rag/editing.rs#L363)).

Therefore the adapter must not trust `EditProposalStatus::Applied`; it must translate `ploke-tui` write results into the `tui::Apply` all-or-rejected evidence shape before `ploke-eval` materializes a child.

### 4. Clean, Dirty, Stale Base, Surface, And Protected Core Checks

Implemented checks:

- Active parent checkout install/validation refuses dirty checkouts ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:1915](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1915), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:1978](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1978)).
- `clean_tree_key` refuses dirty active checkouts before deriving the clean tree key ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:2049](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L2049)).
- `dirty_paths` uses `git status --porcelain --untracked-files=all` as the backend dirty-state source ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:2178](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L2178)).
- Broad harness admission rejects dirty source repos, non-isolated candidate workspaces, stale candidate base heads, no-op changed path sets, and paths outside the broad edit policy ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:1374](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1374), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:1385](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1385), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:1393](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1393), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:1403](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1403)).
- `persist_files` refuses unexpected dirty paths before staging and committing allowed paths ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:1530](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1530)).
- Edit surface path policy for `PlokeTuiTools` is limited to `crates/ploke-tui/src/tools/**`, `crates/ploke-tui/src/rag/tools.rs`, and `crates/ploke-tui/src/rag/editing.rs` ([crates/ploke-eval/src/cli/prototype1_state/backend.rs:2226](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L2226), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:2251](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L2251), [crates/ploke-eval/src/cli/prototype1_state/backend.rs:2296](../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L2296)).
- `surface::Grant` checks artifact identity, graph containment, forbidden protected-core spans, and writable area before minting `surface::Check` ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs:225](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs#L225), [crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs:304](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs#L304)).
- `EditableSurface::broad` constructs a writable area by excluding `ProtectedCore` spans from the graph surface ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs:553](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs#L553), [crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs:576](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs#L576)).

Important gap: the broad harness admission path checks changed file paths against the policy, but it does not yet lower changed files into semantic/touch-level `SurfaceGrant` checks or protected-core span checks. That is acceptable only as submitted-result evidence. It is not enough for request-bound child plan authority.

### 5. Harness Failures And No-Op Edits

`ploke-tui` represents tool and apply failures through proposal status plus tool events:

- `EditProposalStatus` has `Pending`, `Approved`, `Denied`, `Applied`, `Failed(String)`, and `Stale(String)` ([crates/ploke-tui/src/app_state/core.rs:321](../../../../crates/ploke-tui/src/app_state/core.rs#L321)).
- Tool completion/failure crosses the TUI event bus as `ToolCallCompleted` or `ToolCallFailed` with `request_id`, `parent_id`, `call_id`, content/error, and optional UI payload ([crates/ploke-tui/src/app_state/events.rs:87](../../../../crates/ploke-tui/src/app_state/events.rs#L87)).
- Semantic tool calls reject empty edit requests before staging ([crates/ploke-tui/src/rag/tools.rs:626](../../../../crates/ploke-tui/src/rag/tools.rs#L626)).
- Staging emits a `ToolCallCompleted` payload saying the edit is staged, not applied; auto-confirm applies later in a spawned task if enabled ([crates/ploke-tui/src/rag/tools.rs:273](../../../../crates/ploke-tui/src/rag/tools.rs#L273), [crates/ploke-tui/src/rag/tools.rs:316](../../../../crates/ploke-tui/src/rag/tools.rs#L316)).
- Apply failures emit `ToolCallFailed`; zero semantic writes emit `ToolCallCompleted` with `ok=false` and `applied=0` ([crates/ploke-tui/src/rag/editing.rs:381](../../../../crates/ploke-tui/src/rag/editing.rs#L381), [crates/ploke-tui/src/rag/editing.rs:458](../../../../crates/ploke-tui/src/rag/editing.rs#L458)).
- LLM finish timeouts retry according to timeout policy and eventually return `LlmError::Timeout` ([crates/ploke-tui/src/llm/manager/session.rs:478](../../../../crates/ploke-tui/src/llm/manager/session.rs#L478)).
- Per-tool event waits time out after `tool_call_timeout`, returning a `ToolErrorCode::Timeout` with retry hint ([crates/ploke-tui/src/llm/manager/session.rs:1537](../../../../crates/ploke-tui/src/llm/manager/session.rs#L1537)).

There is currently no implemented `ploke-eval` adapter that normalizes all of those into typed bounded-edit attempt evidence. In particular, a no-op edit can be a successful TUI turn with no material child unless the adapter explicitly rejects `applied == 0` or no changed paths.

### 6. How `ploke-eval` Knows A Sub-Agent Returned

For the current broad/hyper-agent path, it does not yet have a live wait loop:

- Complete live mode rejects `BroadHarnessRequest` until a typed request-to-child-plan receipt exists ([crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:745](../../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L745)).
- When selected, the broad path publishes request JSON and prompt markdown, moves the parent into `Parent<AwaitingHarnessPlan>`, and returns a pending request error naming the submitted-result path ([crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1092](../../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L1092), [crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6190](../../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L6190)).
- `Parent<AwaitingHarnessPlan>` carries only `request::Reference<Broad, Published>` ([crates/ploke-eval/src/cli/prototype1_state/parent.rs:44](../../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs#L44), [crates/ploke-eval/src/cli/prototype1_state/parent.rs:981](../../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs#L981)).
- Submitted result verification exists as a typed record check against published request identity, request hash, parent node, workspace path, submitted-result path, admission binding, and relative changed file paths ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:111](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs#L111)).

For `ploke-tui` internal tests, return/terminal observation is event-driven:

- The live adapter test waits for `ToolCallRequested`, `ToolCallCompleted`, and `ChatTurnFinished` events, and separately observes the proposals map ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1737](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs#L1737)).
- This is test-only behavior, not yet the production `ploke-eval` adapter boundary.

## Prompt / Evidence Currently Wired

The broad harness prompt is generated from `BroadHarnessRequest::render_prompt`:

- It includes parent node, source repository snapshot, mutable candidate workspace, edit policy, and child budget ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:930](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs#L930)).
- It includes evaluation scope, selection policy, guidance policy, protected-core pointer, evidence roots, return-evidence contract, and instructions ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:948](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs#L948)).
- Evidence roots include submitted-result output, History blocks, evaluations, nodes, protocol artifacts, and oracle attached reports when present ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:866](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs#L866)).
- The instruction says the harness writes typed submitted-result evidence and that `ploke-eval` later checks it and may mint a child plan after admission ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:762](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs#L762)).

This matches the HyperAgents prompt posture: expose evidence and budget, do not over-specify the edit target, keep oracle/selection material controlled, and do not make TUI state authority ([docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:37](../../../../docs/design/drafts/edit-surface/hyperagents-prompt-posture.md#L37), [docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:72](../../../../docs/design/drafts/edit-surface/hyperagents-prompt-posture.md#L72)).

The direct live TUI test prompt is narrower and test-specific: it tells the model to call `apply_code_edit` exactly once with an exact JSON payload ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1709](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs#L1709)). That is a proof of plumbing, not the intended hyper-agent prompt posture.

## Required Behavior Not Yet Implemented

The docs require the adapter to preserve these boundaries:

- `CheckedProposal`/surface check is produced by `ploke-eval`, not by the harness ([docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md:1500](../../../../docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md#L1500), [docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md:2283](../../../../docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md#L2283)).
- Apply may delegate mechanical writes to `ploke-tui`/`ploke-io`, but `ploke-eval` must verify enough evidence to name the derived Artifact and `ArtifactDelta` ([docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md:1520](../../../../docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md#L1520)).
- Partial apply must not silently count as success; V1 must be all-or-rejected ([docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md:2154](../../../../docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md#L2154)).
- `ploke-eval` should see operation evidence, not TUI session structure ([docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md:2162](../../../../docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md#L2162)).
- Minimum concrete adapter behavior includes target artifact/worktree binding, effective request-policy receipts, artifact-tied index projection, exact semantic resolution, staged proposal evidence, resolved touches before apply, checked apply, and all-or-rejected apply evidence ([docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md:2433](../../../../docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md#L2433)).
- Candidate creation should run: diagnose from History, choose surface, construct objective/grant, ask harness for proposals, check each proposal, apply checked proposals into derived Artifacts, then publish child plan with surface evidence ([docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md:2467](../../../../docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md#L2467)).

The model doc says the same boundary more generally:

- `EditProposal` is an intermediate procedure state, not an admitted Artifact transition or History authority ([docs/workflow/evalnomicon/drafts/edit-surface/model.md:38](../../../../docs/workflow/evalnomicon/drafts/edit-surface/model.md#L38)).
- `ploke-eval` should consume `ploke-tui` through a trait adapter, not import TUI internals into History authority paths ([docs/workflow/evalnomicon/drafts/edit-surface/model.md:291](../../../../docs/workflow/evalnomicon/drafts/edit-surface/model.md#L291)).
- Executor roles must remain distinct: Parent grants, Generator proposes, TUI resolves/stages/applies, checker validates, backend realizes Artifact identity, and History records evidence ([docs/workflow/evalnomicon/drafts/edit-surface/model.md:764](../../../../docs/workflow/evalnomicon/drafts/edit-surface/model.md#L764)).

## Plan Wireframe For Correct Bounded TUI Adapter

1. **Adapter Input Carrier**

   Define an input carrier for:

   - `request::Request<Broad, Published>` or deterministic edit objective;
   - live `EditSurfaceAdmission`;
   - base Artifact/worktree/root/head;
   - `SurfaceGrant`/policy/protected-core pointer;
   - evidence roots and budget;
   - explicit TUI/Router policy config.

   This carrier must not expose `AppState.proposals` or CLI text as authority.

2. **Launch / Return Contract**

   For broad hyper-agent mode, replace the current “publish prompt and stop” continuation with an explicit operation result:

   ```text
   Parent<Ready>
     -> Request<Broad, Published>
     -> HarnessRun<Broad, Running>
     -> SubmittedResult<Broad, Returned | Failed | TimedOut>
     -> AdmissionCheck
     -> ChildPlan<For<Request<Broad, Published>>> | RejectedAttempt
   ```

   The return signal can be either:

   - a typed submitted-result file plus observed process/session exit status; or
   - an in-process `ploke-tui` event/session completion receipt.

   In either case, `ploke-eval` should require an explicit terminal receipt. A file appearing is not enough unless paired with a process/session completion or timeout outcome.

3. **Prompt Construction**

   Use the current broad prompt shape as the base:

   - target worktree path;
   - source snapshot;
   - evidence roots;
   - oracle/report locations when intentionally exposed;
   - remaining budget;
   - protected-core code pointer;
   - return-evidence schema.

   Do not use the narrow test prompt except for adapter smoke tests.

4. **Proposal Evidence**

   The adapter should return a typed proposal receipt containing:

   - request id/hash and run id;
   - prompt/schema/tool digest;
   - effective request-policy receipt for every material Router call;
   - proposal id/call ids;
   - resolved semantic targets or patch files;
   - material spans with expected hashes;
   - preview digest;
   - touched files/spans;
   - harness telemetry references.

5. **Pre-Apply Check**

   `ploke-eval` performs:

   - request binding check;
   - clean source/candidate workspace check;
   - stale base/head check;
   - surface path check;
   - graph/projection identity check;
   - protected-core exclusion;
   - expected hash validation;
   - overlap/non-overlap validation for all touches.

   For V1, either keep single-file checked candidate materialization, or explicitly extend `CheckedSurfaceEdit` to a patch-set carrier. Do not let multi-file TUI proposals bypass the all-or-rejected shape.

6. **Apply And Validate**

   Mechanical apply may happen through `ploke-tui`/`ploke-io`, but the adapter must translate results into `tui::Apply`-like evidence:

   - every planned touch has exactly one write result;
   - each write result names the same span;
   - every write succeeded;
   - final after Artifact file hashes match all write results;
   - no unexpected dirty paths exist;
   - derived Artifact identity is minted by backend state, not by TUI status.

7. **Failure Handling**

   Normalize failures into typed outcomes:

   - provider timeout;
   - tool-call timeout;
   - tool-call failed;
   - no proposal;
   - no edits;
   - no-op/no changed paths;
   - partial apply;
   - stale hash/head;
   - protected-core/surface escape;
   - adapter crash/session exit;
   - submitted result malformed or request-mismatched.

   Failed outcomes should become rejected attempt evidence, not child Artifact authority.

8. **Child Validation / Oracle Visibility**

   The sub-agent can be pointed at oracle summaries/reports only through evidence roots intentionally exposed by the request. The prompt currently includes an attached `final_report.json when present` oracle summary role ([crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:903](../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs#L903)).

   Child validations remain `ploke-eval` owned. The harness may suggest checks in submitted evidence, but actual child materialization, build/test/evaluation, History admission, and successor selection must consume typed evidence and backend results, not harness claims.

## Bottom Line

If blocker 1 is handled only as “accept submitted broad result and make a child plan,” the live run is still structurally weak.

The correct implementation must make the submitted result or live TUI session pass through:

```text
request-bound return receipt
  -> proposal/touch evidence
  -> ploke-eval surface check
  -> all-or-rejected apply validation
  -> derived Artifact
  -> ChildPlan<For<Request<Broad, Published>>>
```

Only after that can the long loop safely let the parent runtime call into `ploke-tui` via the adapter and admit edited child Artifacts.
