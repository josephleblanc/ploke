# Edit-surface persistence walkthrough: admissionfix 195048

Date: 2026-06-06
Source commit for code line references: `77835b6e`
Campaign: `p1-admissionfix-g35flash-p25flash-20260605-195048`
Parent node: `node-28d0482d4bc80fcb`

## Short answer

The good edit outcomes were persisted at the per-slot submitted-result layer.
They were not promoted into a completed parent child plan or successor handoff.

The two applied slots, `r3` and `r4`, each wrote:

- a headless TUI trace sidecar;
- a submitted broad-harness result JSON;
- a candidate workspace commit.

The parent/batch layer still did not have enough admitted children to satisfy
the configured minimum of five. The run ended with only two submitted/applied
candidates, several timeout/no-edit slots, one provider-unavailable slot, and an
unresolved parent lifecycle projection. So the failure is not "valid result JSONs
were never created"; it is "too few admissible child transactions reached the
parent batch handoff, and the parent never produced completed child-plan /
runner-result / successor evidence."

## Evidence roots

Run-review docs were found as unstaged edits in:

```text
/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260605-195048/docs/active/agents/run-reviews/
```

The main synthesis files are:

- `2026-06-05-p1-admissionfix-195048-coverage-status.md`
- `2026-06-05-p1-admissionfix-195048-final-fanin.md`
- `2026-06-05-p1-admissionfix-195048-expanded-trace-fanin.md`
- `2026-06-05-p1-admissionfix-195048-bugs-blockers-synthesis.md`
- `2026-06-05-p1-admissionfix-195048-improvement-synthesis.md`
- slot reviews for `node-28d0482d4bc80fcb`, `r2`, `r3`, `r4`, `r5`, `r6`,
  `r7`, `r8`, and `r10`.

Current artifact roots checked:

```text
/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260605-195048
/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260605-195048
```

## Slot artifact ledger

`prototype1/messages/edit-harness-result/` currently contains submitted result
JSONs only for `r3` and `r4`:

```text
node-28d0482d4bc80fcb-r3.headless-tui.json
node-28d0482d4bc80fcb-r3.json
node-28d0482d4bc80fcb-r4.headless-tui.json
node-28d0482d4bc80fcb-r4.json
```

Other trace-bearing slots have only `.headless-tui.json` sidecars. `r9` remains
request/workspace-only in the reviewed evidence and is kept separate from
terminal trace coverage.

Observed terminal states:

- `node-28d0482d4bc80fcb`: `timed_out`, 900s, no submitted result.
- `r2`: `timed_out`, 900s, no submitted result.
- `r3`: `applied`, submitted result present, candidate commit present.
- `r4`: `applied`, submitted result present, candidate commit present.
- `r5`: `timed_out`, 900s, no submitted result.
- `r6`: `timed_out`, 900s, no submitted result.
- `r7`: `timed_out`, 900s, no submitted result.
- `r8`: `timed_out`, 900s, no submitted result, with protected edit denial in
  the trace.
- `r10`: `provider_unavailable`, authentication-class provider failure, no
  submitted result.

The applied candidate commits are real:

```text
r3: d6f76f75 prototype1 broad harness result broad-harness-request:node-28d0482d4bc80fcb:r3
    crates/ingest/ploke-mbe/src/structural.rs | 45 ++++++++++++++++++++++++++++---

r4: a07ce5e8 prototype1 broad harness result broad-harness-request:node-28d0482d4bc80fcb:r4
    crates/ploke-ty-mcp/src/manager.rs | 1 +
```

The submitted-result files bind back to their published request and result path:

```text
r3 request.submitted_result_path =
  .../edit-harness-result/node-28d0482d4bc80fcb-r3.json

r4 request.submitted_result_path =
  .../edit-harness-result/node-28d0482d4bc80fcb-r4.json
```

Both submitted results carry `return_evidence.authority_boundary` values of
`not_claimed` for `admission`, `grant`, and `child_plan`. That is intentional:
the submitted result is candidate-generation evidence, not final parent
authority.

## Config that shaped this failure

The run profile for the failed run used:

- edit/parent model: `google/gemini-3.5-flash`;
- edit/parent route: `direct-google`;
- protocol model: `google/gemini-2.5-flash`;
- protocol route: `direct-google`;
- `search.children.min = 5`;
- `search.children.max = 5`;
- `search.children.parallel_targets = 5`;
- `execution.broad_tui.max_attempts = 2`;
- `execution.broad_tui.fresh_slots_per_child = 2`;
- `control.parallel_cap = 3`;
- `search.require_keep_for_continuation = true`;
- `selection.strategy = "history-score-child-prop"`;
- `selection.evidence = "operational-and-protocol"`.

That means the parent publishes `max * fresh_slots_per_child = 10` fresh
broad-harness request slots, but needs at least five admitted child
transactions before it can publish a runnable child plan.

## Process map

```text
Parent<Ready>
  |
  | publish_broad_harness_child_plan_request
  | - mark parent node running
  | - publish 10 request slots
  | - each slot has request JSON, prompt markdown, workspace path,
  |   submitted_result_path, admission binding, validation contract
  v
Parent<AwaitingHarnessPlan>
  |
  | admit_broad_harness_batch
  | - run broad slots with parallel cap
  | - each slot invokes the headless TUI adapter
  | - only terminal Applied can write the submitted result JSON
  | - backend then validates/admit-persist-files for that submitted result
  v
Vec<AdmittedBroadHarnessResult>
  |
  | publish_broad_harness_child_plan_from_admitted_batch
  | - if len < child_budget.min: persist rejected-attempt evidence and fail
  | - else: create child nodes, runner requests, branch records, child plan
  v
Parent<Planned> / runnable children
```

For this campaign the flow stopped before a completed parent child plan:

```text
10 requested slots
-> 2 submitted/applied candidates
-> fewer than required minimum 5
-> no completed parent child-plan / runner-result / successor evidence
```

Current parent artifacts confirm the parent lifecycle did not finalize:

- `prototype1/nodes/node-28d0482d4bc80fcb/node.json` still says
  `status = "running"`;
- `runner-result.json` is absent;
- `scheduler.json` still has the parent in `frontier_node_ids`, no completed
  node ids, no failed node ids, and the node status as `planned`;
- `transition-journal.jsonl` has only `parent_started` and a resource sample.

## TUI adapter loop

The headless TUI loop is in
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`.

`run_headless_with_model_inner` starts attempts, wraps the whole slot in the
turn budget timeout, and calls `run_attempt` for each fresh attempt
(`tui_bridge.rs:96-225`).

`run_attempt` observes:

- `ToolCallRequested` events and records tool name/arguments
  (`tui_bridge.rs:476-505`);
- `ToolCallCompleted` events and records tool result content
  (`tui_bridge.rs:506-538`);
- staged edit items and settled gated batches
  (`tui_bridge.rs:539-563`);
- applied items and changed paths from the batch outcome
  (`tui_bridge.rs:564-570`).

The key fixed behavior is after an allowed applied batch:

```text
newly_applied
&& validation_commands not empty
-> validate_applied_batch(...)
-> if classified as Applied, finalize immediately
```

That path is at `tui_bridge.rs:571-610`. The comments explain why the adapter
does not wait for the model to voluntarily stop after a valid applied edit: if
the candidate already satisfies the declared validation, waiting for another
provider turn lets the model consume the whole slot and never reach an
admissible terminal.

`validate_applied_batch` runs the request-declared validation itself
(`tui_bridge.rs:852-904`). The comments are the important contract:

```text
The harness runs the declared validation itself, so admission does not depend on
the model issuing the exact declared cargo commands.
```

`classify_applied_terminal` then rejects failed or missing requested validation
and returns `HeadlessTerminal::Applied` only when the requested command has a
passing observation (`tui_bridge.rs:925-948`). Matching is exact display-command
matching with a small `--` normalization (`tui_bridge.rs:988-1007`).

If the adapter sees `ChatTurnFinished`, it still handles provider failures,
non-completed outcomes, no-edit completions, and completed-with-applied
validation (`tui_bridge.rs:712-760` and following). But for the happy path after
an applied edit plus passing declared validation, the current path can stop
earlier at the batch boundary.

## Submitted-result write path

The parent-side broad-harness runner is in
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`.

`run_broad_headless_tui_attempt_with_options`:

1. prepares the candidate workspace;
2. reads the prompt;
3. builds the TUI budget from the request/profile;
4. calls `run_headless_with_model_capture_responses`;
5. writes diagnostics and the turn-live replay bundle;
6. calls `finish_broad_headless_tui_attempt`
   (`cli_facing.rs:1505-1594`).

`finish_broad_headless_tui_attempt` is the only place that converts a terminal
into a submitted result. It writes the submitted result only for
`HeadlessTerminal::Applied` (`cli_facing.rs:1647-1675`).

Everything else refuses to publish submitted result evidence:

- `CompletedWithoutEdit`;
- `ToolFailed`;
- `NoEdit`;
- `ContextUnavailable`;
- `ProviderUnavailable`;
- `AppliedValidationFailed`;
- `AppliedValidationMissing`;
- `AppliedTurnAborted`;
- `AppliedTimedOut`;
- `TimedOut`.

That is why timeout/no-edit slots correctly have `.headless-tui.json` sidecars
but no submitted-result JSON.

`write_headless_tui_submission` creates the `SubmittedBroadHarnessResult`,
binds it to the published request, and writes it to
`slot.published.submitted_result_path()` (`cli_facing.rs:1792-1863`).

## Backend admission

The backend admission path is
`crates/ploke-eval/src/cli/prototype1_state/backend/harness_ingestion.rs`.

`admit_submitted_broad_harness_result`:

1. verifies the submitted result matches the published request
   (`harness_ingestion.rs:30-34`);
2. verifies the live admission binding matches the published binding
   (`harness_ingestion.rs:36-49`);
3. validates the TUI attempt and turns it into a diff
   (`harness_ingestion.rs:51-53`);
4. persists changed files as a broad-harness result commit
   (`harness_ingestion.rs:64-68`);
5. returns a transaction carrying request, admission, workspace, derivation,
   change set, and submission path (`harness_ingestion.rs:72-82`).

`validate_tui_attempt` is a surface/diff gate, not a semantic benchmark gate:

- source repository path must match;
- source and candidate worktrees must be isolated;
- source must be clean;
- candidate head must still equal the source/base head before persistence;
- changed paths must be nonempty;
- changed paths must be normal repo-relative paths;
- changed paths must match the broad edit surface policy;
- candidate dirty paths must be expected changed paths only
  (`harness_ingestion.rs:85-173`).

This explains why `r4` can be mechanically admitted but still be likely invalid:
the backend proves the candidate is a bounded, surface-valid diff, but it does
not prove proposal completeness or touched-crate buildability.

## Request contract

The broad harness request contract is in
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`.

The model is told to use edit tools and not create separate bookkeeping/result
files (`harness_request.rs:70-75`). The contract comments state the current
admission target: prove a runnable child artifact; candidate quality is
downstream child self-evaluation and selection, not broad-harness admission
(`harness_request.rs:86-100`).

The declared validation command is:

```text
cargo check -p ploke-eval
```

running in the candidate workspace (`harness_request.rs:101-113`). The r4
review shows the weakness of that contract: `r4` touched `ploke-ty-mcp`, while
the post-apply declared validation checked `ploke-eval`, which did not prove the
touched crate still built.

The submitted-result schema itself is in `harness_result.rs:296-365`. Binding
and request verification are in `harness_result.rs:398-477`; changed-file
validation only rejects absolute or parent-directory paths
(`harness_result.rs:487-503`).

## Parent batch admission

`publish_broad_harness_child_plan_request` publishes the broad slots
(`cli_facing.rs:1216-1279`). It uses:

```text
slot_count = child_budget.max * fresh_slots_per_child
```

For this profile that is `5 * 2 = 10`.

`admit_broad_harness_batch` runs slots up to the configured cap and accumulates
admitted transactions (`cli_facing.rs:4340-4435`). Slot-level
`InvalidBatchSelection` errors are warnings and the loop tries another fresh
slot. `ProviderUnavailable` and `DatabaseSetup` are returned immediately
(`cli_facing.rs:4388-4405`).

`try_admit_request_result` looks for the request-bound submitted result path,
decodes `SubmittedBroadHarnessResult`, and calls backend admission
(`cli_facing.rs:1308-1397`). Missing submitted-result files are not admitted.

The final gate is `publish_broad_harness_child_plan_from_admitted_batch`
(`cli_facing.rs:2184-2235`):

```text
if admitted.len() < batch.child_budget.min {
    persist_rejected_plan(...);
    write failed node projection;
    return "broad harness admitted N child transaction(s), fewer than required minimum M";
}
```

If the admitted count is high enough, this function creates children and writes
the typed child-plan message. If it is not high enough, the run has no runnable
child set.

## Why the "good edit" outcomes were not parent-persisted

The r3/r4 edit outcomes were persisted locally as submitted results and
candidate commits. They did not become usable parent outcomes because the parent
wave is all-or-nothing at the configured child-plan gate:

```text
need at least 5 admitted child transactions
observed 2 submitted/applied candidates
=> no completed child plan with runnable children
```

The run-review docs add two quality warnings:

1. `r3` is mechanically successful but weakly tied to the benchmark objective.
   Its changed crate/path was not proven to affect the ripgrep benchmark or
   descendant-selection objective.
2. `r4` is mechanically persisted but likely invalid. The model intended six
   edits, the approval/debug path applied one edit, and the final commit adds
   only the `start_locks` field. The constructor and method-body edits did not
   land.

So there are two different problems:

- Parent persistence problem: only two child transactions reached admission
  under a five-child minimum, and parent lifecycle artifacts did not complete.
- Candidate quality problem: the current per-slot submitted-result/admission
  contract is too weak to flag partial proposal collapse or changed-crate
  validation gaps before the parent considers a candidate mechanically admitted.

## Chain-of-custody and authority references

The evalnomicon design references line up with this distinction.

`docs/workflow/evalnomicon/drafts/edit-surface/model.md:836-848` says the
authority direction is:

```text
ploke-eval owns grants, checks, candidate admission, History evidence.
EditHarness encodes the operation boundary.
ploke-tui adapter implements that boundary using current TUI machinery.
```

The same section says TUI proposals, statuses, chat/tool events, and edit
approval are executor/session mechanics unless `ploke-eval` records them as
evidence for an admitted transition.

`edit-surface/model.md:900-984` sketches the intended `EditHarness` boundary:
proposal, approval, and apply are harness mechanics, while `ploke-eval` owns the
surface check. Only after a passing surface check should the candidate proceed
to artifact application, History admission, child hydration, and attribution.

`edit-surface/proof-index.md:87-96` states the general rule:

```text
record(rec) does not imply admissible_D(rec)
projection(rec) or log(rec) or ui_state(rec) does not imply admissible_D(rec)
```

`edit-surface/proof-index.md:130-143` distinguishes rejected-attempt evidence
from successful artifact-transition admission. That matters here because the
timeout/no-edit slots can be diagnostic evidence without becoming children.

`edit-surface/tui-approve-deny-pipeline.md:8-17` makes the boundary explicit:
`ploke-tui` stages/applies/refreshes local edit mechanics; `ploke-eval` decides
whether the settled mutation is admissible Prototype 1 candidate evidence.
Lines 21-43 describe the happy path: model edit tool, gated approval, post-apply
refresh, eval-side diff validation, then candidate publication.

`edit-surface/tui-approve-deny-pipeline.md:87-99` also calls out partial
mutation as a separate state that should not be treated as clean apply or
ordinary failure. That is exactly the r4 class of problem.

`runtime/authority.md:52-84` says runtime authority is role- and state-bounded;
successful child output is authority-bearing only when the terminal channel
result carries the runner result and treatment evidence.

`runtime/authority.md:126-138` says transition records should be projections of
allowed transitions, not arbitrary status writes. That is why a stale
`node.json` status is not enough to claim parent finality.

`runtime/parent-child-channel.md:9-15` says parent/child lifecycle state must be
driven by the per-runtime channel, not compatibility projections. Lines 118-132
describe `messages/child-plan/<parent-node-id>.json` as the parent-owned typed
candidate publication message.

The chain-of-custody drafts say every History entry should preserve executor,
observer, recorder, proposer, ruling/admitting authority, input/output refs,
timestamps, payload hash, and ordering (`history/crown-authority-background.md:
216-236`). The history-crown persistence map notes that admitted/sealed History
entries are the intended authoritative block contents, but the live path is
still mostly type surface/tests for non-empty entries
(`persistence/map-2026-05-03/history-crown.md:68-75`).

## Durable conclusions

1. The fixed TUI adapter did produce submitted result artifacts for valid
   applied slots in this run.
2. The run still failed because the parent wave needed five admitted child
   transactions and only two slots reached that layer.
3. The current submitted-result layer is necessary but not sufficient evidence
   for child-plan authority or benchmark usefulness.
4. The next repair target is not "make r3/r4 submitted-result files exist";
   those files exist. The next target is a stronger parent/lifecycle projection
   plus better per-slot lifecycle/accounting fields:
   - timeout/no-edit vs provider-unavailable vs protected-denied terminal
     taxonomy;
   - intended/staged/applied/committed edit counts;
   - changed-crate validation coverage;
   - raw provider ledger health;
   - explicit parent batch finalization evidence when below-min child admission
     occurs.
