# Prototype 1 handoff failure campaigns

Collected: 2026-06-07T23:40:59-07:00 from `/home/brasides/code/ploke`.

Scope and boundaries:
- Read-only inspection only. No `prototype1-state`, `prototype1-step`, or `prototype1-continue` was run.
- No `.sqlite` or `.db` files were opened.
- Campaign, instance, protocol, worktree, and stream artifacts under `~/.ploke-eval` were not mutated.
- Times below are Pacific local time when shown from filesystem mtimes or stream text. Artifact JSON timestamps with `+00:00` or `Z` are quoted as written.

## Summary

| Campaign | Host process | Last reached phase | Terminal proof | Verdict |
| --- | --- | --- | --- | --- |
| `p1-nokeep-handoff-5g1x2-a2-20260607-192811` | Dead. Self-filtered `ps` had no process for this campaign; only unrelated `p1-selectfix-replay-5g1x2-a2-20260607-224502` was live. | Successor completion after generation 2 child self-eval and protocol review. | `prototype1/nodes/node-8a73353974d76368/channels/c60e0e08-b397-4784-874c-883492ffa9ac/child-to-parent.jsonl` contains `successor_completion.status="failed"` at `2026-06-07T20:51:09.680949954+00:00`; transition journal repeats the same failed completion at mtime `2026-06-07 13:51:09.679855609 -0700`. | `failed` |
| `p1-historyfix-handoff-5g1x2-a2-20260607-155010` | Dead. Self-filtered `ps` had no process for this campaign; only unrelated `p1-selectfix-replay-5g1x2-a2-20260607-224502` was live. | Successor completion after generation 3 child self-eval, successor selection, and protocol review. | `prototype1/nodes/node-2df04e70d97c9304/channels/b5555883-5928-4e07-b7b3-f1814377d36f/child-to-parent.jsonl` contains `successor_completion.status="failed"` at `2026-06-08T00:33:29.175716860+00:00`; transition journal repeats the same failed completion at mtime `2026-06-07 17:33:29.175018291 -0700`. | `failed` |

Neither campaign terminated cleanly. In both cases the exact terminal channel record is present, but it is a failed successor completion, not a clean completion. The final parent `node.json` records also remain `status="running"`, which is consistent with failed handoff cleanup rather than a normal clean stop.

## `p1-nokeep-handoff-5g1x2-a2-20260607-192811`

Roots inspected:
- Campaign: `/home/brasides/.ploke-eval/campaigns/p1-nokeep-handoff-5g1x2-a2-20260607-192811`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-nokeep-handoff-5g1x2-a2-20260607-192811`
- Instances: `/home/brasides/.ploke-eval/instances/prototype1/p1-nokeep-handoff-5g1x2-a2-20260607-192811`
- Protocol: `/home/brasides/.ploke-eval/protocol/prototype1/p1-nokeep-handoff-5g1x2-a2-20260607-192811`

Host process status:
- No live host process for this campaign was present in `ps -ef | rg '[p]loke-eval loop|[p]rototype1-state|[p]rototype1-step|[p]rototype1-continue|[p]rototype1-runner'`.
- Live Prototype 1 processes belonged to unrelated campaign `p1-selectfix-replay-5g1x2-a2-20260607-224502`.

Profile:
- Parent model: `google/gemini-3.5-flash`, `route_source=direct_google`.
- Protocol model: `google/gemini-2.5-flash`, `route_source=direct_google`, `max_tokens=8000`.
- Run bounds from `prototype1/run-profile.toml`: `max_generations=5`, `parallel_targets=2`, `max_attempts=2`, `fresh_slots_per_child=2`, `stop_on_first_keep=false`, `require_keep_for_continuation=false`, `explore_from_rejected=true`.

Transition journal and timing:
- Journal: `prototype1/transition-journal.jsonl`, 65 lines, mtime `2026-06-07 13:51:09.679855609 -0700`.
- Earlier successor records selected `node-8a73353974d76368` around `2026-06-07T20:25:19Z` with `disposition=continue_ready`.
- Last child observations:
  - `node-1c226992bc62fcf8` generation 2 result written at `2026-06-07T20:46:36Z`; treatment `p1-nokeep-handoff-5g1x2-a2-20260607-192811-treatment-branch-c8afe737b3dcf7da-1780864443224`.
  - `node-828b2750969ae950` generation 2 result written at `2026-06-07T20:51:09Z`; treatment `p1-nokeep-handoff-5g1x2-a2-20260607-192811-treatment-branch-4650ff6d38365585-1780864443229`.
- Terminal journal entry:
  - `kind=successor`, `node_id=node-8a73353974d76368`, `runtime_id=c60e0e08-b397-4784-874c-883492ffa9ac`, `status=failed`, recorded `2026-06-07T20:51:09Z`.
  - Detail: `prototype1-state successor failed: batch selection is invalid: failed to load History traversal candidates: sealed block failed verification before storage: invalid selection decision entry: selection entry 54c7dfe7-546f-4fe7-987d-2b048c4070b3 payload hash does not match decision payload`.

Broad request/result mtimes:
- Generation 1 parent `node-8d1174b7e1d4d187`:
  - Request JSONs: `node-8d1174b7e1d4d187.json`, `-r2.json`, `-r3.json`, `-r4.json` at `2026-06-07 12:58:57 -0700`.
  - Result sidecars: `node-8d1174b7e1d4d187.*` at `2026-06-07 13:04:17 -0700`; `node-8d1174b7e1d4d187-r2.*` at `2026-06-07 13:06:14 -0700`.
  - Child plan: `prototype1/messages/child-plan/node-8d1174b7e1d4d187.json` at `2026-06-07 13:06:18 -0700`.
- Generation 2 parent `node-8a73353974d76368`:
  - Request JSONs: `node-8a73353974d76368.json`, `-r2.json`, `-r3.json`, `-r4.json` at `2026-06-07 13:25:25 -0700`.
  - Result sidecars: `node-8a73353974d76368-r2.*` at `2026-06-07 13:30:24 -0700`; `node-8a73353974d76368.*` at `2026-06-07 13:31:33 -0700`.
  - Child plan: `prototype1/messages/child-plan/node-8a73353974d76368.json` at `2026-06-07 13:31:36 -0700`.

Protocol timing:
- Last protocol artifact under this campaign had mtime `2026-06-07 13:51:08.786848130 -0700`.
- It was `.../treatments/branch-4650ff6d38365585/.../run-1780864444125-structured-current-policy-bde06701/1780865468784_tool_call_segment_review_BurntSushi__ripgrep-2209.json`.
- This lands immediately before the terminal failed successor completion at `13:51:09 -0700`.

Node invocation/result/successor records:
- Final parent node: `prototype1/nodes/node-8a73353974d76368/node.json`, generation 1, branch `branch-4a373b3a766ad0f0`, `status="running"`, `updated_at="2026-06-07T20:25:25.606804640+00:00"`.
- Final successor invocation: `prototype1/nodes/node-8a73353974d76368/invocations/c60e0e08-b397-4784-874c-883492ffa9ac.json`, `role="successor"`, `created_at="2026-06-07T20:25:23.029947019+00:00"`.
- Final successor channel:
  - `successor_ready` at `2026-06-07T20:25:25.558164147+00:00`.
  - `successor_completion.status="failed"` at `2026-06-07T20:51:09.680949954+00:00`.
- Last child result records:
  - `node-1c226992bc62fcf8/results/986bbb9e-d584-4456-9372-4524d5d78d00.json`: `status="succeeded"`, `exit_code=0`, generation 2, branch `branch-c8afe737b3dcf7da`, recorded `2026-06-07T20:46:36.481063465+00:00`.
  - `node-828b2750969ae950/results/abcfebae-9486-4fa3-b150-78366109a7a3.json`: `status="succeeded"`, `exit_code=0`, generation 2, branch `branch-4650ff6d38365585`, recorded `2026-06-07T20:51:09.564093195+00:00`.
- Earlier child result for selected generation 1 node:
  - `node-8a73353974d76368/results/b78df17c-a082-4bf5-b071-802834d2922b.json`: `status="succeeded"`, `exit_code=0`, generation 1, branch `branch-4a373b3a766ad0f0`, recorded `2026-06-07T20:25:19.506228211+00:00`.

Stream terminal evidence:
- Final stderr: `prototype1/nodes/node-8a73353974d76368/streams/c60e0e08-b397-4784-874c-883492ffa9ac/stderr.log`.
- Terminal stderr includes two `WorkspacePathMismatch` records:
  - Expected `.../prototype1/nodes/node-828b2750969ae950/worktree`, observed `.../prototype1/workspaces/edit-harness/node-8a73353974d76368`.
  - Expected `.../prototype1/nodes/node-1c226992bc62fcf8/worktree`, observed `.../prototype1/workspaces/edit-harness/node-8a73353974d76368-r2`.
- Same stderr then ends with the decisive terminal error: `batch selection is invalid: failed to load History traversal candidates: sealed block failed verification before storage: invalid selection decision entry: selection entry 54c7dfe7-546f-4fe7-987d-2b048c4070b3 payload hash does not match decision payload`.
- Final stdout: `prototype1/nodes/node-8a73353974d76368/streams/c60e0e08-b397-4784-874c-883492ffa9ac/stdout.log`.
  - It contains broad headless TUI elapsed timers and broad result commits at `2026-06-07T13:30:24.039-07:00 elapsed_ms=300985` and `2026-06-07T13:31:33.547-07:00 elapsed_ms=370493`.
  - Its tail has no clean completion marker; the terminal completion status is in the channel and transition journal.

Filesystem path checks for mismatch context:
- Missing: `prototype1/nodes/node-828b2750969ae950/worktree`.
- Missing: `prototype1/nodes/node-1c226992bc62fcf8/worktree`.
- Present: `prototype1/workspaces/edit-harness/node-8a73353974d76368`, mtime `2026-06-07 13:31:33.766329148 -0700`.
- Present: `prototype1/workspaces/edit-harness/node-8a73353974d76368-r2`, mtime `2026-06-07 13:46:36.690577499 -0700`.

Verdict:
- `failed`.
- Exact terminal proof: `prototype1/nodes/node-8a73353974d76368/channels/c60e0e08-b397-4784-874c-883492ffa9ac/child-to-parent.jsonl` contains the failed `successor_completion` record, and `prototype1/transition-journal.jsonl` records the same `status=failed` as its last entry.
- No clean ending exists in the inspected persisted surfaces: the host process is gone, the final successor completion is failed, and the final parent node remains `status="running"` rather than a clean terminal state.

## `p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Roots inspected:
- Campaign: `/home/brasides/.ploke-eval/campaigns/p1-historyfix-handoff-5g1x2-a2-20260607-155010`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-historyfix-handoff-5g1x2-a2-20260607-155010`
- Instances: `/home/brasides/.ploke-eval/instances/prototype1/p1-historyfix-handoff-5g1x2-a2-20260607-155010`
- Protocol: `/home/brasides/.ploke-eval/protocol/prototype1/p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Host process status:
- No live host process for this campaign was present in `ps -ef | rg '[p]loke-eval loop|[p]rototype1-state|[p]rototype1-step|[p]rototype1-continue|[p]rototype1-runner'`.
- Live Prototype 1 processes belonged to unrelated campaign `p1-selectfix-replay-5g1x2-a2-20260607-224502`.

Profile:
- Parent model: `google/gemini-3.5-flash`, `route_source=direct_google`.
- Protocol model: `google/gemini-2.5-flash`, `route_source=direct_google`, `max_tokens=8000`.
- Run bounds from `prototype1/run-profile.toml`: `max_generations=5`, `parallel_targets=2`, `max_attempts=2`, `fresh_slots_per_child=2`, `stop_on_first_keep=false`, `require_keep_for_continuation=false`, `explore_from_rejected=true`.

Transition journal and timing:
- Journal: `prototype1/transition-journal.jsonl`, 103 lines, mtime `2026-06-07 17:33:29.175018291 -0700`.
- Earlier successor on `node-903feb19806d1dd8` completed with `status=succeeded` at `2026-06-08T00:05:21Z`, then control moved to `node-2df04e70d97c9304`.
- Final generation 3 child observations:
  - `node-f76d459fe57da90c` result written at `2026-06-08T00:29:45Z`; treatment `p1-historyfix-handoff-5g1x2-a2-20260607-155010-treatment-branch-6bd866fcb23f3425-1780877813497`.
  - `node-b03707937aa5968b` result written at `2026-06-08T00:33:27Z`; treatment `p1-historyfix-handoff-5g1x2-a2-20260607-155010-treatment-branch-e6583d10838d128e-1780877812088`.
- Final selection entry:
  - `kind=successor`, `node_id=node-2df04e70d97c9304`, recorded `2026-06-08T00:33:27Z`.
  - Selected `branch-7c33506afa12a2f3` with `disposition=continue_explore_from_rejected`; selection rationale says `score_child_prop` and `outcome="stop"` inside the selection decision, but the runtime then attempted successor artifact preparation.
- Terminal journal entry:
  - `kind=successor`, `node_id=node-2df04e70d97c9304`, `runtime_id=b5555883-5928-4e07-b7b3-f1814377d36f`, `status=failed`, recorded `2026-06-08T00:33:29Z`.
  - Detail: `prototype1-state successor failed: database setup failed during 'prototype1_successor_artifact_prepare': worktree metadata exists for '/home/brasides/.ploke-eval/campaigns/p1-historyfix-handoff-5g1x2-a2-20260607-155010/prototype1/workspaces/edit-harness/node-903feb19806d1dd8' but the path is missing on disk`.

Broad request/result mtimes:
- Generation 1 parent `node-8e50799a4c796fd5`:
  - Request JSONs: `node-8e50799a4c796fd5.json`, `-r2.json`, `-r3.json`, `-r4.json` at `2026-06-07 16:14:19 -0700`.
  - Result sidecars: `node-8e50799a4c796fd5-r2.*` at `2026-06-07 16:20:15 -0700`; `node-8e50799a4c796fd5.*` at `2026-06-07 16:24:15 -0700`.
  - Child plan: `prototype1/messages/child-plan/node-8e50799a4c796fd5.json` at `2026-06-07 16:24:18 -0700`.
- Generation 2 parent `node-903feb19806d1dd8`:
  - Request JSONs: `node-903feb19806d1dd8.json`, `-r2.json`, `-r3.json`, `-r4.json` at `2026-06-07 16:35:57 -0700`.
  - Result sidecars: `node-903feb19806d1dd8-r2.*` at `2026-06-07 16:41:58 -0700`; `node-903feb19806d1dd8.*` at `2026-06-07 16:45:05 -0700`.
  - Child plan: `prototype1/messages/child-plan/node-903feb19806d1dd8.json` at `2026-06-07 16:45:08 -0700`.
- Generation 3 parent `node-2df04e70d97c9304`:
  - Request JSONs: `node-2df04e70d97c9304.json`, `-r2.json`, `-r3.json`, `-r4.json` at `2026-06-07 17:05:21 -0700`.
  - Result sidecars: `node-2df04e70d97c9304.*` at `2026-06-07 17:11:52 -0700`; `node-2df04e70d97c9304-r2.*` at `2026-06-07 17:14:25 -0700`.
  - Child plan: `prototype1/messages/child-plan/node-2df04e70d97c9304.json` at `2026-06-07 17:14:28 -0700`.

Protocol timing:
- Last protocol artifact under this campaign had mtime `2026-06-07 17:33:26.227999975 -0700`.
- It was `.../treatments/branch-e6583d10838d128e/.../run-1780877813211-structured-current-policy-3688dbbb/1780878806225_tool_call_segment_review_BurntSushi__ripgrep-2209.json`.
- This lands immediately before successor selection at `17:33:27 -0700` and failed successor completion at `17:33:29 -0700`.

Node invocation/result/successor records:
- Final parent node: `prototype1/nodes/node-2df04e70d97c9304/node.json`, generation 2, branch `branch-7c33506afa12a2f3`, `status="running"`, `updated_at="2026-06-08T00:05:21.760556067+00:00"`.
- Final successor invocation: `prototype1/nodes/node-2df04e70d97c9304/invocations/b5555883-5928-4e07-b7b3-f1814377d36f.json`, `role="successor"`, `created_at="2026-06-08T00:05:19.081650646+00:00"`.
- Final successor channel:
  - `successor_ready` at `2026-06-08T00:05:21.710942020+00:00`.
  - `successor_completion.status="failed"` at `2026-06-08T00:33:29.175716860+00:00`.
- Last child result records:
  - `node-f76d459fe57da90c/results/fd38797e-206b-42e2-92db-2f7f5e43fb7d.json`: `status="succeeded"`, `exit_code=0`, generation 3, branch `branch-6bd866fcb23f3425`, recorded `2026-06-08T00:29:45.625568385+00:00`.
  - `node-b03707937aa5968b/results/9b9e97f3-ce90-4886-a298-19de572222b4.json`: `status="succeeded"`, `exit_code=0`, generation 3, branch `branch-e6583d10838d128e`, recorded `2026-06-08T00:33:27.016113154+00:00`.
- Earlier successful handoff record:
  - `node-903feb19806d1dd8/invocations/05b19fa7-0bee-4d8d-bf91-d1bb6e076f5e.json`: `role="successor"`, `created_at="2026-06-07T23:35:54.176294963+00:00"`.
  - Transition journal later records `node-903feb19806d1dd8` successor completion `status=succeeded` at `2026-06-08T00:05:21Z`.

Stream terminal evidence:
- Final stderr: `prototype1/nodes/node-2df04e70d97c9304/streams/b5555883-5928-4e07-b7b3-f1814377d36f/stderr.log`.
- Terminal stderr includes three `WorkspacePathMismatch` records:
  - Expected `.../prototype1/nodes/node-b03707937aa5968b/worktree`, observed `.../prototype1/workspaces/edit-harness/node-2df04e70d97c9304-r2`.
  - Expected `.../prototype1/nodes/node-f76d459fe57da90c/worktree`, observed `.../prototype1/workspaces/edit-harness/node-2df04e70d97c9304`.
  - Expected `.../prototype1/nodes/node-2df04e70d97c9304/worktree`, observed `.../prototype1/workspaces/edit-harness/node-903feb19806d1dd8`.
- Same stderr then ends with the decisive terminal error: `database setup failed during 'prototype1_successor_artifact_prepare': worktree metadata exists for '/home/brasides/.ploke-eval/campaigns/p1-historyfix-handoff-5g1x2-a2-20260607-155010/prototype1/workspaces/edit-harness/node-903feb19806d1dd8' but the path is missing on disk`.
- Final stdout: `prototype1/nodes/node-2df04e70d97c9304/streams/b5555883-5928-4e07-b7b3-f1814377d36f/stdout.log`.
  - It contains broad headless TUI elapsed timers and broad result commits at `2026-06-07T17:11:52.389-07:00 elapsed_ms=393297` and `2026-06-07T17:14:25.836-07:00 elapsed_ms=546744`.
  - Its tail has no clean completion marker; the terminal completion status is in the channel and transition journal.

Filesystem path checks for mismatch context:
- Missing: `prototype1/nodes/node-b03707937aa5968b/worktree`.
- Missing: `prototype1/nodes/node-f76d459fe57da90c/worktree`.
- Missing: `prototype1/nodes/node-2df04e70d97c9304/worktree`.
- Present: `prototype1/workspaces/edit-harness/node-2df04e70d97c9304-r2`, mtime `2026-06-07 17:14:25.991531189 -0700`.
- Present: `prototype1/workspaces/edit-harness/node-2df04e70d97c9304`, mtime `2026-06-07 17:11:52.530649745 -0700`.
- Missing: `prototype1/workspaces/edit-harness/node-903feb19806d1dd8`.

Verdict:
- `failed`.
- Exact terminal proof: `prototype1/nodes/node-2df04e70d97c9304/channels/b5555883-5928-4e07-b7b3-f1814377d36f/child-to-parent.jsonl` contains the failed `successor_completion` record, and `prototype1/transition-journal.jsonl` records the same `status=failed` as its last entry.
- No clean ending exists in the inspected persisted surfaces: the host process is gone, the final successor completion is failed, the final parent node remains `status="running"`, and the stderr artifact proves successor artifact preparation failed on a missing edit-harness workspace path.
