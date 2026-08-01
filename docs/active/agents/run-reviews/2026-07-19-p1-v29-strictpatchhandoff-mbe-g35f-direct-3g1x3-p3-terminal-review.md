# P1 v29 Three-Generation Live Walk Terminal Review

Date: 2026-07-19

Campaign:
`p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018`

Status: complete at R14a; two successful live successor handoffs; final
selection stopped on a rejected branch.

## Verdict

V29 is the first fresh live proof in this implementation thread that combines:

- a strict reviewed-admissible patch gate;
- all-resolved MBE evidence for every treatment;
- three parallel children in each of three generations;
- two parent-to-successor runtime handoffs on the same walk-controlled run;
- reconstruction and advancement after the child plan filled
  `max_total_nodes = 10` exactly;
- a final R12 -> R13a -> R14a stopped report; and
- bounded process memory with every treatment child reaped.

The live run completed without weakening a hash, receipt, oracle, patch, or
node-budget check. It proves the repairs in:

- `6384557e3` (`Prevent dense indexing in sparse eval runtimes`); and
- `f8475f35b` (`Fix replay budgeting at filled node cap`).

The terminal reason needs to remain precise. Generation 3 produced one `Keep`
and two `Reject` branches. History traversal sampled rejected
`node-04be280c6e823de5`, so strict continuation recorded
`StopSelectedBranchRejected`. The run was also at the configured
three-generation / ten-node boundary, but the emitted report does not claim
that the cap itself selected the stop.

This is a successful loop-control and handoff proof. It is not evidence that
every self-edit improved the benchmark agent. Selection kept two branches,
while two other Keep branches remained unselected siblings. Five branches were
operational Rejects, and final traversal stopped on one of those rejected
coordinates.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018`
- Seed checkout:
  `/home/brasides/.ploke-eval/setup-seeds/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018`
- Run profile:
  `/home/brasides/.ploke-eval/profiles/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018.toml`
- Transition journal:
  `prototype1/transition-journal.jsonl`
- Final report:
  `prototype1/nodes/node-25e372fee4efe3eb/reports/state-report.json`
- Branch evaluations:
  `prototype1/evaluations/`
- Per-node MBE reports:
  `prototype1/nodes/<node>/mbe/final_report.json`
- Baseline run:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018/BurntSushi__ripgrep-2209/runs/run-1784445388357-structured-current-policy-d9446633`
- Final selected treatment:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018/treatments/branch-3ed295e40e2449b5/instances/BurntSushi__ripgrep-2209/runs/run-1784454746768-structured-current-policy-b49824a8`
- Final selected treatment protocol:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018/treatments/branch-3ed295e40e2449b5/instances/BurntSushi__ripgrep-2209/runs/run-1784454746768-structured-current-policy-b49824a8`

No sqlite database was opened for this review.

## Admission And Profile

The fresh seed checkout started from committed repair HEAD `f8475f35b`.
The admitted profile SHA-256 is:

```text
d44e6cd497e56a2846c31aeb6350eb32c732d28b54fc28f0aaaf22a34827d3c8
```

The setup preview and admitted plan shared SHA-256:

```text
198ffc56c43f7295f49c6845199d0fda296f7aabb7093a4615a23c5a04c538c3
```

The important admitted settings were:

- direct Google `google/gemini-3.5-flash` for eval and protocol;
- OpenRouter Codestral embeddings;
- `max_generations = 3`;
- `max_total_nodes = 10`;
- one to three children, parallel cap three, full-batch scheduling;
- `require_keep_for_continuation = true`;
- `oracle_gate = "all-resolved"`;
- `patch_gate = "reviewed-admissible"`;
- protocol token limit 8,000 and review parallelism one;
- MBE enabled;
- broad-harness maximum attempts three, two fresh retry slots, and 1,800-second
  timeout; and
- 2,400-second child observation allowance.

Live doctor checks passed provider, embedding, and headless-TUI preflights
before the first walk transition.

## Execution Path

The operator path was:

```text
loop walk step
  -> WalkController phase advancement
  -> baseline run_benchmark_turn + protocol
  -> tui_adapter::run_headless_with_model three-slot fanout
  -> admitted candidate commits
  -> child treatment run_benchmark_turn + protocol
  -> MBE
  -> operational branch comparison
  -> History traversal selection
  -> R13b successor install/handoff, or R13a stopped continuation
```

The walk CLI remained the mutation client throughout. Each successor published
a new walk endpoint for the same seed checkout and campaign authority.

## Generation And Branch Outcomes

The campaign contains one root node and nine treatment nodes:

| Generation | Node | Branch disposition | MBE | Role |
| ---: | --- | --- | --- | --- |
| 0 | `node-1a691618c3c206b3` | parent | baseline complete | initial parent |
| 1 | `node-a7b27c210257b17e` | Keep | 1/1 resolved | selected successor |
| 1 | `node-b3509eea225d3402` | Reject | 1/1 resolved | sibling |
| 1 | `node-ddbf17eb613058d0` | Keep | 1/1 resolved | sibling |
| 2 | `node-25e372fee4efe3eb` | Keep | 1/1 resolved | selected successor |
| 2 | `node-29e49e4993edb91b` | Reject | 1/1 resolved | sibling |
| 2 | `node-25d1a0f8ad347bbe` | Reject | 1/1 resolved | sibling |
| 3 | `node-347b7edaccb15619` | Keep | 1/1 resolved | unselected sibling |
| 3 | `node-2c04a0f6c62917de` | Reject | 1/1 resolved | sibling |
| 3 | `node-04be280c6e823de5` | Reject | 1/1 resolved | sampled stop coordinate |

All nine runner results have `status = succeeded` and `exit_code = 0`.
All nine treatment closure states have eval and protocol `complete`, with one
full protocol instance and no failed or missing instance.

## Live Successor Handoffs

### Generation 0 -> 1

History selected Keep branch `branch-0025844cd2a6d0e4`. The parent installed
commit `8f44c280526fa511511539f0b62bc91db2b235c1`, retired the original runtime,
and started:

```text
runtime 109c494c-fdd4-4b00-bf0c-a5f1d1879017
pid     2176488
phase   R4c
session c9d38cb4-3b70-4a36-8cc4-cd76cff18f1a
```

The ready record includes both the new endpoint and predecessor endpoint,
which is the durable handoff proof rather than an inference from process ids.

### Generation 1 -> 2

History selected Keep branch `branch-9d217ec58ac8ca8e`. The parent installed
commit `70be23e9d1b0b8bbdaaf35e942bc5fccd46a23fc`, retired runtime
`109c494c-...`, and started:

```text
runtime 8bb89724-76d8-4b54-b273-2930a967ec38
pid     2335459
phase   R4c
session 7bdacf8e-c9ea-40b4-8171-6e78e1d8651b
```

That ready record names runtime `109c494c-...` as predecessor. The final
transition journal later records runtime `8bb89724-...` completed with
`status = succeeded`.

## Exact Node-Cap Reconstruction Proof

The predecessor V28 campaign failed after its final legal three-child plan
filled the node budget: durable reconstruction reapplied the live admission cap
to an already admitted plan and fell back to R6 rather than recovering R8.
That run was frozen; no source, digest, or receipt was changed.

Commit `f8475f35b` separates strict live admission from validation of an
already persisted plan. V29 exercised the same boundary live:

1. generation 2 entered R6 with seven typed nodes already present;
2. the legal three-child plan was admitted;
3. all three broad slots completed;
4. the plan brought the durable total to exactly ten nodes;
5. reconstruction recovered R8 rather than rejecting the persisted plan; and
6. walk job 13 advanced R8 -> R9 -> R10 successfully.

This is the requested live proof for the repair. It did not raise the cap or
make replay permissive.

## Terminal Walk State

After all generation-3 treatment runs completed:

- R10 -> R12 assembled three decision-grade child outcomes;
- R12 -> R13a recorded the stopped continuation at journal revision 43;
- R13a -> R14a emitted parent-complete evidence at journal revision 47; and
- R14a exposed no admitted next step.

The final state report records:

```text
completed:Reject
children_ran=3
children_planned=3
selection=Stop
successor=node-04be280c6e823de5
successor_handoff=skipped:StopSelectedBranchRejected
```

The selected branch was MBE-resolved but operationally worse:

- `tool_calls_failed`: 1 -> 2;
- `same_file_patch_max_streak`: 0 -> 1.

The oracle finding was still `better` because the benchmark resolved. Strict
continuation correctly kept the operational Reject authoritative.

## Final Treatment Trace Reconstruction

The bundled trace audit for run
`run-1784454746768-structured-current-policy-b49824a8` found:

- 50 provider responses;
- 49 provider-emitted tool calls;
- 49 recorded tool calls;
- no missing or extra call ids;
- 49 tool-call finishes followed by one `stop`; and
- 11 completed calls, 20 content-bearing reads, 10 context results, six
  duplicate requests, and two recorded failures.

One concrete edit-recovery chain is:

1. response 18 used `code_item_lookup` to resolve `replace_all`;
2. response 19 called `apply_code_edit` with invalid method canon
   `crate::util::replace_all`, which failed;
3. response 20 explicitly corrected the canon to
   `crate::util::Replacer::replace_all`;
4. call `function-call-8ae5b06c-...` applied the production edit;
5. responses 21-23 ran compile and test validation;
6. response 43 attempted `insert_rust_item` for the regression test, which
   failed because the module target was unsuitable;
7. response 44 switched immediately to `non_semantic_patch`, and call
   `function-call-1d10472a-...` applied the test;
8. responses 45, 46, and 48 ran successful test commands; and
9. MBE later verified 276 passing tests, including fixes for regressions 2095
   and 2208.

This is genuine learning and recovery rather than transport-only completion.
The last point where the model had enough information to act was response 42:
it had reached the end of `standard.rs`, then added and validated the
issue-shaped test.

The persisted patch was independently checked in the MBE workdir. It changes
`crates/printer/src/util.rs` and `standard.rs`, bounds the replacement tail at
`end_bound`, and adds `regression_replacement_duplicative`. The validation
audit confirms the final cargo call covered both changed files.

Two patch-quality limitations remain:

- no formatting check was recorded; and
- the persisted production patch has visibly poor indentation and drops the
  `replace_all` doc comment.

MBE success establishes benchmark usefulness, not polished source quality.

## Protocol Review

The baseline protocol reviewed all 57 calls and six usable segments. The final
treatment protocol reviewed all 49 calls and four usable segments. All three
required procedures completed for the baseline and every treatment.

The final segment reviews correctly captured both sides of the trace:

- the first failed semantic edit was followed by a corrected canon and
  successful validation;
- the failed test insertion was followed immediately by an applied patch and
  successful tests; and
- the middle search segment was labeled `search_thrash` because it issued
  eight similar searches and large overlapping reads.

However, every selection record says
`protocol_run_snapshots=0` even though the protocol artifacts and closure
counts are present. Classification: **record present, manual join needed**.
The run-review had to join closure-state protocol roots to the selection
record; the selected-candidate payload does not expose the snapshots it says
are absent.

This did not block selection because the operational and oracle gates were
complete. It is still an important UI/read-side gap: an operator looking only
at selection evidence cannot drill directly into the adjudications that
already exist.

## Runtime Lifecycle And OOM Regression

The sparse headless runtime repair prevented the previous detached dense
indexer lifecycle. Across all three generations:

- each broad batch ran three slots;
- nine treatment child pids reached ready, evaluating, and result-written
  journal states;
- every child exited and was reaped;
- no campaign `ploke-eval` process remained after walk shutdown; and
- the final server check showed about 545 MiB RSS with 27 GiB system memory
  available.

These process and memory checks are operator/convenience evidence, not a
persisted peak-memory metric. They are sufficient to show that V29 did not
reproduce V26's runaway detached-runtime behavior.

One transient direct-Google HTTP 429 occurred in the final batch and recovered
under the existing retry policy. The child, eval, protocol, and MBE records all
completed, so it is provider retry evidence rather than a child failure.

## Observability Gaps

### Selected parent `node.json` status remains `running`

The root and the two selected successor node records still say `running` after
the terminal report, even though History, successor-ready/completed records,
R14a, and the host process table prove they are no longer live.

Classification: **record present, playback gap**. The authoritative lifecycle
is reconstructable, but a UI that treats `node.json.status` as current truth
will mislabel completed parent nodes.

### Final message projection remains null

The final treatment has a provider `stop` response with a substantive answer,
but `agent-turn-summary.json` still stores
`final_assistant_message = null`.

Classification: **record present, manual join needed**. The provider response
and tool lifecycle are present; the summary projection does not join the final
assistant content. This is the previously observed final-message projection
gap, not missing provider output.

## What Is Working

- UI and CLI can share the walk server's durable cursor and transition journal.
- Strict setup/profile commitments, hash checks, node caps, patch review, and
  oracle gates survive live handoff.
- The successor publishes a new endpoint with predecessor identity and resumes
  at R4c.
- Three-wide child execution completes without detached TUI runtimes.
- All eval, protocol, branch comparison, and MBE surfaces persisted for all
  nine children.
- The repaired replay budget distinguishes old admitted facts from new live
  admission without weakening either.
- R13a/R14a explain the final stopped continuation and emit parent-complete
  evidence.

## Follow-Up Actions

1. Project successor-ready/completed and parent-complete evidence into a
   terminal node lifecycle status so the UI does not show dead selected parents
   as `running`.
2. Add the protocol artifact roots or typed snapshot references to selection
   playback; do not report `protocol_run_snapshots=0` when the records exist.
3. Repair final assistant-message projection by replaying the persisted
   provider/message-id join.
4. Surface the distinction between “sampled branch was Reject” and “campaign
   also reached its configured generation/node boundary” in the UI stop card.
5. Add formatting evidence or a patch-quality gate if polished source output
   is required in addition to benchmark resolution.

None of these follow-ups invalidates the live handoff, exact-cap
reconstruction, or terminal-control proof recorded here.

## Repair Verification And Cleanup

Before V29, the exact V28 historical R8 replay reproduced the node-cap failure.
After `f8475f35b`:

- the exact historical replay passed;
- the synthetic filled-cap replay passed;
- three strict live budget tests passed;
- 16 reconstruction tests passed, with the artifact-gated historical test
  ignored in the broad suite; and
- `cargo check -p ploke-eval` passed with existing warnings.

After R14a, the walk server exited cleanly. `cargo clean` removed 10,024 files
and 10.7 GiB from the seed target. No target directory or campaign process
remained. The 1.3 GiB campaign artifact tree and its histories were preserved.
