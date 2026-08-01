# Terminal Review: p1-v13-walktargetfix-keeponly-g35f-oropenai-3g1x3-p3-20260716-214818

Date: 2026-07-16

Campaign: `p1-v13-walktargetfix-keeponly-g35f-oropenai-3g1x3-p3-20260716-214818`

Status: terminal, rejected child, no successor handoff

## Verdict

The controller repair in commit `7e72f4752` (`Fix walk target handoff
completion`) passed its first fresh live campaign proof. The admitted walk
session started at R3 and committed every phase through the correct rejected
terminal route, R14a. The controller did not make a second claim after reaching
its target, and the terminal persistence audits both returned `ready`.

This is not yet the requested multi-generation handoff proof. The only admitted
child was rejected, so policy correctly selected `Stop` and persisted
`StopSelectedBranchRejected`. No successor runtime was started. Advancing
through R13b/R14b merely to exercise handoff would have violated the admitted
`require_keep_for_continuation = true` policy.

The run also exposed a more important semantic gap:

- broad-harness commit `398030133` changed only a comment in
  `Error::is_warning`;
- the candidate executable therefore had no behavior change attributable to
  the self-edit;
- its treatment eval produced a benchmark patch that was closer to the gold
  algorithm than the baseline patch, but that variation was stochastic rather
  than a demonstrated effect of the candidate edit;
- selection currently records operational comparison and oracle availability,
  but no semantic-effect or causal-attribution finding connects the self-edit
  to descendant improvement.

The operational regression gate prevented a bad handoff in this run. A future
run could still over-credit a semantically inert candidate if its treatment
sample happened to improve the measured operational counters.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-v13-walktargetfix-keeponly-g35f-oropenai-3g1x3-p3-20260716-214818`
- Seed checkout:
  `/home/brasides/.ploke-eval/setup-seeds/p1-v13-walktargetfix-keeponly-g35f-oropenai-3g1x3-p3-20260716-214818`
- Control journal:
  `prototype1/control/sessions/5036dc337f63348498838b57ff092146319970b0ddd3e754867edf1a1ba47e93/control-journal.jsonl`
- Authority and terminal records:
  `prototype1/transition-journal.jsonl`,
  `prototype1/evaluations/branch-05f5e5d80cf78022.json`, and
  `prototype1/nodes/node-f79bb67fc45c0e3b/reports/state-report.json`
- Baseline run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-v13-walktargetfix-keeponly-g35f-oropenai-3g1x3-p3-20260716-214818/BurntSushi__ripgrep-2209/runs/run-1784264140929-structured-current-policy-48016eac`
- Admitted broad-harness workspace:
  `prototype1/workspaces/edit-harness/node-f79bb67fc45c0e3b`
- Treatment run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-v13-walktargetfix-keeponly-g35f-oropenai-3g1x3-p3-20260716-214818/treatments/branch-05f5e5d80cf78022/instances/BurntSushi__ripgrep-2209/runs/run-1784265331014-structured-current-policy-5adbca40`
- Benchmark issue, gold patch, and tests:
  `slice.jsonl`, instance `BurntSushi__ripgrep-2209`

No sqlite database was opened for this review.

## Profile And Execution Path

The admitted profile used direct Google `google/gemini-3.5-flash` for eval and
protocol, one benchmark instance, three parallel broad-harness targets,
`max_generations = 3`, and `require_keep_for_continuation = true`.

The control session's setup receipt records seed checkout
`7e72f4752ead36325fc3f46aec157028cd4157d6`. Setup then created parent commit
`1cf661f4e8d0fbc934900e3edbfb615d4f636062`; every live transition epoch used
that generated parent head and the same source-status digest.

The active execution path was:

```text
loop walk step
  -> WalkController::advance_until
  -> Prototype 1 phase transition
  -> baseline run_benchmark_turn + protocol
  -> broad tui_adapter::run_headless_with_model fanout
  -> admitted child build and treatment run_benchmark_turn + protocol
  -> branch evaluation
  -> history-traversal selection
  -> rejected terminal route
```

The broad-harness parent produced three slots:

| Slot | Terminal result | Descendant effect |
| --- | --- | --- |
| base | applied | admitted comment-only commit `398030133` |
| r2 | exhausted | no-progress guard after 15 consecutive `request_code_context` calls |
| r3 | completed without edit | no candidate |

Only the base slot created child `node-29706df262a3823b` /
`branch-05f5e5d80cf78022`.

## Walk-Control Proof

Control session `5bd3fb86-e446-4f08-92ad-ab3da19f0922` committed 13 phase
receipts after its R3 start:

```text
R3 -> R4a -> R4b -> R4c -> R5 -> R6 -> R7
   -> R8 -> R9 -> R10 -> R11 -> R12 -> R13a -> R14a
```

Each `finished` control record has `status = committed`, the expected phase,
and a distinct evidence digest. The final phase evidence is
`5074993887a3d039a621df7e34f9c8d4512a6d30ec335673817c822cb9e53063`.
The session then released fence 14 without claiming another transition.

This is the live confirmation for the v12 replay-backed controller repair:
reaching a requested terminal target now ends the operation instead of making
one extra claim from the predecessor runtime.

### Persistence audits

Two read-only verbose audits ran after R14a:

| Transition | Verdict | Persisted evidence |
| --- | --- | --- |
| `r12-to-r13a` | `ready`, `overall_status = ok` | one stopped journal record, one continuation JSONL record, one `eval_continuation_decision` |
| `r13a-to-r14a` | `ready`, `overall_status = ok` | one parent-complete journal record and one state report |

The R12-to-R13b handoff audit was not run because the branch was rejected and
handoff was correctly skipped.

## Closure State

The baseline and treatment campaigns are both mechanically complete:

| Layer | Baseline | Treatment |
| --- | ---: | ---: |
| Eval | 1/1 complete | 1/1 complete |
| Tool-call intent segmentation | complete | complete |
| Tool-call review | complete | complete |
| Tool-call segment review | complete | complete |
| Patch projection | passed, non-empty | passed, non-empty |
| Oracle evaluated instances | 0 | 0 |

Mechanical closure proves that the provider, tool, record, protocol, and branch
evaluation paths ran. It does not prove that either benchmark patch is correct
or that the candidate self-edit caused an improvement.

## Baseline Eval And Patch

The bundled trace audit found:

- 24 provider responses: 23 `tool_calls`, then one `stop`;
- 23 provider-emitted tool calls and 23 recorded tool calls;
- no missing or extra call ids;
- 23 protocol call reviews and nine usable segment reviews.

The model localized `Replacer::replace_all`, replaced
`replace_with_captures_at` with a capture iterator, reacted to failing tests,
and ended with a passing workspace `cargo test`. That is a real
edit-test-repair chain, but the exported algorithm remained wrong.

The final submission:

1. duplicates the same `subject = &subject[..range.end + MAX_LOOK_AHEAD]`
   truncation line;
2. stops capture iteration when `m.start() >= range.end`;
3. then appends `subject[last_match..]`, including the extended look-ahead tail.

The benchmark gold patch instead appends only through
`min(bytes.len(), range.end)`. The baseline tail therefore retains the same
over-extension the issue asks the patch to prevent. Intermediate edits touched
`standard.rs`, but the final exported patch contains no regression test for
issues 2095/2208.

The last passing test is useful mechanical evidence, not semantic proof: it did
not run the benchmark's issue-shaped regression tests against the exported
patch.

## Baseline Final-Message Projection Bug

The final answer exists but is lost by the summary projection.

Concrete trace chain:

1. `llm-full-responses.jsonl` response index 23 has
   `finish_reason = stop`, 1,048 bytes of assistant content, and provider
   response id `mrZZapKqK7SM9LsPsfGjuAw`.
2. The provider sidecar associates it with assistant message id
   `c190d455-06a6-4bc6-9eca-f9132b52c5b5`.
3. `TurnFinished` also points at `c190d455-...`.
4. The raw trace then creates the actual final content under a different id,
   `f7f7a40a-320f-4e99-8a6d-1263403e1d0f`.
5. The final update for `c190d455-...` still contains only
   `Calling tools...`.
6. `agent-turn-summary.json` persists `final_assistant_message = null`.

Classification: **record present, manual join needed**. This is not a missing
provider answer. The writer/read-side join uses an id that does not own the
final content. The treatment run repeats the same `final_assistant_message =
null` symptom after its own provider `stop` response.

Broken contract: a terminal provider response represented in the recorded event
stream must project to the same completed assistant message referenced by
`TurnFinished`.

Current coverage gap: this campaign is a persisted repro, but no replay
regression yet proves that the production projection joins the two ids and
retains the final answer.

## Protocol Review And Blind Spot

All required baseline protocol procedures completed, but they judged local tool
use rather than final patch semantics.

For example, call 15's `non_semantic_patch` was labeled `key_progress` with high
confidence because a preceding test failed and the following test passed. The
protocol did not compare the resulting diff to the benchmark gold boundary,
notice the unbounded final tail, or require the two issue-shaped regression
tests. It therefore over-credited a mechanically successful repair loop.

Broken contract: protocol completion must not be interpreted as semantic patch
success when the procedure reviewed tool-call usefulness but did not inspect
the final benchmark behavior.

The protocol records themselves are present. The gap is in adjudication scope,
not artifact persistence.

## Candidate Self-Edit

The admitted child commit was:

```diff
 pub fn is_warning(&self) -> bool {
+    // Walktarget optimization flag
     matches!(self, Error::Warning(_))
 }
```

Commit `3980301335153c94582bd82da841a89d8f2bb0ff` changes one line in
`crates/ploke-error/src/lib.rs`. It changes no executable behavior and is
unrelated to benchmark patch generation. The harness nevertheless:

- applied and committed it;
- passed the declared `cargo check -p ploke-eval`;
- built it as child `node-29706df262a3823b`;
- ran a full treatment eval and protocol campaign from it.

This is mechanically valid under the current surface and compile contract, but
semantically inert.

## Treatment Eval

The treatment benchmark patch is closer to the gold algorithm than the
baseline:

- it introduces a `limit`;
- multi-line replacement keeps `limit = range.end`;
- capture iteration stops at that limit;
- the trailing bytes use `subject[last_match..limit]`.

That directly addresses the baseline's unbounded-tail error. It also adds a
unit test, although the test's simple `hello\n` pattern does not reproduce the
benchmark's extended look-around cases.

The treatment still has quality and operational regressions:

- `replace_all` loses its documentation and is emitted with malformed
  indentation;
- the validation audit records five of nine edit requests as test-scoped;
- intermediate edits changed test regexes and expected output;
- `insert_rust_item` failed once;
- same-file retries increased from 2 to 4;
- the maximum same-file streak increased from 2 to 5.

The final workspace test passed, and the patch is plausibly a semantic
improvement over the baseline. It is not evidence that the comment-only
self-edit improved the runtime, because that edit cannot affect the treatment
behavior.

## Evaluation, Selection, And Correct Stop

`prototype1/evaluations/branch-05f5e5d80cf78022.json` persisted
`overall_disposition = reject` for three regressions:

| Metric | Parent | Child |
| --- | ---: | ---: |
| `tool_calls_failed` | 0 | 1 |
| `same_file_patch_retry_count` | 2 | 4 |
| `same_file_patch_max_streak` | 2 | 5 |

The transition journal then persisted:

- selected branch `branch-05f5e5d80cf78022`;
- selected branch disposition `reject`;
- selection outcome `stop`;
- disposition `stop_selected_branch_rejected`;
- operational verdict `worse`, confidence `high`;
- oracle verdict `inconclusive`, confidence `low`.

The state report agrees:

```text
completed:Reject;children_ran=1;children_planned=1;selection=Stop;
successor=node-29706df262a3823b;
successor_handoff=skipped:StopSelectedBranchRejected
```

It records `successor_runtime = null`, `successor_pid = null`, and no
successor-ready path. No live handoff occurred, and that is the correct policy
outcome.

## What Is Working

- Commit `7e72f4752` fixed target completion without weakening executable,
  source-status, profile, or evidence-digest checks.
- A fresh live walk session traversed R3 through R14a under the repaired
  controller.
- Baseline and treatment provider call ledgers have exact recorded-call parity.
- Eval, protocol, branch evaluation, selection, stop, and parent-complete
  records all persisted.
- Both terminal persistence audits are ready.
- The keep-only continuation policy prevented a rejected child from becoming a
  successor runtime.
- CLI trace and phase records provide enough evidence to reconstruct this run
  without reading sqlite state.

## What Is Not Working Yet

- Multi-generation parent/successor handoff remains unproven by this campaign.
- Final assistant content still requires a manual id join despite a provider
  `stop` response and completed turn.
- Baseline protocol completion can over-credit an edit-test loop whose final
  patch is semantically wrong.
- Broad-harness admits a comment-only, behaviorally inert self-edit as a child
  candidate.
- Selection has no explicit semantic-effect or causal-attribution policy tying
  descendant improvement to the candidate self-edit.
- Oracle evidence was configured as record-only but evaluated zero instances;
  it did not resolve patch correctness.

## Action Items

### Before claiming multi-generation completion

1. Start another fresh campaign from the unchanged strict validation policy.
2. Require at least one behavior-changing, task-relevant self-edit to survive
   branch evaluation.
3. Exercise R12-to-R13b, verify predecessor/successor ownership handoff, then
   continue at least one full successor generation.
4. Run the R12-to-R13b and R13b-to-R14b persistence audits before calling the
   multi-generation proof complete.

### Observability and replay

1. Add a historical replay regression for the final-message id split:
   provider/`TurnFinished` id `c190d455-...` versus content-bearing id
   `f7f7a40a-...`.
2. Keep the raw provider response, trace, and summary as separate surfaces in
   the UI until the writer-side join is fixed.
3. Surface “protocol complete” separately from “final patch semantically
   reviewed.”

### Selection policy

1. Persist a candidate semantic-effect classification such as
   `behavior_change`, `test_only`, `comment_only`, or `unknown`.
2. Require causal attribution before interpreting a better treatment sample as
   self-improvement. A comment-only runtime should not receive credit for
   stochastic descendant variation.
3. Add final-patch semantic adjudication or issue-shaped test/oracle evidence
   rather than treating passing local tests as sufficient.
4. Preserve the existing strict stop and digest validation checks; do not solve
   this by permitting rejected handoff or rewriting sealed history.

## Related Records

- [`2026-07-16-walk-until-target-second-claim.md`](../../bugs/2026-07-16-walk-until-target-second-claim.md)
  contains the fixed controller contract and v12 replay evidence.
- [`2026-07-16-prototype1-r12-policy-stop-routing.md`](../../bugs/2026-07-16-prototype1-r12-policy-stop-routing.md)
  covers R12 keep-policy routing and step-mode terminal behavior.
- [`2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md`](../../bugs/2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md)
  tracks the existing final-message and validation-summary family of gaps.
