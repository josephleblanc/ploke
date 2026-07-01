# 2026-06-27 p1-runnerio DB-only trajectory Q&A

**Run inspected:** `p1-runnerio-db-20260627-125536`  
**Worktree selected for `walk db_query`:** `/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536`  
**Constraint for this inspection:** answers below use only `ploke-eval loop walk db_query` results against the owner eval DB. I did not directly read run artifact files. File paths below are DB values, not file reads.

## Answerability matrix

Legend: **Yes** = directly answerable from that source; **Partial** = answerable only by inference, only at a coarser granularity, or missing one important field; **No** = not answerable from that source without consulting the other source or code.

| Question / fact | DB-only | File-only | Notes |
|---|---:|---:|---|
| Campaign, parent node, generation, branch, worktree, configured binary path | Yes | Yes | Both sources agree on `p1-runnerio-db-20260627-125536`, `node-b86fe92de458ef31`, generation `0`, and parent branch. |
| Exact git commit / artifact coordinate | Partial | Yes | DB has paths but no binary/git digest row; files/git state show HEAD `42ba40f...` and request target artifact. |
| Exact authority-bearing binary digest | No | Partial | File projection has binary path, but the node-local binary was absent; no digest was available. |
| Reconstructed phase before each operator step | Partial | Partial | Both can infer milestones, but neither provides a full R-phase ledger. |
| Which R transitions completed | Partial | Partial | `parent_started`, baseline completion, and child-plan attempt are visible; full transition chain is not. |
| Failed transition and whether durable evidence was written first | Yes | Yes | Both show broad child planning timed out and persisted a rejected zero-child plan. |
| Operator guard vs real live-edge failure | Partial | Partial | Both show live broad-harness artifacts, but neither records exact CLI flags. |
| Parent identity source: active checkout vs successor invocation | Partial | Yes | DB infers from generation/no invocation; files include active checkout parent identity and no invocation files. |
| Parent identity / scheduler / branch / generation agreement | Yes | Partial | DB normalized rows agree; files also agree on identity but expose scheduler-vs-node projection drift. |
| Genesis vs predecessor startup | Partial | Yes | DB infers generation-0/no invocation; files show generation-0 identity and no History/invocation files. |
| Startup validation path: History predecessor vs genesis absence | No | Partial | Files show no History/predecessor artifacts, so genesis path is likely; neither has explicit startup-admission proof. |
| Parent-start evidence recorded once | Yes | Yes | DB has one transition event; journal has one `parent_started` entry plus one resource entry. |
| Admitted run profile identity | Yes | Yes | Both expose profile name/path/source/hash. |
| Search policy: max generations / max nodes / child min-max / schedule | No | Yes | Files answer from `run-profile.toml`, `scheduler.json`, and request JSON; DB lacks normalized search-policy rows. |
| Broad-TUI retries / timeout | Partial | Yes | DB only sees timeout in rejection reason; files show profile `max_attempts=1`, timeout `300s`. |
| Why `child_budget.min = 1` | No | Yes | Files show profile/scheduler/request all set min `1`; DB lacks the policy relation. |
| Generation source: deterministic vs broad-harness | Yes | Yes | DB shows broad producer/request id; files show `[generation] source = "broad-harness-request"`. |
| Deterministic no-op planner disabled globally | No | No | This is a code/configuration claim outside this run’s persisted facts. |
| Whether child-plan message existed before the step | No | Partial | File mtimes imply it was written after request, but no before-snapshot exists. |
| Child-plan binding to parent and expected child generation | Yes | Yes | Both show parent `node-b86...`, child generation `1`. |
| Published request id / slot | Yes | Yes | Both show `broad-harness-request:node-b86fe92de458ef31`. |
| Prompt/model/provider/route used | Partial | Yes | DB has campaign and agent-turn model rows; files expose prompt text, pre-child planner model, and TUI selected model. |
| Headless TUI started | Partial | Partial | Both show diagnostics/turn artifacts, but zero events/attempts. |
| Provider call happened | Partial | Yes | DB has zero model exchanges; files show `llm-full-responses.jsonl` is 0 bytes and no final message. |
| Tools executed | Yes | Yes | Both show zero persisted tool/events for the TUI attempt. |
| Timeout vs crash vs submitted result | Yes | Yes | Both show timeout after 300 seconds and no submitted result/admitted child. |
| Candidate workspace path | No | Yes | Files expose `candidate_workspace_path`; DB has no harness-request relation yet. |
| Changed paths / diff | No | Yes | Files/git state show uncommitted diff in `crates/ploke-tree-egui/src/main.rs`; DB has no admitted change rows. |
| Whether rejection was empty/protected/malformed/unbound | Partial | Yes | Both show timeout as the persisted reason; files also show no validations/protected denial. |
| Rejected child-plan persisted before below-minimum error | Yes | Yes | Both show child count `0` and one rejected attempt. |
| Rejected attempt reason and producer id | Yes | Yes | Both preserve producer id, proposal id, policy, target, and timeout reason. |
| Number of candidate attempts | Yes | Partial | DB/file child-plan show one rejected attempt; headless diagnostics show zero internal attempts retained. |
| Attempt target/workspace/candidate/branch/status | Partial | Partial | DB has target/reason but not workspace/branch; files add workspace/branch but no admitted candidate id. |
| Why each attempt was rejected | Yes | Yes | Timeout after 300 seconds. |
| Protected-write denial | Partial | Yes | DB has no tool/trace denial; files show empty validations/events. |
| “No admitted changes” semantics for this run | Partial | Yes | This run was timeout, not no-admitted-changes; files reveal an uncommitted diff with no submitted/admitted result. |
| Last tool/model event before timeout | Yes | Yes | Both show no persisted tool/model event before timeout. |
| Child-plan DB rows agree with message | Partial | N/A | DB proves internal row consistency; file-only proves message consistency; cross-source comparison requires both. |
| Scheduler/node status root rows | Yes | Partial | DB has normalized status timeline; files show root rows but also stale `scheduler.json` vs node-local `node.json`. |
| Generation-1 child rows absent because no child admitted | Yes | Yes | Both show zero child-plan children and no child node/runtime artifacts. |
| Root runner request | Yes | Yes | DB rows and `runner-request.json` both capture root request and argv. |
| Runner result absent due no child spawn | Partial | Yes | DB absence supports it; files show no runner-result/invocation/channel/result paths. |
| Invalid schema rows / current schema | Yes | Partial | DB can count invalid rows; files can only show inspected file schema versions. |
| Owner DB fresh/current-schema | Partial | No | DB can show no known invalid rows; files cannot establish DB freshness. |
| Claims relying only on `eval_record_ref` | Yes | N/A | DB section explicitly avoids payload-ref-only claims where normalized rows are absent. |
| Agent turn exists | Yes | Yes | DB has one `eval_agent_turn`; files have turn-live summary/trace. |
| Tool-event rows / persisted tool events | Yes | Yes | Both show none. |
| Model/message/trace event absence | Yes | Yes | DB relations are empty; files have no responses/events. |
| Why event/model/tool trace rows are absent | No | No | Neither source proves writer bypass vs no events before timeout. |
| Logs/checkpoints for rejected attempt | Partial | Yes | DB stores refs; files confirm actual diagnostics/turn-live files and contents. |
| Elapsed request-to-timeout timeline | Partial | Yes | DB has enough timestamps to infer ~300s; files give request mtime, turn-live mtime, node update, and timeout file. |
| No R8 selectable state with runnable children | Partial | Yes | Both show a zero-child rejected plan; neither has explicit R8 success/failure ledger. |
| No R9 schedule / R10 selection / R11 fanout | Yes | Yes | Absence of child, selection, invocation, channel, result artifacts/rows. |
| No materialize/build/spawn/observe | Yes | Yes | Both show no child runtime/build/observe evidence. |
| No child invocation/channel/result | Yes | Yes | Rows and files are absent. |
| No runner-result proof | Yes | Yes | Rows/files absent. |
| No selection/handoff/History seal | Partial | Yes | DB has no selection/continuation rows but no History schema; files show no `evaluations/` or `history/` directory. |

High-level takeaway: **DB-only** now answers the normalized parent/setup/child-plan rejection story, but misses policy, harness workspace/diff, and detailed trace payloads. **File-only** answers policy and harness mechanics better, but exposes stale projection drift and still cannot provide a typed R-phase ledger or explicit DB freshness proof.

## Method

Context selection only:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop walk use \
  /home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536 \
  --format json
```

All evidence below came from commands of this form:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop walk db_query \
  --format json \
  --script '<cozo query>'
```

Useful relation counts from the DB:

```text
eval_campaign                              1
eval_profile_commitment                    1
eval_scheduler_node                        1
eval_scheduler_node_status_event           3
eval_scheduler_node_target_part            0
eval_runner_request                        1
eval_runner_request_arg                    4
eval_runner_request_target_part            0
eval_runner_result                         0
eval_child_plan                            1
eval_child_plan_child                      0
eval_child_plan_rejected_attempt           1
eval_agent_turn                            1
eval_agent_turn_event                      0
eval_tool_event                            0
eval_model_exchange                        0
eval_message_event                         0
eval_trace_event                           0
eval_transition_event                      1
eval_invocation                            0
eval_channel_message                       0
eval_channel_receipt                       0
eval_selection_decision                    0
eval_continuation_decision                 0
eval_baseline                              1
eval_baseline_instance                     1
eval_record_ref                            4
```

Schema-hygiene counts visible from DB:

```text
invalid_child_plan             0
invalid_scheduler_node         0
invalid_scheduler_status       0
invalid_runner_request         0
invalid_runner_result          0
```

Present projection schema versions:

```text
eval_child_plan       prototype1-child-plan-file.v1
eval_runner_request   prototype1-runner-request.v1
eval_scheduler_node   prototype1-scheduler-node.v1
```

## Overall trajectory

### Which campaign, parent node, generation, branch, worktree, and binary were active?

**Answer from DB:**

- campaign: `p1-runnerio-db-20260627-125536`
- prototype root: `/home/brasides/.ploke-eval/campaigns/p1-runnerio-db-20260627-125536/prototype1`
- storage backend: `dual-strict`
- node / parent id: `node-b86fe92de458ef31`
- generation: `0`
- branch: `prototype1-parent-p1-runnerio-db-20260627-125536-gen0`
- runner workspace root: `/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536`
- runner binary path: `/home/brasides/.ploke-eval/campaigns/p1-runnerio-db-20260627-125536/prototype1/nodes/node-b86fe92de458ef31/bin/ploke-eval`

Evidence came from `eval_campaign`, `eval_scheduler_node`, and `eval_runner_request`.

### Which exact commit/binary drove authority-bearing live steps?

**DB-only answer:** not fully answerable.

The DB records a `binary_path` and runner argv, but no normalized binary digest or git commit for the live authority-bearing binary in this run. `eval_binary_ref` has `0` rows.

Known from DB:

```text
binary_path=/home/brasides/.ploke-eval/campaigns/p1-runnerio-db-20260627-125536/prototype1/nodes/node-b86fe92de458ef31/bin/ploke-eval
runner args: loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536
```

Gap: DB needs normalized binary/build/git-head facts for authority-bearing live execution.

### What was the reconstructed phase before each step?

**DB-only answer:** not fully answerable.

The DB has one normalized transition event:

```text
transition=r4c_to_r5
phase=parent_started
outcome=recorded
node_id=node-b86fe92de458ef31
generation=0
```

But it does not contain a complete walk phase ledger for every reconstructed `R*` phase before each operator step. Later progress is inferable only from durable facts:

- baseline row exists and is `complete`, implying baseline establishment happened;
- child-plan row exists with rejected attempt, implying R7 child planning was attempted;
- no child-plan children, invocations, channels, runner results, selection, or continuation rows exist.

Gap: DB needs normalized walk-phase / transition attempt rows if we want to answer this without `walk show` or journal/file replay.

### Which R transitions actually completed?

**DB-only answer:** only partially answerable.

Directly recorded in `eval_transition_event`:

```text
r4c_to_r5 -> parent_started -> recorded
```

Inferred from normalized DB facts:

- setup/context existed (`eval_campaign`, `eval_profile_commitment`);
- root scheduler and runner-request rows existed;
- parent start was recorded once;
- baseline completed (`eval_baseline.status=complete`);
- policy/budget likely reached because child planning was attempted, but search policy/child budget are not normalized yet;
- R7→R8 did **not** complete to a runnable child-plan authority state because `eval_child_plan.child_count=0`, `eval_child_plan_child=0`, and the scheduler node ended `failed`.

Not completed, by absence of DB rows:

- child schedule/fanout/runtime materialization/build/spawn/observe;
- selection;
- continuation;
- handoff;
- final report.

### Which transition failed, and did it fail before or after writing durable evidence?

**Answer from DB:** the failed live edge is the R7→R8 child-plan admission edge, and it failed **after** durable rejected child-plan evidence was written.

Evidence:

```text
eval_child_plan:
  parent_node_id=node-b86fe92de458ef31
  child_generation=1
  child_count=0
  rejected_count=1
  recorded_at=2026-06-27T13:13:02.076095573+00:00

eval_child_plan_rejected_attempt:
  producer_id=prototype1:broad-headless-tui-adapter-v1
  proposal_id=broad-harness-request:node-b86fe92de458ef31
  outcome=rejected
  reason=broad headless-tui slot 'broad-harness-request:node-b86fe92de458ef31' timed out after 300 seconds; diagnostics='...node-b86fe92de458ef31.headless-tui.json'
```

The current scheduler node status is `failed`, with a failed status event recorded at `2026-06-27T13:13:02.075766090+00:00`.

### Was the failure an operator guard, e.g. missing `--watch`, or a real live-edge failure?

**Answer from DB:** DB evidence supports this being a real live-edge failure, not merely an operator guard.

Why:

- `eval_child_plan_rejected_attempt` was written by `prototype1:broad-headless-tui-adapter-v1`.
- `proposal_id` was a concrete broad-harness slot: `broad-harness-request:node-b86fe92de458ef31`.
- `reason` says the broad headless-TUI slot timed out after 300 seconds.
- `eval_agent_turn` has a matching `request_id`/`task_id`.

Caveat: the DB does not store the exact operator CLI flags (`--watch`), so this is an evidence-based conclusion from live harness rows, not a direct flag audit.

## Parent identity and authority

### Was the parent identity loaded from active checkout or successor invocation?

**DB-only answer:** active-checkout/genesis path is strongly indicated, but the exact identity source is not normalized.

Evidence:

- generation is `0`;
- root node has `parent_node_id=null`;
- runner request invokes `loop prototype1-state --repo-root <worktree>` and has no runtime/handoff id;
- `eval_transition_event.runtime_id` is empty;
- `eval_invocation` has `0` rows.

This points to a generation-0 active-checkout parent, not a successor invocation.

Gap: DB should normalize parent identity source / startup authority branch directly.

### Did parent identity, scheduler node, branch id, and generation agree?

**Answer from DB:** yes for the normalized rows available.

Join of `eval_scheduler_node`, `eval_runner_request`, and `eval_baseline` agrees on:

```text
node_id=node-b86fe92de458ef31
generation=0
branch_id=prototype1-parent-p1-runnerio-db-20260627-125536-gen0
target_relpath=.ploke/prototype1/parent_identity.json
```

### Was this genesis startup or predecessor handoff startup?

**DB-only answer:** genesis startup is strongly indicated.

Evidence:

- generation `0`;
- no invocations;
- no channel messages or receipts;
- no continuation/handoff rows;
- no runtime id on the parent-start transition.

Gap: DB does not explicitly store `startup_kind=genesis|predecessor`.

### Was startup validated through History/checkout evidence, or only local genesis absence?

**DB-only answer:** not fully answerable.

The DB records parent-start evidence and generation-0 rows, but no normalized History startup/admission facts for this run. There are no history-block/head rows in the current eval-store schema slice.

Best DB-only conclusion: this appears to be generation-0 genesis startup; whether it was local configured-store absence versus a stronger History admission is not directly represented.

### Did parent-start evidence get recorded once, with stable parent/runtime ids?

**Answer from DB:** yes, once, with stable parent/node id and no runtime id.

Evidence:

```text
eval_transition_event count = 1
transition=r4c_to_r5
phase=parent_started
parent_id=node-b86fe92de458ef31
node_id=node-b86fe92de458ef31
generation=0
runtime_id=""
```

`eval_record_ref` also has one `parent_started` and one `resource_parent_start` citation row, both produced by `node-b86fe92de458ef31`.

## Policy and budget

### What admitted run profile was used?

**Answer from DB:**

```text
profile_name=p1-broad-db-1x1-live
profile_path=/home/brasides/.ploke-eval/campaigns/p1-runnerio-db-20260627-125536/prototype1/run-profile.toml
source_path=/home/brasides/.ploke-eval/profiles/prototype1/p1-broad-db-1x1-20260627-111548.toml
content_sha256=462cfa85dca62eebb6483af2aaa73fe347ae71b7ea9be0a1ecd774a6922cf223
storage_ref={"worktree_root":"~/.ploke-eval/worktrees","eval":{"backend":"dual-strict"}}
```

### What were `max_generations`, `max_total_nodes`, child min/max, schedule mode, retries, and broad-TUI timeout?

**DB-only answer:** mostly not normalized yet.

The DB has some campaign/protocol budgets:

```text
eval_campaign_eval_budget:
  max_turns=40
  max_tool_calls=200
  wall_clock_secs=1800

eval_campaign_protocol_policy:
  max_concurrency=100
  tool_review_parallelism=1
  max_tokens=4096
  reasoning_mode=omit
```

But it does **not** expose normalized search-policy fields such as:

- `max_generations`,
- `max_total_nodes`,
- child `min/max`,
- child schedule mode,
- broad retry count.

The timeout is only visible indirectly in the rejected attempt reason:

```text
timed out after 300 seconds
```

Gap: this confirms the existing known gap: search policy / child budget / broad harness parameters need normalized DB relations.

### Why was `child_budget.min = 1` for this run?

**DB-only answer:** not directly answerable.

We can infer a minimum greater than zero because a zero-child plan was rejected as below-minimum and because the profile name is `p1-broad-db-1x1-live`, but the actual `child_budget.min` is not normalized in the DB.

### Which generation source was active: deterministic, broad-harness, or other?

**Answer from DB:** broad-harness.

Evidence:

```text
eval_child_plan_rejected_attempt.producer_id=prototype1:broad-headless-tui-adapter-v1
proposal_id=broad-harness-request:node-b86fe92de458ef31
```

### Was deterministic no-op generation correctly disabled?

**DB-only answer:** not directly answerable as a configuration fact.

The DB does prove this run used broad-harness rather than deterministic generation. It does not store a normalized flag saying deterministic generation was disabled globally.

## R7→R8 child-plan mechanics

### Did a child-plan message already exist before the step?

**DB-only answer:** not answerable as a before/after fact.

The DB currently contains one child-plan row recorded at `2026-06-27T13:13:02.076095573+00:00`. It does not record whether that row existed before the R7→R8 step began.

### If yes, was it bound to the same parent node and expected child generation?

**Answer from DB for the persisted plan:** yes.

```text
parent_node_id=node-b86fe92de458ef31
child_generation=1
```

The parent/root scheduler row is generation `0`, so child generation `1` is consistent.

### If no, what request id/slot was published?

**Answer from DB:**

```text
proposal_id=broad-harness-request:node-b86fe92de458ef31
request_id=broad-harness-request:node-b86fe92de458ef31
task_id=broad-harness-request:node-b86fe92de458ef31
```

The DB does not currently have first-class `eval_harness_request` rows; this comes from `eval_child_plan_rejected_attempt` and `eval_agent_turn`.

### What prompt/model/provider/route was used?

**DB-only answer:** partially answerable.

Campaign/protocol rows:

```text
campaign model_id=google/gemini-3.5-flash
campaign route_source=direct_google
campaign provider_slug=null
protocol model_id=google/gemini-3.5-flash
protocol route_source=direct_google
```

Agent-turn row for the broad harness attempt:

```text
selected_model=google/gemini-2.5-pro
provider=null
route_source=null
```

Prompt text/messages are not normalized. `eval_agent_turn` stores refs to trace/summary/full response paths, but those are file paths and were not read for this inspection.

### Did the headless TUI start successfully?

**DB-only answer:** it started far enough to create DB-visible broad-harness attempt evidence, but DB cannot prove full TUI initialization.

Evidence:

- `eval_agent_turn = 1` for the broad-harness request;
- rejected attempt producer is `prototype1:broad-headless-tui-adapter-v1`;
- rejected reason names the headless-TUI slot and timeout.

### Did it call the provider?

**DB-only answer:** not provable.

Evidence against a completed provider exchange:

```text
eval_model_exchange = 0
eval_message_event = 0
eval_agent_turn_event = 0
eval_agent_turn.response_count = 0
```

The agent-turn row selected a model, but the DB has no model exchange/response row.

### Did it execute tools?

**Answer from DB:** no normalized tool events were recorded.

```text
eval_tool_event = 0
```

This means either no tools executed or this path did not persist them before timeout. DB alone cannot distinguish those two possibilities.

### Did it timeout, crash, or submit a result?

**Answer from DB:** timeout.

Rejected attempt reason:

```text
broad headless-tui slot 'broad-harness-request:node-b86fe92de458ef31' timed out after 300 seconds
```

No child-plan children were admitted. No runner result exists. DB does not show a submitted admitted result.

### What workspace did it mutate?

**DB-only answer:** not answerable for the broad-harness candidate workspace.

The root runner workspace is recorded:

```text
/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536
```

But no normalized harness workspace relation exists, and `eval_child_plan_child` is empty, so no admitted child workspace was recorded.

### What changed paths were detected?

**DB-only answer:** none are normalized for this failed attempt.

Relevant absence:

```text
eval_child_plan_child = 0
eval_patch = 0
eval_apply_event = 0
eval_artifact = 0
eval_tool_event = 0
```

The DB cannot say whether there were temporary unadmitted diffs inside the candidate workspace.

### Were changes rejected because they were empty, protected, malformed, or unbound?

**Answer from DB:** timeout is the only normalized rejection reason for this run.

```text
reason=... timed out after 300 seconds ...
policy=workspace_except_ploke_eval
outcome=rejected
```

No DB row says `empty`, `protected`, `malformed`, or `unbound` for this run.

### Was a rejected child-plan persisted before returning the below-minimum error?

**Answer from DB:** yes.

Evidence:

```text
eval_child_plan.child_count=0
eval_child_plan.rejected_count=1
eval_child_plan_rejected_attempt count=1
```

This is exactly the durable rejected-plan shape expected by the zero-admission fix.

### Does the normalized DB row show rejected attempt reason and producer id?

**Answer from DB:** yes.

```text
producer_id=prototype1:broad-headless-tui-adapter-v1
policy=workspace_except_ploke_eval
outcome=rejected
target_relpath=.
reason=broad headless-tui slot 'broad-harness-request:node-b86fe92de458ef31' timed out after 300 seconds; diagnostics='...headless-tui.json'
```

## Candidate/admission details

### How many candidate attempts were made?

**Answer from DB:** one rejected broad-harness attempt is recorded.

```text
eval_child_plan_rejected_attempt = 1
attempt_index=0
```

No admitted child candidates exist.

### For each attempt: target path, workspace root, candidate id, branch id, status?

**Answer from DB:** partially answerable.

Rejected attempt:

```text
attempt_index=0
producer_id=prototype1:broad-headless-tui-adapter-v1
proposal_id=broad-harness-request:node-b86fe92de458ef31
run_id=c6c8776ef84c070e4ccf22689a3168c4c93f99005a721f15d649c62145dc85ae
target_relpath=.
outcome=rejected
policy=workspace_except_ploke_eval
reason=timeout after 300 seconds
```

Workspace root, candidate id, and branch id are not normalized for rejected attempts. Those fields exist for admitted `eval_child_plan_child` rows, but there are zero such rows.

### Why exactly was each rejected?

**Answer from DB:** the sole attempt timed out after 300 seconds.

### Did protected-write enforcement deny anything?

**DB-only answer:** no protected-write denial is visible.

There are no `eval_tool_event` rows, no `eval_trace_event` rows, and the rejected-attempt reason is timeout rather than protected-write denial.

### Did “no admitted changes” mean no file diff, no accepted tool edit, or no committed candidate branch?

**Answer for this run:** the DB reason is **not** “no admitted changes”; it is timeout.

Therefore this DB cannot answer the semantics of a “no admitted changes” failure for a different run. For this run, there are no normalized rows for changed paths, accepted edits, or committed candidate branches.

### If timeout: last observed tool/model event before timeout?

**Answer from DB:** no tool/model event was recorded before timeout.

Relevant rows:

```text
eval_tool_event = 0
eval_model_exchange = 0
eval_message_event = 0
eval_agent_turn_event = 0
```

The DB-visible timing sequence is:

```text
scheduler status running: 2026-06-27T13:08:00.111230448+00:00
agent turn recorded:      2026-06-27T13:13:01.898272349+00:00
scheduler status failed:  2026-06-27T13:13:02.075766090+00:00
child plan recorded:      2026-06-27T13:13:02.076095573+00:00
```

Elapsed time from scheduler `running` to `failed` is about five minutes, matching the 300-second timeout reason.

## DB coverage

### Do `eval_child_plan`, `eval_child_plan_child`, and `eval_child_plan_rejected_attempt` agree with the child-plan message?

**DB-only answer:** internally consistent, but not independently compared to the file message because no files were read.

DB facts:

```text
eval_child_plan.child_count=0
eval_child_plan.rejected_count=1
eval_child_plan_child count=0
eval_child_plan_rejected_attempt count=1
plan_id matches rejected attempt plan_id
parent_node_id=node-b86fe92de458ef31
child_generation=1
schema_version=prototype1-child-plan-file.v1
```

The DB also stores `message_path` and `message_sha256`, but this inspection did not read the message file to compare bytes.

### Do `eval_scheduler_node` and `eval_scheduler_node_status_event` show the parent/root node?

**Answer from DB:** yes.

`eval_scheduler_node` has one row for `node-b86fe92de458ef31`, generation `0`, branch `prototype1-parent-p1-runnerio-db-20260627-125536-gen0`, target `.ploke/prototype1/parent_identity.json`.

Status timeline:

```text
planned 2026-06-27T12:56:28.631832768+00:00
running 2026-06-27T13:08:00.111230448+00:00
failed  2026-06-27T13:13:02.075766090+00:00
```

The current scheduler-node projection status is `failed`.

### Were generation-1 child scheduler rows absent because no child was admitted?

**Answer from DB:** yes, as far as DB facts can show.

Evidence:

```text
eval_scheduler_node rows by generation: generation 0 => 1
eval_child_plan_child = 0
eval_child_plan.child_count = 0
```

No generation-1 scheduler rows exist.

### Does `eval_runner_request` show root request rows?

**Answer from DB:** yes.

```text
eval_runner_request = 1
eval_runner_request_arg = 4
node_id=node-b86fe92de458ef31
branch_id=prototype1-parent-p1-runnerio-db-20260627-125536-gen0
workspace_root=/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536
args=loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536
```

### Is `eval_runner_result` absent because no child was spawned?

**DB-only answer:** it is absent, and no child spawn evidence exists.

Evidence:

```text
eval_runner_result = 0
eval_invocation = 0
eval_channel_message = 0
eval_child_plan_child = 0
```

The causal “because” is not stored as a normalized relation, but the DB shows no admitted child and no runtime attempt evidence.

### Are invalid schema rows zero?

**Answer from DB:** yes.

```text
invalid_child_plan             0
invalid_scheduler_node         0
invalid_scheduler_status       0
invalid_runner_request         0
invalid_runner_result          0
```

### Is the owner DB fresh/current-schema, not old contaminated?

**DB-only answer:** current-schema rows are present and invalid schema counts are zero for the normalized slices inspected.

Evidence:

```text
eval_child_plan       prototype1-child-plan-file.v1
eval_runner_request   prototype1-runner-request.v1
eval_scheduler_node   prototype1-scheduler-node.v1
invalid_* counts       0
```

Caveat: DB-only inspection can prove no visible invalid rows for these checked schemas; it cannot prove the DB was never reused historically except by absence of known contamination indicators.

### Are claims relying only on `eval_record_ref` instead of normalized rows?

**Answer:** main trajectory claims above rely on normalized rows where available. `eval_record_ref` is present only as citation/provenance for:

```text
parent_started          1
resource_parent_start   1
runner_request          1
scheduler_node          1
```

For several questions, the answer is “not normalized / not answerable,” rather than falling back to `eval_record_ref.payload_json` as authority.

## Low-level runtime/tool trace

### Was there an `eval_agent_turn`?

**Answer from DB:** yes, one.

```text
turn_id=5fe13e3e8bc84caf7704c9cdf534a365761c56b78ed4a3fab2d45f13cdfb8725
request_id=broad-harness-request:node-b86fe92de458ef31
task_id=broad-harness-request:node-b86fe92de458ef31
selected_model=google/gemini-2.5-pro
event_count=0
response_count=0
recorded_at=2026-06-27T13:13:01.898272349+00:00
```

### Were there `eval_tool_event` rows?

**Answer from DB:** no.

```text
eval_tool_event = 0
```

### Were `eval_model_exchange`, `eval_message_event`, and `eval_trace_event` absent?

**Answer from DB:** yes.

```text
eval_model_exchange = 0
eval_message_event  = 0
eval_trace_event    = 0
```

### If absent, is that because the path bypasses those writers, or because the run failed before they fired?

**DB-only answer:** not distinguishable.

The DB shows a timed-out broad-harness attempt with an agent-turn summary row but no event/model/tool/message rows. It does not say whether the writer path was bypassed or whether timeout occurred before those rows were emitted.

### Which logs/checkpoints correspond to the rejected attempt?

**DB-only answer:** the DB stores refs/paths but they were not read.

From `eval_agent_turn`:

```text
trace_ref=.../messages/edit-harness-result/node-b86fe92de458ef31.turn-live/agent-turn-trace.json
summary_ref=.../messages/edit-harness-result/node-b86fe92de458ef31.turn-live/agent-turn-summary.json
full_response_ref=.../messages/edit-harness-result/node-b86fe92de458ef31.turn-live/llm-full-responses.jsonl
```

From `eval_child_plan_rejected_attempt.reason`:

```text
diagnostics=.../messages/edit-harness-result/node-b86fe92de458ef31.headless-tui.json
```

These are DB values only; no file contents were inspected.

### What elapsed time happened between request publication, first model/tool event, last event, and timeout?

**DB-only answer:** partial.

No first/last model/tool events exist in DB. The visible timing is:

```text
scheduler planned: 2026-06-27T12:56:28.631832768+00:00
parent start:      2026-06-27T12:56:51.148674946+00:00 ingested
baseline complete: 2026-06-27T13:07:55.711746116+00:00
scheduler running: 2026-06-27T13:08:00.111230448+00:00
agent turn row:    2026-06-27T13:13:01.898272349+00:00
scheduler failed:  2026-06-27T13:13:02.075766090+00:00
child plan row:    2026-06-27T13:13:02.076095573+00:00
```

The `running`→`failed` interval is approximately 302 seconds, consistent with the recorded 300-second headless-TUI timeout.

## What did not happen because of the blocker?

### No `R8` selectable parent state.

**DB-only answer:** no successful R8 is recorded. The child-plan row has `child_count=0`; the failed scheduler status and rejected attempt indicate R7→R8 did not admit runnable children.

Caveat: there is no explicit normalized `R8 succeeded/failed` transition row.

### No `R9` child schedule shaping.

**DB-only answer:** no normalized schedule rows exist. Search policy/child schedule relations are not yet implemented, and there are zero child rows to schedule.

### No `R10` selection strategy over runnable children.

**Answer from DB:** supported by absence of selection rows.

```text
eval_selection_decision = 0
eval_selection_candidate = 0
eval_selection_score = 0
```

### No `R11` child fanout.

**Answer from DB:** yes.

```text
eval_invocation = 0
eval_channel_message = 0
eval_runner_result = 0
eval_build_event = 0
eval_binary_ref = 0
```

### No materialize/build/spawn/observe.

**Answer from DB:** yes, no normalized child runtime/build/observe evidence exists.

### No child invocation/channel/result.

**Answer from DB:** yes.

```text
eval_invocation = 0
eval_channel_message = 0
eval_channel_receipt = 0
eval_runner_result = 0
```

### No runner result proof.

**Answer from DB:** yes.

```text
eval_runner_result = 0
```

### No selection/handoff/History seal.

**Answer from DB:** no selection or continuation rows exist. History-seal relations are not yet normalized in this eval-store schema, so the DB can only show absence of selection/continuation facts:

```text
eval_selection_decision = 0
eval_continuation_decision = 0
```

## Summary: what the DB can and cannot explain today

DB can explain this run at a useful high level:

- gen0 root parent started;
- baseline completed;
- root scheduler and runner request were normalized;
- broad-harness child planning was attempted;
- the attempt timed out after 300 seconds;
- a rejected zero-child child-plan was persisted;
- no child runtime was admitted, spawned, or observed.

DB cannot yet fully explain:

- exact authority-bearing git commit/binary digest;
- complete R-phase transition timeline;
- normalized search policy / child budget / schedule-mode values;
- harness request/result workspace and changed paths;
- provider/message/tool low-level event timeline;
- why event/model/tool trace writers emitted no rows;
- History startup/handoff authority details.

Those gaps align with the current DB-first roadmap: normalize search policy/child budget, harness request/result, richer LLM/model/message/trace rows, and History/parent identity/handoff facts.

---

# File-only trajectory Q&A for the same run

**Run inspected:** `p1-runnerio-db-20260627-125536`  
**Constraint for this section:** answers below use only persisted files and persisted git/worktree state. I did not use database queries for this section. Paths and facts come from files under:

```text
/home/brasides/.ploke-eval/campaigns/p1-runnerio-db-20260627-125536
/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536
```

Primary files inspected:

```text
campaign.json
closure-state.json
prototype1/run-profile.toml
prototype1/run-profile.commitment.json
prototype1/scheduler.json
prototype1/transition-journal.jsonl
prototype1/nodes/node-b86fe92de458ef31/node.json
prototype1/nodes/node-b86fe92de458ef31/runner-request.json
prototype1/messages/child-plan/node-b86fe92de458ef31.json
prototype1/messages/edit-harness-request/node-b86fe92de458ef31.json
prototype1/messages/edit-harness-result/node-b86fe92de458ef31.headless-tui.json
prototype1/messages/edit-harness-result/node-b86fe92de458ef31.turn-live/*
prototype1/messages/pre-child-planning/node-b86fe92de458ef31.json
.ploke/prototype1/parent_identity.json in the active worktree
candidate workspace git status/diff under prototype1/workspaces/edit-harness/node-b86fe92de458ef31
```

Important file-only caveat: mutable projections are not all consistent. In particular, `scheduler.json` still has the root node as `planned`, while `nodes/node-b86fe92de458ef31/node.json` records the same node as `failed`. The node-local projection and child-plan/harness files are more specific for the blocker.

## Overall trajectory

### Which campaign, parent node, generation, branch, worktree, and binary were active?

**Answer from files:**

- campaign: `p1-runnerio-db-20260627-125536` from `campaign.json` and parent identity.
- active parent/root node: `node-b86fe92de458ef31` from `.ploke/prototype1/parent_identity.json`, `node.json`, `runner-request.json`, and child-plan file.
- generation: `0` from parent identity, `node.json`, and `runner-request.json`.
- branch/artifact branch: `prototype1-parent-p1-runnerio-db-20260627-125536-gen0` from parent identity, `node.json`, and runner request.
- active worktree: `/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536` from parent identity/journal/runner request.
- configured binary path: `/home/brasides/.ploke-eval/campaigns/p1-runnerio-db-20260627-125536/prototype1/nodes/node-b86fe92de458ef31/bin/ploke-eval` from `node.json` and `runner-request.json`.

File-existence note: the configured node-local binary path was absent when inspected, so the path is persisted but the binary artifact was not present in this node directory.

### Which exact commit/binary drove authority-bearing live steps?

**Answer from files/git state:**

- active parent worktree branch: `prototype1-parent-p1-runnerio-db-20260627-125536-gen0`
- active parent worktree HEAD: `42ba40f726149553cadb3cbc4e45396a9e49aad1`
- broad-harness request admission binding target artifact: `artifact:git-commit:42ba40f726149553cadb3cbc4e45396a9e49aad1`
- candidate workspace branch: `prototype1-broad-broad-harness-request-node-b86fe92de458ef31`
- candidate workspace HEAD: `42ba40f726149553cadb3cbc4e45396a9e49aad1`

Binary path is persisted in `node.json`/`runner-request.json`, but the binary file was absent. So files can answer the commit/artifact coordinate more strongly than the node-local binary artifact.

### What was the reconstructed phase before each step?

**File-only answer:** not fully answerable.

Persisted files show these milestones:

- `transition-journal.jsonl` contains one `parent_started` record and one `resource` record.
- `closure-state.json` shows eval closure complete and protocol partial.
- `messages/pre-child-planning/node-b86fe92de458ef31.json` shows pre-child planning completed at `2026-06-27T13:08:01.593807504+00:00`.
- `messages/edit-harness-request/node-b86fe92de458ef31.json` exists.
- `messages/edit-harness-result/node-b86fe92de458ef31.headless-tui.json` records timeout.
- `messages/child-plan/node-b86fe92de458ef31.json` records a zero-child rejected plan.

But there is no persisted file-only R-phase ledger saying “the walk was at R7 before this step.” That phase is inferable from the shape of the files, not directly recorded as a phase transition series.

### Which R transitions actually completed?

**File-only answer:** partially answerable by persisted artifacts.

Directly recorded:

```text
transition-journal.jsonl line 1: kind=parent_started
transition-journal.jsonl line 2: kind=resource, phase=parent_start, subject=cargo_target, status=missing
```

Inferred from files:

- Parent identity was initialized/loaded: `.ploke/prototype1/parent_identity.json` exists.
- Parent start reached journal recording: `transition-journal.jsonl` exists with `parent_started`.
- Baseline eval closure completed: `closure-state.json` has `eval.status=complete`, `eval.complete_total=1`.
- Policy/child planning was entered: run profile, scheduler policy, pre-child-planning review, harness request, and child-plan file exist.
- R7 child-plan attempt did not produce runnable children: child-plan `children=[]`, one rejected surface attempt.

Not completed by file absence:

- no `nodes/<node>/invocations/`;
- no `nodes/<node>/channels/`;
- no `nodes/<node>/results/`;
- no `nodes/<node>/runner-result.json`;
- no `evaluations/` directory;
- no `history/` directory.

### Which transition failed, and did it fail before or after writing durable evidence?

**Answer from files:** the child-plan live edge failed after writing durable failure evidence.

Evidence:

- `node.json` records root node status `failed`, updated at `2026-06-27T13:13:02.075766090+00:00`.
- `messages/edit-harness-result/node-b86fe92de458ef31.headless-tui.json` records:

```json
{
  "terminal": { "terminal": "timed_out", "secs": 300 },
  "attempts": [],
  "events": [],
  "validations": []
}
```

- `messages/child-plan/node-b86fe92de458ef31.json` records `children=[]` and one rejected surface attempt with the timeout reason.

So the failed transition persisted a rejected child-plan message before returning the below-minimum/zero-child outcome.

### Was the failure an operator guard, e.g. missing `--watch`, or a real live-edge failure?

**Answer from files:** real live-edge failure.

Evidence:

- A broad-harness request file exists.
- A headless-TUI diagnostics file exists.
- A turn-live directory exists.
- A child-plan file records a rejected broad-headless-TUI attempt.
- The headless diagnostics explicitly say `timed_out` after 300 seconds.

The files do not preserve the exact operator CLI flags, but the persisted live harness artifacts would not exist for a pure missing-`--watch` guard.

## Parent identity and authority

### Was the parent identity loaded from active checkout or successor invocation?

**Answer from files:** active checkout / generation-0 parent identity.

Evidence:

- `.ploke/prototype1/parent_identity.json` exists in the active worktree and records generation `0`.
- `transition-journal.jsonl` parent-start record embeds the same parent identity and `repo_root`.
- There are no `invocations/` files under the root node directory.
- There are no successor-ready or successor-completion files.

### Did parent identity, scheduler node, branch id, and generation agree?

**Answer from files:** mostly yes for identity/branch/generation; status/candidate fields diverge between projections.

Agreement:

```text
node_id / parent_id = node-b86fe92de458ef31
generation = 0
branch_id = prototype1-parent-p1-runnerio-db-20260627-125536-gen0
target_relpath = .ploke/prototype1/parent_identity.json
```

This appears in parent identity, runner request, child plan parent id, and node projections.

Projection divergence:

- `scheduler.json` root node has `candidate_id=root-parent` and `status=planned`.
- `nodes/node-b86fe92de458ef31/node.json` has `candidate_id=node-b86fe92de458ef31` and `status=failed`.

The file-only view therefore exposes stale/mutable scheduler projection drift.

### Was this genesis startup or predecessor handoff startup?

**Answer from files:** genesis startup.

Evidence:

- generation `0` in parent identity.
- no invocation files.
- no successor handoff files.
- no `history/` directory for predecessor sealed-head validation.

### Was startup validated through History/checkout evidence, or only local genesis absence?

**Answer from files:** local genesis path, not History predecessor validation.

Evidence:

- no `prototype1/history/` directory exists for this run.
- generation is `0`.
- no successor invocation is present.

Files can show genesis startup artifacts but do not contain a stronger explicit startup-admission proof.

### Did parent-start evidence get recorded once, with stable parent/runtime ids?

**Answer from files:** yes, one parent-start record.

`transition-journal.jsonl` has two lines:

1. `kind=parent_started`, `parent_id=node-b86fe92de458ef31`, `node_id=node-b86fe92de458ef31`, generation `0`, pid `528625`, repo root `/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536`.
2. `kind=resource`, `subject=cargo_target`, `phase=parent_start`, status `missing`.

No runtime id is present for this generation-0 parent-start journal entry.

## Policy and budget

### What admitted run profile was used?

**Answer from files:**

From `run-profile.toml` and `run-profile.commitment.json`:

```text
name = p1-broad-db-1x1-live
profile_path = /home/brasides/.ploke-eval/campaigns/p1-runnerio-db-20260627-125536/prototype1/run-profile.toml
source_path = /home/brasides/.ploke-eval/profiles/prototype1/p1-broad-db-1x1-20260627-111548.toml
sha256 = 462cfa85dca62eebb6483af2aaa73fe347ae71b7ea9be0a1ecd774a6922cf223
admitted_at = 2026-06-27T12:56:28.340553384+00:00
storage.eval.backend = dual-strict
```

### What were `max_generations`, `max_total_nodes`, child min/max, schedule mode, retries, and broad-TUI timeout?

**Answer from files:** fully answerable from `run-profile.toml`, `scheduler.json`, and the edit-harness request.

From profile/scheduler:

```text
search.max_generations = 1
search.max_total_nodes = 4
search.schedule / child_schedule_mode = full-batch
search.stop_on_first_keep = false
search.require_keep_for_continuation = false
search.explore_from_rejected = true
search.children.min = 1
search.children.max = 1
search.children.parallel_targets = 1
```

From broad TUI execution profile:

```text
execution.broad_tui.max_attempts = 1
execution.broad_tui.fresh_slots_per_child = 1
execution.broad_tui.graph_nearest = 24
execution.broad_tui.timeout_secs = 300
```

From edit-harness request:

```text
request.child_budget.min_children = 1
request.child_budget.max_children = 1
request.graph_restriction.nearest_items = 24
request.graph_restriction.seed_modules = ["crates/ploke-tui/src/tools/mod.rs"]
```

### Why was `child_budget.min = 1` for this run?

**Answer from files:** because the admitted run profile and published harness request both set it.

Evidence:

```text
run-profile.toml: [search.children] min = 1
scheduler.json: policy.child_budget.min = 1
edit-harness request: request.child_budget.min_children = 1
```

### Which generation source was active: deterministic, broad-harness, or other?

**Answer from files:** broad harness request.

Evidence:

```text
run-profile.toml: [generation] source = "broad-harness-request"
child-plan rejected attempt producer_id = prototype1:broad-headless-tui-adapter-v1
edit-harness request_id = broad-harness-request:node-b86fe92de458ef31
```

### Was deterministic no-op generation correctly disabled?

**File-only answer:** this run did not use deterministic generation.

Files prove the active generation source was `broad-harness-request`. They do not prove the global code-level deterministic planner guard was enabled/disabled.

## R7→R8 child-plan mechanics

### Did a child-plan message already exist before the step?

**File-only answer:** not conclusively answerable.

The child-plan file exists and has an mtime of `2026-06-27 06:13:02 -0700`. The request file mtime is `2026-06-27 06:08:00 -0700`, and the pre-child-planning review was generated at `2026-06-27T13:08:01.593807504+00:00`. This supports that the child-plan file was written during/after the broad-harness attempt, but there is no file-only before/after snapshot proving non-existence before the step.

### If yes, was it bound to the same parent node and expected child generation?

**Answer for the persisted child-plan file:** yes.

```text
parent_node_id = node-b86fe92de458ef31
child_generation = 1
```

The parent identity generation is `0`, so child generation `1` is expected.

### If no, what request id/slot was published?

**Answer from files:**

```text
request_id = broad-harness-request:node-b86fe92de458ef31
request_hash = c6c8776ef84c070e4ccf22689a3168c4c93f99005a721f15d649c62145dc85ae
submitted_result_path = .../messages/edit-harness-result/node-b86fe92de458ef31.json
```

The submitted result path named by the request is absent.

### What prompt/model/provider/route was used?

**Answer from files:** several model/route layers are visible.

Campaign/profile protocol model:

```text
google/gemini-3.5-flash
route_source/direct-google
provider/google in the TOML profile sentinel field
```

Parent-side pre-child planning review:

```text
planner_route = direct-google
planner_model_id = google/gemini-2.5-flash-lite
status = completed
```

Headless TUI agent turn:

```text
selected_model = google/gemini-2.5-pro
```

Prompt text was persisted in two places:

```text
messages/edit-harness-request/node-b86fe92de458ef31.md
messages/edit-harness-result/node-b86fe92de458ef31.turn-live/agent-turn-summary.json.issue_prompt
```

The summary prompt says to modify the candidate checkout, stay outside protected core, use the direct-Google planner review, inspect the pre-child-planning JSON, and restrict edits to the 24 nearest code items seeded from `crates/ploke-tui/src/tools/mod.rs`.

### Did the headless TUI start successfully?

**Answer from files:** it started enough to create diagnostics and turn-live files, but did not record any attempts/events.

Evidence:

```json
headless diagnostics:
{
  "attempts": [],
  "terminal": { "terminal": "timed_out", "secs": 300 },
  "events": [],
  "validations": []
}
```

`agent-turn-summary.json` and `agent-turn-trace.json` exist, but each has `events=[]`, no final assistant message, and no terminal record.

### Did it call the provider?

**File-only answer:** no provider response was persisted for the headless-TUI agent turn.

Evidence:

- `llm-full-responses.jsonl` in the turn-live directory is `0` bytes and has `0` lines.
- `agent-turn-summary.json` has `events=[]` and `final_assistant_message=null`.

Separate note: the parent-side pre-child planning review did complete and contains a structured model response, but that was prior guidance, not the submitted child evidence.

### Did it execute tools?

**Answer from files:** no tool execution was persisted in the turn-live trace.

Evidence:

```text
agent-turn-summary.json events = []
agent-turn-trace.json events = []
headless diagnostics events = []
headless diagnostics validations = []
```

### Did it timeout, crash, or submit a result?

**Answer from files:** timeout, no submitted result file.

Evidence:

```json
"terminal": { "terminal": "timed_out", "secs": 300 }
```

The request's `submitted_result_path` points to `messages/edit-harness-result/node-b86fe92de458ef31.json`, but that file is absent.

### What workspace did it mutate?

**Answer from files/git state:** candidate workspace:

```text
/home/brasides/.ploke-eval/campaigns/p1-runnerio-db-20260627-125536/prototype1/workspaces/edit-harness/node-b86fe92de458ef31
```

The candidate workspace is a git worktree on branch:

```text
prototype1-broad-broad-harness-request-node-b86fe92de458ef31
```

It was at HEAD:

```text
42ba40f726149553cadb3cbc4e45396a9e49aad1
```

### What changed paths were detected?

**Answer from persisted candidate workspace git state:** one uncommitted modified file existed:

```text
M crates/ploke-tree-egui/src/main.rs
```

Diff summary:

```text
crates/ploke-tree-egui/src/main.rs | 6 ++++--
1 file changed, 4 insertions(+), 2 deletions(-)
```

The diff changed `build_tree_groups` from a `BTreeMap` to `std::collections::HashMap` and sorted collected groups by `first_index` before returning.

Important nuance: the harness result/agent summary had no submitted result and `patch_artifact.expected_file_changes=[]`, `applied=false`, and `any_expected_file_changed=false`. So the workspace contains a raw uncommitted diff, but the parent did not receive an admitted submitted result for it.

### Were changes rejected because they were empty, protected, malformed, or unbound?

**Answer from files:** the persisted rejection reason is timeout, not empty/protected/malformed/unbound.

Child-plan rejected attempt reason:

```text
broad headless-tui slot 'broad-harness-request:node-b86fe92de458ef31' timed out after 300 seconds
```

Headless diagnostics contain no validations and no protected-write denial. The uncommitted changed file is outside `crates/ploke-eval`, but no file-only admission validation was recorded because the attempt timed out without a submitted result.

### Was a rejected child-plan persisted before returning the below-minimum error?

**Answer from files:** yes.

`messages/child-plan/node-b86fe92de458ef31.json` contains:

```text
children = []
rejected_surface_attempts length = 1
producer_id = prototype1:broad-headless-tui-adapter-v1
outcome.kind = rejected
```

### Does the normalized DB row show rejected attempt reason and producer id?

**File-only equivalent:** the persisted child-plan file shows the rejected attempt reason and producer id.

```text
producer_id = prototype1:broad-headless-tui-adapter-v1
proposal_id = broad-harness-request:node-b86fe92de458ef31
policy = workspace_except_ploke_eval
target_relpath = .
outcome.kind = rejected
outcome.reason = timed out after 300 seconds
```

## Candidate/admission details

### How many candidate attempts were made?

**File-only answer:** one logical rejected surface attempt is recorded in the child-plan file, but the headless-TUI diagnostics record zero retained internal attempts.

```text
child-plan rejected_surface_attempts length = 1
headless diagnostics attempts = []
```

This means the parent persisted one rejected planning attempt for the request slot, while the TUI diagnostics did not retain any completed/submitted attempt records.

### For each attempt: target path, workspace root, candidate id, branch id, status?

**Answer from files:**

Rejected planning attempt:

```text
proposal_id = broad-harness-request:node-b86fe92de458ef31
run_id/request_hash = c6c8776ef84c070e4ccf22689a3168c4c93f99005a721f15d649c62145dc85ae
target_relpath = .
policy = workspace_except_ploke_eval
outcome = rejected
reason = timeout after 300 seconds
```

Workspace/branch from request/git state:

```text
candidate_workspace_path = .../prototype1/workspaces/edit-harness/node-b86fe92de458ef31
candidate workspace branch = prototype1-broad-broad-harness-request-node-b86fe92de458ef31
candidate workspace HEAD = 42ba40f726149553cadb3cbc4e45396a9e49aad1
```

No admitted child candidate id or child branch id exists in the child-plan file because `children=[]`.

### Why exactly was each rejected?

**Answer from files:** timeout after 300 seconds.

### Did protected-write enforcement deny anything?

**File-only answer:** no protected-write denial is persisted.

Evidence:

- headless diagnostics `validations=[]`;
- headless diagnostics `events=[]`;
- agent summary `events=[]`;
- rejection reason is timeout.

### Did “no admitted changes” mean no file diff, no accepted tool edit, or no committed candidate branch?

**Answer for this run:** this run was not a “no admitted changes” rejection; it was a timeout.

Files show there was an uncommitted candidate workspace diff:

```text
M crates/ploke-tree-egui/src/main.rs
```

But there was no submitted result file and no admitted child-plan child entry. So for this run, the persisted-file interpretation is: a workspace diff existed, but no result was submitted/admitted before timeout.

### If timeout: last observed tool/model event before timeout?

**Answer from files:** no tool/model event was persisted before timeout.

Evidence:

```text
headless diagnostics events = []
agent-turn-summary events = []
agent-turn-trace events = []
llm-full-responses.jsonl = 0 lines / 0 bytes
```

Visible timing from file data:

```text
request file mtime:        2026-06-27 06:08:00 -0700
pre-child planning:        2026-06-27T13:08:01.593807504+00:00
turn-live summary mtime:   2026-06-27 06:13:01 -0700
child-plan file mtime:     2026-06-27 06:13:02 -0700
node.json failed updated:  2026-06-27T13:13:02.075766090+00:00
```

The request-to-failure interval is about five minutes, matching the 300-second timeout.

## DB coverage questions answered from files only

### Do `eval_child_plan`, `eval_child_plan_child`, and `eval_child_plan_rejected_attempt` agree with the child-plan message?

**File-only answer:** the child-plan message itself is internally consistent; this section does not compare to DB rows.

File facts:

```text
parent_node_id = node-b86fe92de458ef31
child_generation = 1
children length = 0
rejected_surface_attempts length = 1
```

The rejected attempt is bound to the same request id/hash visible in the edit-harness request and pre-child-planning files.

### Do `eval_scheduler_node` and `eval_scheduler_node_status_event` show the parent/root node?

**File-only equivalent:** scheduler/node files show the root node, but with projection drift.

- `scheduler.json` shows one root node, status `planned`, frontier containing `node-b86fe92de458ef31`, and no completed/failed node ids.
- `nodes/node-b86fe92de458ef31/node.json` shows the same root node as status `failed` with updated time `2026-06-27T13:13:02.075766090+00:00`.

So files reveal the root node, but the mutable scheduler projection is stale relative to node-local projection.

### Were generation-1 child scheduler rows absent because no child was admitted?

**File-only answer:** yes.

Evidence:

- only one node directory exists under `prototype1/nodes/`: `node-b86fe92de458ef31`;
- child-plan file has `children=[]`;
- no child `node.json`, `runner-request.json`, worktree, invocation, or result directory exists for generation 1.

### Does `eval_runner_request` show root request rows?

**File-only equivalent:** `nodes/node-b86fe92de458ef31/runner-request.json` exists and records the root request.

Key fields:

```text
node_id = node-b86fe92de458ef31
generation = 0
branch_id = prototype1-parent-p1-runnerio-db-20260627-125536-gen0
workspace_root = /home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536
runner_args = ["loop", "prototype1-state", "--repo-root", "/home/brasides/.ploke-eval/worktrees/p1-runnerio-db-20260627-125536"]
```

### Is `eval_runner_result` absent because no child was spawned?

**File-only answer:** no runner result exists, and no child spawn files exist.

Absent paths:

```text
nodes/node-b86fe92de458ef31/runner-result.json
nodes/node-b86fe92de458ef31/invocations/
nodes/node-b86fe92de458ef31/results/
nodes/node-b86fe92de458ef31/channels/
```

Because `children=[]`, there was no admitted child to materialize/spawn.

### Are invalid schema rows zero?

**File-only equivalent:** not a row question. The inspected persisted files have current expected file schemas where applicable:

```text
campaign.json schema_version = campaign-manifest.v1
run-profile.commitment.json schema_version = prototype1-run-profile-commitment.v1
scheduler.json schema_version = prototype1-scheduler.v1
node.json schema_version = prototype1-treatment-node.v1
runner-request.json schema_version = prototype1-treatment-node.v1
parent_identity.json schema_version = prototype1-parent-identity.v1
```

The child-plan file does not have a top-level schema_version field; its rejected attempt has `schema_version=1`.

### Is the owner DB fresh/current-schema, not old contaminated?

**File-only answer:** not answerable without inspecting the DB.

Files show current file schemas and a fresh campaign id, but freshness/current-schema of the owner DB is inherently a database question.

### Are claims relying only on `eval_record_ref` instead of normalized rows?

**File-only answer:** not applicable. This section does not use DB rows or `eval_record_ref`; it uses persisted JSON/TOML/JSONL/git files directly.

## Low-level runtime/tool trace

### Was there an `eval_agent_turn`?

**File-only equivalent:** yes, a turn-live summary/trace directory exists for the broad-harness request.

Files:

```text
messages/edit-harness-result/node-b86fe92de458ef31.turn-live/agent-turn-summary.json
messages/edit-harness-result/node-b86fe92de458ef31.turn-live/agent-turn-trace.json
messages/edit-harness-result/node-b86fe92de458ef31.turn-live/llm-full-responses.jsonl
```

Key fields:

```text
task_id = broad-harness-request:node-b86fe92de458ef31
selected_model = google/gemini-2.5-pro
events length = 0
final_assistant_message = null
```

### Were there `eval_tool_event` rows?

**File-only equivalent:** no tool events were persisted.

Evidence:

```text
agent-turn-summary.json events = []
agent-turn-trace.json events = []
headless diagnostics events = []
```

### Were `eval_model_exchange`, `eval_message_event`, and `eval_trace_event` absent?

**File-only equivalent:** no persisted model response/message event records were present in the turn-live files.

Evidence:

```text
llm-full-responses.jsonl = 0 lines, 0 bytes
agent-turn-summary final_assistant_message = null
agent-turn-summary events = []
```

### If absent, is that because the path bypasses those writers, or because the run failed before they fired?

**File-only answer:** not distinguishable with certainty.

The persisted files show timeout and no events/responses. That supports “failed before any persisted event/response,” but it does not prove whether a writer path was bypassed versus no provider/tool event occurring before timeout.

### Which logs/checkpoints correspond to the rejected attempt?

**Answer from files:**

```text
messages/edit-harness-result/node-b86fe92de458ef31.headless-tui.json
messages/edit-harness-result/node-b86fe92de458ef31.turn-live/agent-turn-summary.json
messages/edit-harness-result/node-b86fe92de458ef31.turn-live/agent-turn-trace.json
messages/edit-harness-result/node-b86fe92de458ef31.turn-live/llm-full-responses.jsonl
messages/pre-child-planning/node-b86fe92de458ef31.json
messages/pre-child-planning/node-b86fe92de458ef31.prompt.md
messages/edit-harness-request/node-b86fe92de458ef31.json
messages/edit-harness-request/node-b86fe92de458ef31.md
```

### What elapsed time happened between request publication, first model/tool event, last event, and timeout?

**File-only answer:** there were no first/last model/tool events. The visible timing is request publication to timeout.

Approximate persisted-file timeline:

```text
profile admitted:            2026-06-27T12:56:28.340553384+00:00
parent identity created:     2026-06-27T12:56:29.561743+00:00
parent_started journal ms:   1782565011059
closure eval complete:       2026-06-27T13:05:04.286+00:00 transition marker; closure updated 2026-06-27T13:07:55.595054182+00:00
request file mtime:          2026-06-27 06:08:00 -0700
pre-child planning generated:2026-06-27T13:08:01.593807504+00:00
turn-live files mtime:       2026-06-27 06:13:01 -0700
node failed updated:         2026-06-27T13:13:02.075766090+00:00
child-plan file mtime:       2026-06-27 06:13:02 -0700
```

Request-to-failure is about five minutes, consistent with `timeout_secs=300`.

## What did not happen because of the blocker?

### No `R8` selectable parent state.

**File-only answer:** no successful runnable child-plan exists.

`messages/child-plan/node-b86fe92de458ef31.json` has:

```text
children = []
rejected_surface_attempts length = 1
```

A child-plan file exists, but it is a rejected zero-child plan rather than a runnable child authority set.

### No `R9` child schedule shaping.

**File-only answer:** no child schedule artifacts exist beyond the zero-child plan.

There are no child entries in the child-plan file and no generation-1 node directories.

### No `R10` selection strategy over runnable children.

**File-only answer:** no selection artifacts exist.

There is no `evaluations/` directory, no selection decision file found in the inspected prototype1 tree, and scheduler `selected_successor` is `null`.

### No `R11` child fanout.

**File-only answer:** no child fanout files exist.

Absent:

```text
invocations/
channels/
results/
runner-result.json
child worktree under nodes/<child>/worktree
child bin/target under nodes/<child>/
```

### No materialize/build/spawn/observe.

**File-only answer:** yes. The node-local runtime/build/observe directories are absent, and no child node was admitted.

### No child invocation/channel/result.

**File-only answer:** yes. There are no invocation, channel, result, or runner-result files for a child.

### No runner result proof.

**File-only answer:** yes. `nodes/node-b86fe92de458ef31/runner-result.json` is absent, and no child node directories exist.

### No selection/handoff/History seal.

**File-only answer:** yes.

Absent:

```text
prototype1/evaluations/
prototype1/history/
prototype1/nodes/node-b86fe92de458ef31/successor-ready/
prototype1/nodes/node-b86fe92de458ef31/successor-completion/
```

Scheduler projection has `selected_successor=null`.

## File-only summary and differences from DB-only inspection

File-only inspection reaches the same main conclusion as DB-only inspection:

- generation-0 parent started;
- baseline eval closure completed;
- broad-harness child planning was attempted;
- the attempt timed out after 300 seconds;
- a rejected zero-child child-plan was persisted;
- no child was admitted/spawned/evaluated;
- no selection, continuation, handoff, or History seal occurred.

Additional facts only visible from files in this inspection:

- The admitted profile exposes the missing DB-normalized search facts: `max_generations=1`, `max_total_nodes=4`, child min/max `1/1`, full-batch scheduling, and broad TUI timeout `300s`.
- The edit-harness request records candidate workspace path, graph restriction, protected-core policy, target artifact commit, and request hash.
- The parent-side pre-child-planning review completed using `direct-google` / `google/gemini-2.5-flash-lite` before the headless-TUI attempt.
- The headless-TUI turn selected `google/gemini-2.5-pro` but persisted no events and no provider responses.
- The candidate workspace contains an uncommitted diff in `crates/ploke-tree-egui/src/main.rs`, even though no submitted result file exists and no child was admitted.
- `scheduler.json` is stale relative to node-local `node.json`: scheduler still says root node `planned`, while node-local projection says `failed`.

Those differences clarify the DB roadmap: normalized DB persistence should add search/budget policy, harness request/result/workspace/change facts, low-level provider/tool/message trace facts, and projection consistency/historical status facts so DB-only inspection can answer the same questions without falling back to files.
