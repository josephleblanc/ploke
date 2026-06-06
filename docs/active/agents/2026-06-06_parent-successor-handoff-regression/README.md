# 2026-06-06 Parent Successor Handoff Regression Investigation

Status: local implementation and focused regression coverage complete; live validation pending
Scope: Prototype 1 runs that stopped before actual child self-evaluation or
parent/successor handoff after June 2.

Companion docs:

- [`implementation-notes-2026-06-06.md`](implementation-notes-2026-06-06.md)
  records the local fix, regression test, and verification commands for the
  broad batch finalization bug.

## Conclusion

The post-June-2 failure is not that every broad headless-TUI candidate timed
out, and it is not that the current TUI adapter requires the model to
self-evaluate. Current source can produce validated submitted results after an
applied tool batch while preserving later repair tool calls when validation is
not satisfied.

The remaining controller bug is at broad batch finalization: a provider/database
fatal slot can return from `admit_broad_harness_batch` before the parent writes
a durable child-plan outcome, and the existing below-minimum failed-plan path
also hides partial admitted candidates from parent-readable surface-attempt
evidence. The fix should make broad batch admission collect an explicit slot
ledger and always finalize/persist the batch outcome before returning.

## Question

Since the June 2 successful runs, Prototype 1 live-loop attempts have repeatedly
failed to produce a parent-owned child plan with enough runnable children and
therefore have not reached child self-evaluation or successor handoff.

This note records the evidence chain while investigating:

1. where the bug was first reported in the bug docs;
2. the most recent run/worktree that actually reached parent/successor handoff;
3. the commits and code-path changes between that success and the current
   repeated failure;
4. a durable fix that preserves multi-tool-call turns inside the headless
   `ploke-tui` adapter loop.

## Current Working Hypothesis

The recent failures are not all the same layer. At least one current run
persisted two broad-harness candidate patches (`r3`, `r4`) but still did not
produce child self-evaluation because the parent batch required five admitted
child transactions and the batch aborted on a later provider failure before
child-plan persistence. Earlier zero-admission reports include different
upstream causes: provider failure, post-apply timeout, missing validation,
stale-indexing, or no-edit exploration.

The investigation must therefore distinguish:

- broad-harness slot terminal state;
- submitted-result JSON persistence;
- backend admission into `AdmittedBroadHarnessResult`;
- child-plan publication;
- child C1-C4 execution and runner result;
- successor selection/handoff.

## Running Evidence Log

### 2026-06-06: setup

- Found two bug/documentation trees:
  - `docs/active/bugs`
  - `docs/bugs` with `docs/bugs/live_bugs`
- `docs/bugs` did not contain the relevant Prototype 1 reports for this issue
  family. The actionable reports are in `docs/active/bugs`.
- Initial search found related reports:
  - `docs/active/bugs/2026-05-25-prototype1-child-plan-zero-admission-timeout.md`
  - `docs/active/bugs/2026-05-25-headless-tui-timeout-submission-admission.md`
  - `docs/active/bugs/2026-06-04-prototype1-headless-timeout-after-apply.md`
  - `docs/active/bugs/2026-06-04-prototype1-post-apply-stale-snippet-indexing.md`
  - `docs/active/bugs/2026-06-04-prototype1-headless-tui-runtime-actor-leak.md`
- Current anchor run:
  `p1-admissionfix-g35flash-p25flash-20260605-195048`.
- Verified for that run:
  - `r3` and `r4` wrote submitted broad-harness result JSONs and candidate
    commits;
  - `r3` terminal was `applied`, changed
    `crates/ingest/ploke-mbe/src/structural.rs`, and recorded successful
    request-declared validation `cargo check -p ploke-eval`;
  - `r4` terminal was `applied`, changed
    `crates/ploke-ty-mcp/src/manager.rs`, and recorded successful
    request-declared validation `cargo check -p ploke-eval`;
  - remaining slot terminals included `timed_out` and one
    `provider_unavailable`; they were not all timeouts;
  - no child node directories exist under campaign `prototype1/nodes` beyond
    the parent;
  - no live child `runner-result.json` exists;
  - no `messages/child-plan/<parent>.json` exists;
- scheduler still lists only the parent as frontier/planned.

### 2026-06-06: bug chronology conclusion

The active bug docs show a sequence of adjacent failures rather than one old
report that exactly names the current source bug:

- 2026-05-22:
  `2026-05-22-prototype1-google-post-apply-indexing-timeout.md` first records
  the applied-edit/no-submitted-result family. Google applied candidate edits,
  but post-apply runtime/indexing work prevented a submitted harness result from
  being written.
- 2026-05-25:
  `2026-05-25-prototype1-child-plan-zero-admission-timeout.md` first records
  the exact visible controller error family:
  `broad harness admitted 0 child transaction(s), fewer than required minimum`.
  That bug was below-minimum batch evidence not being persisted before return;
  the source fix made rejected-attempt-only child plans durable.
- 2026-05-25:
  `2026-05-25-prototype1-broad-headless-google-401-slot-thrash.md` first
  records the direct-Google 401/provider-unavailable layer. The intended
  contract is that provider authentication/quota/availability failures stop
  child planning as provider/environment blockers, not as ordinary no-edit
  model failures.
- 2026-06-04 / 2026-06-05:
  `2026-06-04-prototype1-headless-timeout-after-apply.md` records the
  post-June-2 broad-headless-TUI failure shape: applied edits were being
  classified as timeout/abort/missing-validation instead of producing submitted
  results. The current source now runs request-declared validation after an
  applied batch and finalizes `Applied` when validation passes.

The currently observed `p1-admissionfix-g35flash-p25flash-20260605-195048`
run is a later composition of those fixes and policies:

- the adapter correctly admitted only the two slots that reached `Applied` and
  passed request-declared validation;
- the profile required five admitted children;
- a later slot reached `ProviderUnavailable`;
- the batch admission loop returned that fatal provider error before child-plan
  persistence, so the already-admitted partial evidence did not become a
  parent-readable failed batch record.

### 2026-06-06: candidate validation versus child self-evaluation

Current failing runs distinguish candidate-level validation from child-node
self-evaluation:

- In `p1-admissionfix-g35flash-p25flash-20260605-195048`, candidate slots
  `node-28d0482d4bc80fcb-r3` and `node-28d0482d4bc80fcb-r4` were valid
  headless-TUI submissions:
  - `messages/edit-harness-result/node-28d0482d4bc80fcb-r3.headless-tui.json`
    has `terminal.terminal = "applied"` and a passing declared validation
    record for `cargo check -p ploke-eval`;
  - `messages/edit-harness-result/node-28d0482d4bc80fcb-r4.headless-tui.json`
    has `terminal.terminal = "applied"` and a passing declared validation
    record for `cargo check -p ploke-eval`;
  - submitted result JSONs exist for exactly `r3` and `r4`.
- Those candidate patches did not become child treatment nodes:
  - `messages/child-plan/` is empty for the run;
  - `prototype1/nodes/` contains only parent `node-28d0482d4bc80fcb`;
  - there are zero `runner-result.json` files.
- The recent child-plan files that do exist also stopped before child
  self-evaluation:
  - `p1-memfix-0604a`: `children = 0`, `rejected_surface_attempts = 10`;
  - `p1-admissionfix-g35flash-p25flash-20260604-221908`: `children = 0`,
    `rejected_surface_attempts = 10`;
  - `p1-admissionfix-g35flash-p25flash-20260605-142328`: `children = 0`,
    `rejected_surface_attempts = 10`;
  - `p1-admissionfix-g35flash-p25flash-20260605-173910`: `children = 0`,
    `rejected_surface_attempts = 10`;
  - each of those campaigns has only one node directory and zero
    `runner-result.json` files.

By contrast, the June 2 reference run
`p1-gemini35-flash-direct-3g2x3-par2-20260602-165628` did cross the child
self-evaluation boundary:

- child plan
  `prototype1/messages/child-plan/node-8b74b8f416f0dbdc.json` has
  `children = 3`, `rejected_surface_attempts = 0`;
- the three generation-1 children
  `node-66ba1fa79eb257b5`, `node-01c12bc5469ed398`, and
  `node-e74899dbdd50bf7a` each have `runner-result.json` with
  `status = "succeeded"`;
- `transition-journal.jsonl` records `observe_child` before/after events,
  `child` result-written events, `successor` selection, `active_checkout_advanced`,
  and `successor_handoff` for selected child `node-e74899dbdd50bf7a`;
- the successor process later failed on provider `HTTP_429`, but the
  parent/successor handoff boundary had already been reached.

### 2026-06-06: latest confirmed handoff

Parsed every campaign `prototype1/transition-journal.jsonl` for
`kind = "successor_handoff"` and sorted by `recorded_at`.

Latest confirmed handoff:

```text
recorded_at = 1780451028878
campaign = p1-gemini35-flash-direct-3g2x3-par2-20260602-165628
selected_node = node-e74899dbdd50bf7a
runtime_id = 29db0415-da40-4d8c-8923-91d0ef7c6e01
journal = /home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-165628/prototype1/transition-journal.jsonl
```

The matching active worktree is:

```text
/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-3g2x3-par2-20260602-165628
HEAD = d2e9289cdf12ef3a5b764c77d1e1cbb470d918a2
commit = prototype1 broad harness result broad-harness-request:node-8b74b8f416f0dbdc:r8
```

The source merge-base between that selected child commit and the latest useful
failing parent run `p1-admissionfix-g35flash-p25flash-20260605-195048`
(`HEAD = d648cf427325531e964b0aa19de88fe65a9a654e`) is:

```text
97b1a101b9363b88b8e41f1447cb36d76a3eb8a2
Add Google endpoint probes and provider retry
```

### 2026-06-06: profile delta at the handoff boundary

The latest handoff profile was smaller and less strict:

```text
campaign = p1-gemini35-flash-direct-3g2x3-par2-20260602-165628
model.id = google/gemini-3.5-flash
search.max_generations = 3
search.require_keep_for_continuation = false
search.children.min = 2
search.children.max = 3
search.children.parallel_targets = 2
control.parallel_cap = 2
```

The latest useful failing profile was larger and all-or-nothing at five
children:

```text
campaign = p1-admissionfix-g35flash-p25flash-20260605-195048
model.id = google/gemini-3.5-flash
protocol.model.id = google/gemini-2.5-flash
search.max_generations = 10
search.require_keep_for_continuation = true
search.children.min = 5
search.children.max = 5
search.children.parallel_targets = 5
execution.broad_tui.max_attempts = 2
execution.broad_tui.fresh_slots_per_child = 2
control.parallel_cap = 3
```

The source changes after the handoff baseline also made admission stricter in
ways that are mostly correct:

- `6bd5f8b6 Add typed post-apply terminal states and validation gating for
  headless TUI`: stopped treating post-apply timeout/abort/missing-validation
  slots as admissible `Applied` children.
- `39ce0818 Finalize broad headless-TUI slots at validated applied batch`:
  fixed the post-apply stall by running declared validation after an applied
  batch and finalizing immediately when it passes, preserving multi-tool-call
  repair when it does not.
- `06e811f5 Make broad-harness admission gate a buildability check`: narrowed
  request-declared validation to `cargo check -p ploke-eval`.

So the post-June-2 failures are not explained by a single stale-file check.
They combine:

1. stricter, correct terminal/validation gating that no longer over-admits
   timed-out or unvalidated patches;
2. a profile that requires five admitted children in one full batch;
3. a latent batch-abort path for provider/database failures that can drop
   already admitted slot evidence before child-plan persistence.

### 2026-06-06: current source boundary for lost partial evidence

Current source path:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
```

Relevant functions:

- `finish_broad_headless_tui_attempt` lines 1647-1757:
  only `HeadlessTerminal::Applied` writes a submitted result; post-apply
  timeout, missing validation, validation failure, and aborted turns correctly
  refuse submission.
- `publish_broad_harness_child_plan_from_admitted_batch` lines 2184-2201:
  if `admitted.len() < child_budget.min`, it persists a failed child-plan with
  rejected attempt evidence, marks the parent failed, then returns
  `InvalidBatchSelection`.
- `admit_broad_harness_batch` lines 4340-4436:
  if any slot returns `PrepareError::ProviderUnavailable` or
  `PrepareError::DatabaseSetup`, it immediately returns that error before
  calling `publish_broad_harness_child_plan_from_admitted_batch`.

This matches `p1-admissionfix-g35flash-p25flash-20260605-195048`:

- `r3` and `r4` reached `Applied`, wrote submitted results, and passed
  declared validation;
- `r10` ended `provider_unavailable` with Google `HTTP_401`;
- no child-plan file was written;
- only the parent node exists; no child `runner-result.json` exists.

The immediate provider-fatal branch existed in the June 2 baseline too. It was
latent there because the latest handoff batch did not hit a provider failure
before producing the required child plan. The later profile and provider/auth
conditions exposed it.

### 2026-06-06: current adapter preserves multi-tool-call turns

The fix should not collapse the model's tool loop into "one edit then stop."
Current `tui_bridge.rs` already has the right shape for this layer:

- `run_attempt` settles completed tool batches and extends the `applied` list;
- after a newly applied batch, request-declared validation is run by the
  harness, not by the model;
- only a classified `HeadlessTerminal::Applied` short-circuits the turn;
- validation failure or missing validation keeps the attempt running so the
  model can repair with more tool calls;
- the `ChatTurnFinished` branch remains a fallback boundary for completed
  turns that did not finalize immediately.

Source lines checked:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs:571-610
crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs:712-825
crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs:852-948
```

So the durable fix should not move validation back into the model's
responsibility and should not depend on the model self-evaluating. The model
may make multiple tool calls; the harness validates the resulting candidate at
bounded applied/stop boundaries.

## Durable Fix Design

The source boundary to fix is batch admission and failed-batch persistence in
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`, not the TUI
tool-calling loop.

### Broken contract

Once broad headless-TUI child planning has produced slot outcomes, the parent
must persist an authority-bearing batch outcome before returning, even when the
batch cannot produce enough children or a later slot exposes a provider or
database blocker.

The record may contain no child transactions, but it must preserve
parent-readable evidence for:

- slots that failed before submission;
- slots that reached provider/database blocker;
- slots that produced valid submitted results but were not materialized because
  the batch as a whole stayed below `child_budget.min`.

### Non-fixes

- Do not admit `AppliedTimedOut`, `AppliedValidationMissing`,
  `AppliedValidationFailed`, `AppliedTurnAborted`, or provider-unavailable
  terminals as children.
- Do not weaken request-declared validation or make the model responsible for
  running it.
- Do not treat provider authentication/quota failures as ordinary no-edit model
  failures that silently spend fresh slots.
- Do not rely on stale submitted-result files as proof of child self-evaluation;
  submitted results are only candidate evidence until a child plan and runner
  results exist.

### Proposed implementation shape

1. Introduce a small batch-attempt ledger inside `admit_broad_harness_batch`.
   Each slot completion should be recorded as one of:
   - admitted submitted result;
   - non-admissible slot with rejection reason;
   - fatal provider/database blocker with reason.
2. Replace the immediate return at the current provider/database branch with a
   finalization path:
   - stop spawning fresh slots;
   - abort or drain remaining running slot tasks;
   - persist the batch outcome before returning.
3. Teach the failed-batch persistence path to include partial-success evidence.
   Existing `surface_attempt::Evidence` can represent the durable attempt
   carrier. For a candidate that was valid but not materialized because the
   batch remained below minimum, store a rejected surface-attempt reason such
   as:

   ```text
   admitted submitted result was not materialized as a child because the broad
   harness batch admitted 2 child transaction(s), fewer than required minimum 5
   ```

   Include the request id, request hash, policy, and submitted-result path in
   the reason or an adjacent typed field if a new schema version is introduced.
4. If `admitted.len() >= child_budget.min`, publish the admitted child plan.
   A late provider failure should still be visible as rejected/fatal attempt
   evidence if the child-plan schema allows mixed children and rejected
   attempts. The configured policy currently gates on `min`, not on perfect
   success for every launched slot.
5. If `admitted.len() < child_budget.min`, persist a rejected-attempt-only child
   plan before returning. Unlike the current `rejected_attempts(&batch,
   &admitted)` path, the persisted evidence must not hide partial admitted
   slots; otherwise the next operator cannot tell "the model produced two valid
   candidates" from "no candidate ever passed validation."
6. Keep `finish_broad_headless_tui_attempt` strict. It should continue to write
   submitted results only for `HeadlessTerminal::Applied`.

### Minimal regression target

Add a focused non-live test around the batch finalization helper, not around a
fresh provider call:

```text
provider_unavailable_after_partial_admissions_persists_failed_child_plan
```

Fixture shape:

- construct a broad-harness batch with `child_budget.min = 5`;
- provide two slots whose submitted-result files are valid and admissible;
- provide one slot outcome equivalent to `PrepareError::ProviderUnavailable`;
- leave the remaining slots as non-admissible or cancelled.

Assertions:

- the function returns a blocker/below-min error rather than creating children;
- `messages/child-plan/<parent>.json` is written;
- the persisted plan has `children = []`;
- persisted surface-attempt evidence includes the provider-unavailable slot;
- persisted surface-attempt evidence also records the two valid-but-not-planned
  submitted results;
- retrying child-plan resolution reuses that child-plan file instead of
  minting a fresh batch.

This test proves the current missing property without live API calls: partial
candidate success plus fatal slot failure cannot erase the parent-readable
batch outcome.

### Live validation after source fix

After the regression passes, run one fresh `prototype1-state` campaign with the
same strict shape that exposed the bug:

```text
model.id = google/gemini-3.5-flash
search.children.min = 5
search.children.max = 5
search.children.parallel_targets = 5
execution.broad_tui.max_attempts = 2
execution.broad_tui.fresh_slots_per_child = 2
```

Expected outcomes:

- if fewer than five slots pass validation, the run writes a durable failed
  child-plan with complete surface-attempt evidence;
- if at least five slots pass validation, the run creates generation-1 child
  nodes and child runner results;
- in either case, a provider/database blocker does not erase already observed
  slot evidence.
