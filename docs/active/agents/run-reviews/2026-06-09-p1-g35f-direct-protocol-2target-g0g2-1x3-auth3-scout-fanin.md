# Scout fan-in: p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113

Status: scout fan-in / incomplete run review. This is not a successful-run
review. It preserves the red flags found by parallel scouts and marks the
campaign as requiring blocker repair before reuse.

## Short Verdict

The campaign produced real broad-harness patch attempts, real eval run roots,
and a sealed gen1 History selection. It should not be treated as benchmark
useful or cleanly closed.

The sealed gen1 selection chose `node-0dae679bb16a4604` /
`branch-b7dcbe2fd935fa16` even though the branch evaluation says
`overall_disposition = reject`. The selection formula used
`metric_inputs = operational_and_protocol`, but protocol evidence was incomplete
for treatment 2209 runs and oracle evidence was inconclusive. The selected
successor then failed during child completion, and the later gen2 child result
left contradictory state: `runner-result.json` says succeeded, `node.json`
remains `binary_built`, branch evaluation is absent, and no gen2 History
selection was sealed.

## Evidence Roots Checked

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113`
- Target instances: `BurntSushi__ripgrep-2209`,
  `BurntSushi__ripgrep-2295`
- Baseline run roots:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/BurntSushi__ripgrep-2209/runs/run-1781022565306-structured-current-policy-fed2b654`
  and
  `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/BurntSushi__ripgrep-2295/runs/run-1781022315278-structured-current-policy-b2931568`
- Baseline run registrations:
  `/home/brasides/.ploke-eval/registries/runs/run-1781022565306-structured-current-policy-fed2b654.json`
  and
  `/home/brasides/.ploke-eval/registries/runs/run-1781022315278-structured-current-policy-b2931568.json`
- Broad-harness surfaces:
  `prototype1/messages/edit-harness-request/*.json`,
  `prototype1/messages/edit-harness-result/*.json`,
  `*.headless-tui.json`, `*.turn-live/*`, and
  `prototype1/workspaces/edit-harness/*`
- Selection and lifecycle surfaces:
  `prototype1/history/blocks/segment-000000.jsonl`,
  `prototype1/transition-journal.jsonl`,
  `prototype1/evaluations/*.json`, and
  `prototype1/nodes/*/{node.json,runner-result.json,channels/*/child-to-parent.jsonl}`

## Execution Paths

Gen1 selection path:

```text
baseline eval/protocol roots for 2209 and 2295
  -> branch evaluations under prototype1/evaluations/
  -> sealed History block 5782a6ef-6874-43f2-bf66-a8a3645f1b90
  -> selection entry e460ad73-a4b3-472c-859f-914940e7f375
  -> selected candidate:node-0dae679bb16a4604:plan_index=1
  -> successor runtime d38b78ed-724e-4536-9235-47fc67933c18
```

Late gen2 child path:

```text
prototype1-state
  -> leaf_runner_argv
  -> Prototype1RunnerCommand::run
  -> execute_prototype1_runner_invocation
  -> run_prototype1_resolved_branch_treatment
  -> advance_eval_closure
  -> advance_protocol_closure
  -> build_prototype1_treatment_evidence
  -> child-to-parent Result for runtime a7691248-60cb-419f-9a3a-52670fcb7e08
  -> runner-result.json status=succeeded
  -> transition-journal observe_child:before without observe_child:after
  -> no branch evaluation or gen2 History selection for branch-3e52c562999775da
```

## Concrete Trace Chains

Broad-harness trace:

```text
broad-harness-request:node-0dae679bb16a4604
  -> submitted proposal 47542366-2641-56dd-9ba6-da1d9d12545f
  -> workspace commit 2559170a
  -> applied edit evidence present
  -> later successor selected node-0dae679bb16a4604
  -> child completion failed first by timeout, then by git add status 128
```

Selection trace:

```text
History selected candidate:node-0dae679bb16a4604:plan_index=1
  -> branch-b7dcbe2fd935fa16 evaluation overall_disposition=reject
  -> metric formula score_child_prop uses operational_and_protocol
  -> diagnostics report missing treatment protocol aggregate for 2209
  -> oracle finding is inconclusive
```

Gen2 stale-observe trace:

```text
node-7809adc3fc6e4aad invocation a7691248-60cb-419f-9a3a-52670fcb7e08
  -> channel ready/evaluating/Result
  -> runner-result.json succeeded
  -> node.json still binary_built
  -> no branch-3e52c562999775da evaluation report
  -> no sealed gen2 selection block
```

## Record Surface Status

| Surface | Label | Notes |
| --- | --- | --- |
| Campaign manifest, run profile, closure state | present | `direct_google` / `google`; eval/protocol baseline closure is mechanically complete. |
| Baseline run roots | present, manual join needed | Run roots include `record.json.gz`, turn summaries/traces, raw responses, validation audit, patch projection, and submission files. |
| Broad request files | present | Base and retry request files exist for both broad nodes; r4-r9 are request-only/stale surfaces. |
| Broad result and headless traces | mixed | Present for `node-0dae...` base/r2/r3 and `node-71...` r2/r3; absent for `node-71...` base and r4-r9. |
| Candidate workspaces | mixed | Present for most materialized candidates; `node-71b405858b38a913-r3` result claims applied edits but workspace is absent. |
| Validation output | record present, playback gap | Headless traces retain summaries; `debug_relay` reports dropped/truncated output. |
| Protocol artifacts | present, manual join needed | Baseline protocol complete; treatment protocol aggregate missing for some 2209 comparisons used by selection. |
| Evaluation and selection | present, manual join needed | Gen1 selection sealed a rejected branch. |
| Gen2 selection | record absent | Gen2 child result exists, but no branch evaluation or History selection was sealed. |
| Run-root model ledgers | present, manual join needed | Registered eval roots have non-empty `llm-full-responses.jsonl`; trace audit found no missing provider call ids. |
| Raw model/provider provenance | record present, manual join needed | Campaign/profile say `direct_google`; run records show `google/gemini-3.5-flash` with provider `google`; registered summaries leave `model_route` null. |
| Edit-harness model sidecars | record present, manual join needed | Sidecars exist, but five `llm-full-responses.jsonl` files are zero-length and one ends in HTTP 429. |

## Red Flags

- Mechanical completion is not benchmark usefulness. The selected gen1 branch
  was rejected, oracle evidence was inconclusive, and protocol evidence used by
  selection had missing treatment aggregates.
- The broad-harness records include real patch attempts, but also stale
  request-only files, a missing candidate workspace for an applied-result claim,
  and truncated validation visibility.
- Successor lifecycle failed after selection: child completion first timed out
  waiting for `node-7809adc3fc6e4aad`, then failed during
  `prototype1_child_artifact_commit` with `git add -- <paths>` status 128.
- Gen2 is not cleanly closed: terminal runner evidence exists without the
  matching parent observation, branch evaluation, or sealed History selection.
- Registered eval roots show real model/tool progress, but their aggregate
  completion is not enough: 2209 final validation did not cover changed files,
  2295 has a checkout/base mismatch between benchmark diff base and checkout
  head, and both summaries complete with `final_assistant_message = null`.
- Campaign edit-harness sidecars are not sufficient raw-provider evidence by
  themselves: five sidecar provider ledgers are empty, and the
  `node-71b405858b38a913` sidecar ends in `HTTP_429 RESOURCE_EXHAUSTED`.

## Follow-Up

Treat this as blocker-repair input, not as a run to resume normally. The
minimum repair coverage is:

- replay the late terminal child result through the parent comparison/evaluation
  boundary without re-running child materialize/build;
- require branch evaluation evidence before direct `prototype1-state` re-entry
  treats a terminal successful child as selectable;
- make selection/protocol accounting reject or explicitly mark partial
  treatment protocol evidence when the formula uses protocol inputs;
- preserve full-enough validation output to verify applied broad-harness edits
  against changed files.

Related bug report:
[`2026-06-09-prototype1-late-child-result-recovery-corrupts-node-state.md`](../../bugs/2026-06-09-prototype1-late-child-result-recovery-corrupts-node-state.md).
