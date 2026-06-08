# Recent Prototype 1 Node Coverage

Date: 2026-06-08.

Purpose: identify which nodes in the recent Prototype 1 campaigns have only
status/scout coverage versus which still need quality-gated run reviews under
the run-review skill.

## Coverage Standard

A full run review is counted only when it satisfies the run-review quality gate:
it names the execution path, reconstructs at least one concrete
model/tool-to-edit-or-failure chain, separates mechanical completion from
benchmark usefulness, verifies an important tool result against a checkout or
persisted artifact, identifies the last point where the model had enough
information to act, and ties follow-ups to observed artifact gaps or protocol
blind spots.

Terminal reports, operator logs, and scout reports are useful evidence, but they
do not by themselves count as full run-review coverage.

For each campaign, use these artifact roots before writing a missing-record
claim:

- Campaign state: `~/.ploke-eval/campaigns/<campaign>/prototype1/`
- Node/runtime state: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node>/`
- Branch evaluation: `~/.ploke-eval/campaigns/<campaign>/prototype1/evaluations/<branch>.json`
- Treatment run roots:
  `~/.ploke-eval/instances/prototype1/<campaign>/treatments/<branch>/instances/BurntSushi__ripgrep-2209/runs/`
- Treatment protocol roots:
  `~/.ploke-eval/protocol/prototype1/<campaign>/treatments/<branch>/instances/BurntSushi__ripgrep-2209/runs/`
- Baseline run roots:
  `~/.ploke-eval/instances/prototype1/<campaign>/BurntSushi__ripgrep-2209/runs/`
- Baseline protocol roots:
  `~/.ploke-eval/protocol/prototype1/<campaign>/BurntSushi__ripgrep-2209/runs/`
- Broad parent request/result surfaces:
  `~/.ploke-eval/campaigns/<campaign>/prototype1/messages/edit-harness-{request,result}/`

Do not open sqlite/db files directly; treat them as path witnesses.

## Current Coverage Summary

| Campaign | Existing durable coverage | Full run-review coverage status |
| --- | --- | --- |
| `p1-selectfix-replay-5g1x2-a2-20260607-224502` | Terminal status, operator log, authority scout, broad/eval scout. | Not complete. Scouts cover campaign structure and final gen4 evidence, but full reviews are still needed for branch-level semantic/tool usefulness. |
| `p1-handofffix-embed-5g1x2-a2-20260607-192954` | Clean termination report. | Not complete. The report proves clean stop and timings, not trace-level child usefulness. |
| `p1-nokeep-handoff-5g1x2-a2-20260607-192811` | Failure termination report. | Not complete. The report proves the payload-hash terminal failure after gen2 children; it does not fully review child traces. |
| `p1-historyfix-handoff-5g1x2-a2-20260607-155010` | Live-status/root-cause report plus failure termination report. | Not complete. The reports prove missing-workspace successor failure, not full child trace usefulness. |
| `p1-rejected-handoff-5g1x2-a2-20260607-191057` | Setup/pre-child incomplete-evidence report. | No child-node run-review coverage applicable; only baseline/root incomplete-state review remains possible. |
| `p1-handofffix-5g1x2-a2-20260607-190702` | Setup/pre-child failure report. | No child-node run-review coverage applicable; baseline failed at embedding preflight before broad children. |

## `p1-selectfix-replay-5g1x2-a2-20260607-224502`

Existing reports:

- [`2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/terminal-status.md`](2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/terminal-status.md)
- [`2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/authority-status-scout.md`](2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/authority-status-scout.md)
- [`2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/broad-eval-scout.md`](2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/broad-eval-scout.md)
- [`../operator-logs/2026-06-07_prototype1-selectfix-replay-live-run.md`](../operator-logs/2026-06-07_prototype1-selectfix-replay-live-run.md)

Current status: terminal and scout coverage are present; full run-review coverage
is still missing for every branch-level treatment node. The broad/eval scout
already contains one trace-chain candidate for `branch-4fa5078a62ae685e` and
direct checkout verification for `branch-0b3c3b943489e60b`; those are the best
starting points for full reviews.

| Node | Gen | Branch | Eval | Coverage note |
| --- | ---: | --- | --- | --- |
| `node-8cf286015fb78261` | 0 | `prototype1-parent-p1-selectfix-replay-5g1x2-a2-20260607-224502-gen0` | baseline parent | Terminal/status covered only. Baseline run root still needs a full baseline eval/protocol review if campaign-wide coverage requires baseline. |
| `node-10b418a02e91f3f1` | 1 | `branch-52a318aef7a42b68` | keep | Not full-review covered. |
| `node-f21de5ba2e927ab0` | 1 | `branch-657235b177dc4597` | reject | Not full-review covered. |
| `node-c00551625661424e` | 2 | `branch-80116f6702fafe10` | reject | Not full-review covered. |
| `node-7d41954c057f3002` | 2 | `branch-a21b6ff8dee21a03` | reject | Not full-review covered; selected later as historical rejected parent. |
| `node-655e9397c9273545` | 2 | `branch-e304e47deb78395e` | reject | Not full-review covered. |
| `node-e0215ac6e2825a28` | 2 | `branch-01ade840a0a918b6` | reject | Scout-level live review exists in the older live-scout report, but not full-review covered. |
| `node-666e03c70c944722` | 3 | `branch-c17c4cda5764dbb7` | keep | Not full-review covered. |
| `node-d245f3418c712226` | 3 | `branch-0e230b45dab588be` | keep | Not full-review covered; final stop selected this historical candidate. |
| `node-44b3f97965446494` | 3 | `branch-e21d8f841040888f` | keep | Not full-review covered. |
| `node-86b020f1dfdcf8af` | 3 | `branch-564844dfd9b65469` | keep | Not full-review covered; final parent whose gen4 children completed. |
| `node-72b640e0a78cafcc` | 4 | `branch-4fa5078a62ae685e` | keep | Highest-priority full review. Scout has trace-chain candidate; protocol was segmentation-only at final scout time. |
| `node-eb38c0cf2ab7b58c` | 4 | `branch-0b3c3b943489e60b` | reject | Highest-priority full review. Scout has direct checkout verification and full call-review/partial segment-review evidence; rejected for failed-tool regression. |

Recommended review order:

1. `node-72b640e0a78cafcc` / `branch-4fa5078a62ae685e`
2. `node-eb38c0cf2ab7b58c` / `branch-0b3c3b943489e60b`
3. `node-86b020f1dfdcf8af` / `branch-564844dfd9b65469`
4. Historical rejected continuation path:
   `node-f21de5ba2e927ab0`, `node-e0215ac6e2825a28`,
   `node-7d41954c057f3002`

## `p1-handofffix-embed-5g1x2-a2-20260607-192954`

Existing report:

- [`../2026-06-07_prototype1-loop-termination-reports/handofffix-embed-budget-stop.md`](../2026-06-07_prototype1-loop-termination-reports/handofffix-embed-budget-stop.md)

Current status: clean-stop and timing coverage are present; full run-review
coverage is missing for child nodes.

| Node | Gen | Branch | Eval | Coverage note |
| --- | ---: | --- | --- | --- |
| `node-e14b23074d4b1387` | 0 | `prototype1-parent-p1-handofffix-embed-5g1x2-a2-20260607-192954-gen0` | baseline parent | Terminal/status covered only. |
| `node-84b583594e201ee2` | 1 | `branch-7048fd94aa38570b` | keep | Not full-review covered. |
| `node-4f5ce42174cc01d6` | 1 | `branch-e926c3faa92613c9` | reject | Not full-review covered. |
| `node-8167e33daa3b9bc6` | 2 | `branch-a885e5b12646f6be` | keep | Not full-review covered. |
| `node-b56d3539fe294ed3` | 2 | `branch-afdcff78a2bf6e87` | reject | Not full-review covered; used for rejected historical traversal. |
| `node-d05350cdb42e3185` | 3 | `branch-9ded1d142d60298a` | reject | Not full-review covered. |
| `node-7815b0481a271a5e` | 3 | `branch-feabca86774c57a6` | reject | Not full-review covered; final selected stop coordinate. |
| `node-c7ea8d834c87f9f6` | 3 | `branch-ed47a449ae5a187a` | reject | Not full-review covered. |
| `node-1c4fb95839e61fcc` | 3 | `branch-1fec33b974df7b88` | keep | Not full-review covered; final parent. |
| `node-1537e64bdeebe183` | 4 | `branch-2780bedd6071e108` | reject | High-priority full review as a final-generation child. |
| `node-3dec900d03d8c57a` | 4 | `branch-8a4fc1767af20136` | reject | High-priority full review as a final-generation child. |

Recommended review order:

1. Final-generation children `node-1537e64bdeebe183` and
   `node-3dec900d03d8c57a`
2. Final parent `node-1c4fb95839e61fcc`
3. Historical traversal candidates `node-b56d3539fe294ed3` and
   `node-7815b0481a271a5e`

## `p1-nokeep-handoff-5g1x2-a2-20260607-192811`

Existing report:

- [`../2026-06-07_prototype1-loop-termination-reports/handoff-failure-campaigns.md`](../2026-06-07_prototype1-loop-termination-reports/handoff-failure-campaigns.md)

Current status: terminal failure coverage is present for the payload-hash
selection blocker; child trace/usefulness reviews are missing.

| Node | Gen | Branch | Eval | Coverage note |
| --- | ---: | --- | --- | --- |
| `node-8d1174b7e1d4d187` | 0 | `prototype1-parent-p1-nokeep-handoff-5g1x2-a2-20260607-192811-gen0` | baseline parent | Terminal/status covered only. |
| `node-27cc9c0378bc28a0` | 1 | `branch-95c85fb5a1ace104` | keep | Not full-review covered. |
| `node-8a73353974d76368` | 1 | `branch-4a373b3a766ad0f0` | keep | Not full-review covered; final failed parent. |
| `node-1c226992bc62fcf8` | 2 | `branch-c8afe737b3dcf7da` | keep | Not full-review covered. |
| `node-828b2750969ae950` | 2 | `branch-4650ff6d38365585` | reject | Not full-review covered. |

Recommended review order:

1. `node-8a73353974d76368` final failed parent and its successor-selection
   channel.
2. Gen2 children `node-1c226992bc62fcf8` and `node-828b2750969ae950`, because
   they completed immediately before the hash mismatch failure.

## `p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Existing reports:

- [`../2026-06-07_prototype1-live-run-status/README.md`](../2026-06-07_prototype1-live-run-status/README.md)
- [`../2026-06-07_prototype1-loop-termination-reports/handoff-failure-campaigns.md`](../2026-06-07_prototype1-loop-termination-reports/handoff-failure-campaigns.md)

Current status: root-cause and terminal failure coverage are present for the
missing edit-harness workspace during successor artifact preparation; full child
trace/usefulness reviews are missing.

| Node | Gen | Branch | Eval | Coverage note |
| --- | ---: | --- | --- | --- |
| `node-8e50799a4c796fd5` | 0 | `prototype1-parent-p1-historyfix-handoff-5g1x2-a2-20260607-155010-gen0` | baseline parent | Terminal/status covered only. |
| `node-3509413f9d0bef24` | 1 | `branch-a7066dda07790eb7` | reject | Not full-review covered. |
| `node-903feb19806d1dd8` | 1 | `branch-eed0761ea6384596` | reject | Not full-review covered; earlier successful handoff then later missing workspace reference. |
| `node-d00c1b0d118a2ab6` | 2 | `branch-ad50f3e79db6d227` | reject | Not full-review covered. |
| `node-2df04e70d97c9304` | 2 | `branch-7c33506afa12a2f3` | reject | Not full-review covered; final failed parent. |
| `node-f76d459fe57da90c` | 3 | `branch-6bd866fcb23f3425` | reject | Not full-review covered. |
| `node-b03707937aa5968b` | 3 | `branch-e6583d10838d128e` | keep | Not full-review covered. |

Recommended review order:

1. `node-2df04e70d97c9304` final failed parent and successor artifact-prep
   stderr/channel evidence.
2. Gen3 children `node-f76d459fe57da90c` and `node-b03707937aa5968b`, because
   they completed immediately before the failed successor preparation.
3. `node-903feb19806d1dd8`, because its missing edit-harness workspace was the
   path referenced by the terminal error.

## Setup And Pre-Child Campaigns

Existing report:

- [`../2026-06-07_prototype1-loop-termination-reports/setup-and-prechild-failures.md`](../2026-06-07_prototype1-loop-termination-reports/setup-and-prechild-failures.md)

`p1-rejected-handoff-5g1x2-a2-20260607-191057` has only root
`node-55d98560307267b1`, status `planned`, and no child-plan or child-result
surfaces. Review coverage should focus on the incomplete baseline/run-root
evidence listed in the setup/pre-child report, not child nodes.

`p1-handofffix-5g1x2-a2-20260607-190702` has only root
`node-2e58ea711dd2446e`, status `planned`, and failed before broad child
generation at `embedding_model_preflight`. Review coverage should focus on the
preflight failure artifacts, not child nodes.

## Backlog Shape

For complete review coverage of the recent campaigns, create full review files
in `docs/active/agents/run-reviews/` for the high-priority nodes above. Use a
filename that includes the campaign, node id, and branch id. Each review should
cite:

- node root under `prototype1/nodes/<node>`;
- runner invocation/result/channel and stream paths;
- treatment run root and `record.json.gz`;
- `agent-turn-trace.json`, `llm-full-responses.jsonl`, validation audit, patch
  projection, and submission artifacts;
- protocol artifacts, if present;
- evaluation artifact for the branch;
- direct checkout or persisted-artifact verification for at least one important
  tool result or patch claim.

Do not add a review to the main run-review index as durable synthesis unless it
passes the quality gate.
