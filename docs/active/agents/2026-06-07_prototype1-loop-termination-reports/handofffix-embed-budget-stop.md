# Prototype 1 termination report: handofffix embed budget stop

Campaign: `p1-handofffix-embed-5g1x2-a2-20260607-192954`

Observed from `/home/brasides/code/ploke` at `2026-06-07T23:41:32-07:00`.

## Verdict

Final verdict: `clean`.

The run is not live, and the target campaign ended by a recorded successor budget stop, not by a crash, missing child result, or active host process. The terminal decision was:

```text
disposition=stop_historical_traversal_budget
selected_next_branch_id=branch-feabca86774c57a6
selected_branch_disposition=reject
selected candidate node=node-7815b0481a271a5e
selection outcome=stop
```

The exact artifact proving the parent runtime ended completely is:

```text
/home/brasides/.ploke-eval/campaigns/p1-handofffix-embed-5g1x2-a2-20260607-192954/prototype1/nodes/node-1c4fb95839e61fcc/channels/7efc6b1e-e943-403c-b52f-7d428b2b2971/child-to-parent.jsonl
```

Its final record is a `prototype1-successor-completion.v1` with `status="succeeded"`, `node_id="node-1c4fb95839e61fcc"`, `runtime_id="7efc6b1e-e943-403c-b52f-7d428b2b2971"`, and `recorded_at="2026-06-08T04:58:24.167761214+00:00"`. The transition journal also points to this same file in its final entry.

## Host Process Status

Self-filtered `ps` found live Prototype 1 processes only for another campaign:

```text
p1-selectfix-replay-5g1x2-a2-20260607-224502
  pid=1395245 prototype1-state
  pid=1478346 prototype1-runner
  pid=1478403 prototype1-runner
```

No live `ploke-eval loop`, `prototype1-state`, `prototype1-step`, `prototype1-continue`, or `prototype1-runner` process was found for `p1-handofffix-embed-5g1x2-a2-20260607-192954`.

Authority roots inspected:

```text
campaign:  /home/brasides/.ploke-eval/campaigns/p1-handofffix-embed-5g1x2-a2-20260607-192954
worktree:  /home/brasides/.ploke-eval/worktrees/p1-handofffix-embed-5g1x2-a2-20260607-192954
instances: /home/brasides/.ploke-eval/instances/prototype1/p1-handofffix-embed-5g1x2-a2-20260607-192954
protocol:  /home/brasides/.ploke-eval/protocol/prototype1/p1-handofffix-embed-5g1x2-a2-20260607-192954
```

No sqlite/db files were opened.

## Transition Journal Timing

Timing source: durable epoch-ms `recorded_at` in `prototype1/transition-journal.jsonl`, rendered below as UTC.

| Phase | Journal evidence |
| --- | --- |
| Gen 0 parent start | `2026-06-08T02:36:07Z` `parent_started`, node `node-e14b23074d4b1387`; `resource parent_start` follows on the same second. |
| Gen 1 children from gen 0 | `materialize_branch` at `02:55:54Z`; builds completed at `02:58:16Z`; children observed before at `02:58:22Z`; `node-84b583594e201ee2` result at `03:04:18Z`; `node-4f5ce42174cc01d6` result at `03:06:29Z`. |
| Gen 1 successor | `03:06:29Z` selected `node-84b583594e201ee2` / `branch-7048fd94aa38570b` with `continue_ready`; successor runtime ready at `03:06:47Z`; gen 0 parent completed at `03:06:47Z`. |
| Gen 2 children from `node-84...` | materialize at `03:14:43Z`; builds completed at `03:16:58Z`/`03:16:59Z`; children observed before at `03:17:04Z`; `node-8167e33daa3b9bc6` result at `03:25:45Z`; `node-b56d3539fe294ed3` result at `03:29:48Z`. |
| Gen 2 successor | `03:29:49Z` selected `node-8167e33daa3b9bc6` / `branch-a885e5b12646f6be` with `continue_ready`; successor runtime ready at `03:30:10Z`; `node-84...` parent completed at `03:30:10Z`. |
| Gen 3 children from `node-816...` | materialize at `03:41:14Z`; builds completed at `03:43:35Z`; children observed before at `03:43:40Z`; `node-d05350cdb42e3185` result at `03:50:59Z`; `node-7815b0481a271a5e` result at `03:55:39Z`. |
| Historical traversal to rejected branch | `03:55:40Z` selected `node-b56d3539fe294ed3` / `branch-afdcff78a2bf6e87` with `continue_explore_from_rejected`; successor runtime ready at `03:56:04Z`; prior `node-816...` parent runtime completed at `03:56:04Z`. |
| Gen 3 children from `node-b56...` | materialize at `04:03:13Z`; builds completed at `04:05:35Z`/`04:05:36Z`; children observed before at `04:05:42Z`; `node-1c4fb95839e61fcc` result at `04:16:01Z`; `node-c7ea8d834c87f9f6` result at `04:17:58Z`. |
| Historical traversal back to `node-816...` | `04:17:59Z` selected `node-8167e33daa3b9bc6` / `branch-a885e5b12646f6be` with `continue_historical_traversal`; successor runtime ready at `04:18:21Z`; `node-b56...` parent runtime completed at `04:18:21Z`. |
| Historical gen 3 rerun from `node-816...` | materialize at `04:18:22Z`; builds completed at `04:20:37Z`; children observed before at `04:20:42Z`; `node-7815b0481a271a5e` result at `04:20:43Z`/after at `04:20:44Z`; `node-d05350cdb42e3185` result at `04:29:38Z`. |
| Continue to final parent | `04:29:39Z` selected `node-1c4fb95839e61fcc` / `branch-1fec33b974df7b88` with `continue_historical_traversal`; successor runtime ready at `04:30:01Z`; prior `node-816...` parent runtime completed at `04:30:01Z`. |
| Gen 4 final children from `node-1c...` | materialize at `04:43:37Z`; builds completed at `04:45:51Z`/`04:45:52Z`; children observed before at `04:45:57Z`; `node-1537e64bdeebe183` result at `04:58:18Z`; `node-3dec900d03d8c57a` result at `04:58:23Z`. |
| Final stop and completion | `04:58:24Z` selected then stopped on `stop_historical_traversal_budget`; `resource parent_complete` for `node-1c4fb95839e61fcc` at `04:58:24Z`; final `successor completed status=succeeded` at `04:58:24Z`. |

The journal tail is internally complete: both final gen-4 children have result-written and observe-after records before the successor stop and parent completion records.

## Broad Request And Result Mtimes

Timing source: filesystem mtimes from `prototype1/messages/edit-harness-request`, `prototype1/messages/edit-harness-result`, and `prototype1/messages/child-plan`. Times are local `-0700`.

| Parent node | Request JSON mtimes | Result JSON mtimes | Child-plan mtime |
| --- | --- | --- | --- |
| `node-e14b23074d4b1387` | `2026-06-07 19:49:33` for `node-e14...{,-r2,-r3,-r4}.json` | `19:55:46` `node-e14...-r2.json`; `19:55:50` `node-e14....json` | `19:55:53` |
| `node-84b583594e201ee2` | `2026-06-07 20:06:47` for `node-84...{,-r2,-r3,-r4}.json` | `20:14:22` `node-84...-r2.json`; `20:14:39` `node-84....json` | `20:14:42` |
| `node-8167e33daa3b9bc6` | `2026-06-07 20:30:10` for `node-816...{,-r2,-r3,-r4}.json` | `20:38:35` `node-816...-r2.json`; `20:41:09` `node-816....json` | `20:41:13` |
| `node-b56d3539fe294ed3` | `2026-06-07 20:56:04` for `node-b56...{,-r2,-r3,-r4}.json` | `21:01:14` `node-b56...-r2.json`; `21:03:09` `node-b56....json` | `21:03:12` |
| `node-1c4fb95839e61fcc` | `2026-06-07 21:30:01` for `node-1c...{,-r2,-r3,-r4}.json` | `21:41:17` `node-1c...-r2.json`; `21:43:32` `node-1c....json` | `21:43:36` |

For each parent above, the matching `*.headless-tui.json`, `agent-turn-trace.json`, `agent-turn-summary.json`, and `llm-full-responses.jsonl` sidecars landed at the same second as the corresponding result JSON.

## Node Invocation, Result, And Successor Records

Node record sources:

```text
prototype1/nodes/<node>/node.json
prototype1/nodes/<node>/runner-request.json
prototype1/nodes/<node>/runner-result.json
prototype1/nodes/<node>/channels/<runtime>/child-to-parent.jsonl
```

| Node | Gen | Branch | Node file status | Runtime/result evidence |
| --- | ---: | --- | --- | --- |
| `node-e14b23074d4b1387` | 0 | `prototype1-parent-p1-handofffix-embed-5g1x2-a2-20260607-192954-gen0` | `running` | Root parent; transition journal records parent start and parent complete. |
| `node-84b583594e201ee2` | 1 | `branch-7048fd94aa38570b` | `running` | `runner-result.json` says `succeeded`; successor-completion channel `1bdf24b7-34ed-46d6-9a15-03113415f5a4` says `succeeded` at `2026-06-08T03:30:10.048325728+00:00`. |
| `node-4f5ce42174cc01d6` | 1 | `branch-e926c3faa92613c9` | `succeeded` | `runner-result.json` says `succeeded`. |
| `node-8167e33daa3b9bc6` | 2 | `branch-a885e5b12646f6be` | `running` | `runner-result.json` says `succeeded`; successor-completion channels `637883b5-2843-45ad-9f63-ba2ec73c2cbf` and `6d5ffaef-ad28-4e17-9432-4754fc12e70c` both say `succeeded`. |
| `node-b56d3539fe294ed3` | 2 | `branch-afdcff78a2bf6e87` | `running` | `runner-result.json` says `succeeded`; successor-completion channel `f5ff0efd-bb58-4255-87f2-6a6b2e7e3000` says `succeeded`. |
| `node-d05350cdb42e3185` | 3 | `branch-9ded1d142d60298a` | `succeeded` | Two treatment result channel records exist, both `succeeded`, for runtimes `0757180c-1571-4244-b69a-f9e61075f372` and `ada14d55-2b2a-4630-a03b-f7e80b50a0d4`. |
| `node-7815b0481a271a5e` | 3 | `branch-feabca86774c57a6` | `succeeded` | Two treatment result channel records exist, both `succeeded`, for runtimes `2226abe2-c3fa-4854-98be-ce0b8f5fb627` and `92d4af32-4ad9-43c7-81e7-72dfcf68974b`; this node was the final selected stop coordinate. |
| `node-c7ea8d834c87f9f6` | 3 | `branch-ed47a449ae5a187a` | `succeeded` | `runner-result.json` says `succeeded`. |
| `node-1c4fb95839e61fcc` | 3 | `branch-1fec33b974df7b88` | `running` | `runner-result.json` says `succeeded`; final successor-completion channel `7efc6b1e-e943-403c-b52f-7d428b2b2971` says `succeeded` at `2026-06-08T04:58:24.167761214+00:00`. |
| `node-1537e64bdeebe183` | 4 | `branch-2780bedd6071e108` | `succeeded` | `runner-result.json` says `succeeded`; result channel has `exit_code=0`, `recorded_at=2026-06-08T04:58:18.067132814+00:00`. |
| `node-3dec900d03d8c57a` | 4 | `branch-8a4fc1767af20136` | `succeeded` | `runner-result.json` says `succeeded`; result channel has `exit_code=0`, `recorded_at=2026-06-08T04:58:23.139934569+00:00`. |

Important caveat: several parent `node.json` files still say `running`. I did not treat that as live-process evidence because the host process list has no target process and the runtime channels record successor completions with `status="succeeded"`.

## Stream Terminal Evidence

Final parent stream:

```text
/home/brasides/.ploke-eval/campaigns/p1-handofffix-embed-5g1x2-a2-20260607-192954/prototype1/nodes/node-1c4fb95839e61fcc/streams/7efc6b1e-e943-403c-b52f-7d428b2b2971/stdout.log
```

Relevant terminal lines:

```text
"outcome": "completed:Reject;children_ran=2;children_planned=2;selection=Stop;successor=node-7815b0481a271a5e;successor_handoff=skipped:StopHistoricalTraversalBudget"
"node_status": "succeeded"
"successor_runtime": null
"successor_pid": null
"successor_ready_path": null
```

The final parent stderr contains `WorkspacePathMismatch` entries for the gen-4 child workspaces:

```text
expected: .../nodes/node-3dec900d03d8c57a/worktree
observed: .../prototype1/workspaces/edit-harness/node-1c4fb95839e61fcc

expected: .../nodes/node-1537e64bdeebe183/worktree
observed: .../prototype1/workspaces/edit-harness/node-1c4fb95839e61fcc-r2
```

Those entries are artifact/workspace-path evidence in stderr, but they are not the terminal verdict. The parent stdout and channel completion both report a completed stop.

Final gen-4 child stderr terminals:

```text
node-1537e64bdeebe183 runtime 958dd9c7-5985-425c-a6b0-a9bcd8a4e575:
04:45:57 loop.prototype1_branch.evaluate.branch-2780bedd6071e108.start
04:58:18 loop.prototype1_branch.evaluate.branch-2780bedd6071e108.end +740.730s

node-3dec900d03d8c57a runtime 06c879dd-cd3d-4589-9f0f-9c47301e3afb:
04:45:57 loop.prototype1_branch.evaluate.branch-8a4fc1767af20136.start
04:58:23 loop.prototype1_branch.evaluate.branch-8a4fc1767af20136.end +745.803s
```

Final child stdout logs include non-terminal warning lines, including `TOOL_EXECUTION_FAILED` warnings near `21:50:32-21:50:48 -0700`. The child result channels still report `status="succeeded"` and `exit_code=0`, so these warnings are not evidence that the campaign process failed to terminate cleanly.

## Protocol And Eval Timing

Protocol artifact root:

```text
/home/brasides/.ploke-eval/protocol/prototype1/p1-handofffix-embed-5g1x2-a2-20260607-192954
```

There are `640` protocol JSON artifacts. Filename and `created_at_ms` timing align with the child self-eval windows. The final two gen-4 children have protocol windows:

| Branch | Run id | Protocol JSON count | Protocol first/last, local `-0700` | Child eval terminal |
| --- | --- | ---: | --- | --- |
| `branch-2780bedd6071e108` | `run-1780893958021-structured-current-policy-7d115b2e` | 64 | `2026-06-07 21:51:05` to `21:58:17` | stderr ended `21:58:18 -0700` / `+740.730s` |
| `branch-8a4fc1767af20136` | `run-1780893958049-structured-current-policy-fd66ed58` | 66 | `2026-06-07 21:51:21` to `21:58:22` | stderr ended `21:58:23 -0700` / `+745.803s` |

The corresponding instance artifacts are:

```text
.../treatments/branch-2780bedd6071e108/.../run-1780893958021-structured-current-policy-7d115b2e/agent-turn-trace.json
.../treatments/branch-2780bedd6071e108/.../run-1780893958021-structured-current-policy-7d115b2e/llm-full-responses.jsonl

.../treatments/branch-8a4fc1767af20136/.../run-1780893958049-structured-current-policy-fd66ed58/agent-turn-trace.json
.../treatments/branch-8a4fc1767af20136/.../run-1780893958049-structured-current-policy-fd66ed58/llm-full-responses.jsonl
```

Their mtimes are `2026-06-07 21:50:33 -0700` and `2026-06-07 21:50:49 -0700`, respectively, before the protocol adjudication window and final child result records.

## Evidence Limits

- I did not mutate `~/.ploke-eval` campaign, worktree, instance, or protocol artifacts.
- I did not open sqlite/db files.
- `node.json` status is not a sufficient liveness source for this campaign because parent nodes remain marked `running` after successor-completion channel records say `succeeded`.
- The clean verdict is a mechanical termination verdict. It does not mean the selected branch was semantically accepted; the final selected branch disposition was `reject`, and that rejection is what drove the budget-stop outcome.
