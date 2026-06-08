# Prototype 1 Setup And Pre-Child Failures

Read-only status report for:

- `p1-rejected-handoff-5g1x2-a2-20260607-191057`
- `p1-handofffix-5g1x2-a2-20260607-190702`

Inspection used `ps`, `jq`, `find`, `stat`, `wc`, `git status`, and the read-only
Prototype 1 run-summary helper. No campaign, instance, protocol, worktree,
database, or README artifacts were modified. SQLite/DB files were treated only as
path witnesses.

## Summary

| Campaign | Verdict | Clean? | Last reached timing surface | Ending evidence |
| --- | --- | --- | --- | --- |
| `p1-rejected-handoff-5g1x2-a2-20260607-191057` | `incomplete evidence` | Not clean; no durable terminal record was found. | Parent state-machine reached `parent_start` resource at `recorded_at=1780860314146` (`2026-06-07 12:25:14.146 -0700`); baseline run registry later reached `patching.status=in_progress` at `2026-06-07T19:25:16.029484926+00:00`, with `agent-turn-trace.json` mtime `2026-06-07 12:25:33.529849024 -0700`. | No clean ending artifact exists in the inspected surfaces: `closure-state.json` has `eval.status="missing"`, run registry `run-1780860315186-structured-current-policy-5d2d63ca.json` remains `execution_status="running"` / `patching.status="in_progress"`, the trace has `terminal_record=null`, and promised terminal artifacts are absent. |
| `p1-handofffix-5g1x2-a2-20260607-190702` | `failed` | Not a clean successful termination; the failure is durably persisted. | Parent state-machine reached `parent_start` resource at `recorded_at=1780884707240` (`2026-06-07 19:11:47.240 -0700`); baseline eval failure was recorded at `2026-06-08T02:11:47.345786512+00:00`. | `batch-run-summary.json` and `closure-state.json` record `instances_failed=1` / `eval_status="failed"` with an `embedding_model_preflight` failure for `mistralai/codestral-embed-2505` caused by a missing environment variable. |

Host process probe:

- Command shape: `ps -ef | rg '[p]loke-eval loop|[p]rototype1-state|[p]rototype1-step|[p]rototype1-continue|[p]rototype1-runner'`.
- No process matched either target campaign.
- The only live Prototype 1 process matched a different campaign:
  `p1-selectfix-replay-5g1x2-a2-20260607-224502`.

## `p1-rejected-handoff-5g1x2-a2-20260607-191057`

Roots inspected:

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-rejected-handoff-5g1x2-a2-20260607-191057`
- Instance root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-rejected-handoff-5g1x2-a2-20260607-191057`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-rejected-handoff-5g1x2-a2-20260607-191057`
  was absent.
- Worktree path advertised by node/runner records:
  `/home/brasides/.ploke-eval/worktrees/p1-rejected-handoff-5g1x2-a2-20260607-191057`
  was absent at inspection time.

Transition-journal tail and timing:

```json
{"kind":"parent_started","recorded_at":1780860314124,"campaign_id":"p1-rejected-handoff-5g1x2-a2-20260607-191057"}
{"kind":"resource","recorded_at":1780860314146,"campaign_id":"p1-rejected-handoff-5g1x2-a2-20260607-191057","parent_id":"node-55d98560307267b1","node_id":"node-55d98560307267b1","generation":0,"phase":"parent_start","subject":"cargo_target","status":"measured","path":"/home/brasides/.ploke-eval/worktrees/p1-rejected-handoff-5g1x2-a2-20260607-191057/target"}
```

- Journal file mtime: `2026-06-07 12:25:14.145703938 -0700`.
- Last durable state-machine phase: `parent_start`.
- No child-plan, broad-generation, observe-child, successor-selection, terminal
  error, or handoff transition was present.

Baseline/run timing evidence:

- `closure-state.json` mtime: `2026-06-07 12:25:14.230704574 -0700`;
  `updated_at="2026-06-07T19:25:14.231450574+00:00"`.
- `closure-state.json` registry status was `complete`, but eval status was
  `missing` with `expected_total=1`, `complete_total=0`, `missing_total=1`.
- Run registry:
  `/home/brasides/.ploke-eval/registries/runs/run-1780860315186-structured-current-policy-5d2d63ca.json`
  has:
  - `created_at="2026-06-07T19:25:15.186499580+00:00"`
  - `started_at="2026-06-07T19:25:15.187006512+00:00"`
  - `setup.status="completed"` at `2026-06-07T19:25:16.029482701+00:00`
  - `patching.status="in_progress"` at
    `2026-06-07T19:25:16.029484926+00:00`
  - `execution_status="running"`
  - `submission_status="missing"`
- `agent-turn-trace.json` mtime:
  `2026-06-07 12:25:33.529849024 -0700`; it had `event_count=24`,
  `selected_model="google/gemini-3.5-flash"`, `terminal_record=null`,
  empty final assistant message, and no applied patch.
- Last trace events were still ordinary assistant/tool events. The final visible
  assistant event was: `I will search for context about replacement and
  multi-line matching within the codebase using request_code_context.`

Broad request/result mtimes:

- No `prototype1/messages` directory existed.
- No `edit-harness-request`, `edit-harness-result`, `.headless-tui.json`,
  `.turn-live`, or `llm-full-responses.jsonl` files were found under the
  campaign or instance roots.

Node invocation/result/successor records:

- Present node record:
  `/prototype1/nodes/node-55d98560307267b1/node.json`
  with `status="planned"`, `generation=0`, and branch
  `prototype1-parent-p1-rejected-handoff-5g1x2-a2-20260607-191057-gen0`.
- Present runner request:
  `/prototype1/nodes/node-55d98560307267b1/runner-request.json`.
- Promised runner result path:
  `/prototype1/nodes/node-55d98560307267b1/runner-result.json`
  was absent.
- No `invocations`, `results`, `successor`, or `streams` paths were present.
- `scheduler.json` kept `node-55d98560307267b1` in `frontier_node_ids` and had
  empty `completed_node_ids` and `failed_node_ids`.

Stream stderr/stdout terminal evidence:

- No `stderr`, `stdout`, `*.log`, or campaign-owned `streams` files were found
  under the campaign or instance roots.
- The advertised worktree path was absent, so no worktree-side parent identity
  or stream logs were available.

Final verdict: `incomplete evidence`.

This was not clean. The exact persisted evidence against a clean ending is the
combination of:

- `closure-state.json`: eval remains `missing`.
- Run registry
  `run-1780860315186-structured-current-policy-5d2d63ca.json`: lifecycle remains
  `execution_status="running"` with `patching.status="in_progress"`.
- `agent-turn-trace.json`: `terminal_record=null`.
- Absent terminal surfaces: `execution-log.json`, `agent-turn-summary.json`,
  `llm-full-responses.jsonl`, `multi-swe-bench-submission.jsonl`,
  `benchmark-patch-projection.json`, `validation-audit.json`, `record.json.gz`,
  and `final-snapshot.db` were absent from the run root.

There is no durable artifact proving a complete clean ending or a clean recorded
failure for this campaign.

## `p1-handofffix-5g1x2-a2-20260607-190702`

Roots inspected:

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-handofffix-5g1x2-a2-20260607-190702`
- Instance root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-handofffix-5g1x2-a2-20260607-190702`
- Batch root:
  `/home/brasides/.ploke-eval/batches/prototype1/p1-handofffix-5g1x2-a2-20260607-190702/ripgrep-burntsushi-ripgrep-2209-eval-slice-20260608021147`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-handofffix-5g1x2-a2-20260607-190702`
  was absent.
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-handofffix-5g1x2-a2-20260607-190702`
  existed and was on branch
  `prototype1-parent-p1-handofffix-5g1x2-a2-20260607-190702-gen0` with clean
  `git status --short --branch` output.

Transition-journal tail and timing:

```json
{"kind":"parent_started","recorded_at":1780884707219,"campaign_id":"p1-handofffix-5g1x2-a2-20260607-190702"}
{"kind":"resource","recorded_at":1780884707240,"campaign_id":"p1-handofffix-5g1x2-a2-20260607-190702","parent_id":"node-2e58ea711dd2446e","node_id":"node-2e58ea711dd2446e","generation":0,"phase":"parent_start","subject":"cargo_target","status":"measured","path":"/home/brasides/.ploke-eval/worktrees/p1-handofffix-5g1x2-a2-20260607-190702/target"}
```

- Journal file mtime: `2026-06-07 19:11:47.239785774 -0700`.
- Last durable state-machine phase: `parent_start`.
- No child-plan, broad-generation, observe-child, successor-selection, terminal
  error, or handoff transition was present.

Baseline/run timing evidence:

- Parent identity:
  `/home/brasides/.ploke-eval/worktrees/p1-handofffix-5g1x2-a2-20260607-190702/.ploke/prototype1/parent_identity.json`
  mtime `2026-06-07 19:08:11.676832075 -0700`, with
  `created_at="2026-06-08T02:08:11.677463660+00:00"`.
- Batch manifest mtime:
  `2026-06-07 19:11:47.331786414 -0700`.
- Empty `multi-swe-bench-submission.jsonl` mtime:
  `2026-06-07 19:11:47.338786463 -0700`, size `0`.
- Batch failure summary mtime:
  `2026-06-07 19:11:47.345786512 -0700`.
- `closure-state.json` mtime:
  `2026-06-07 19:11:47.533787820 -0700`;
  `updated_at="2026-06-08T02:11:47.534620683+00:00"`.
- `closure-state.json` eval status was `partial`, with `expected_total=1`,
  `failed_total=1`, and
  `last_transition_at="2026-06-08T02:11:47.345786512+00:00"`.

Durable failure text:

```text
database setup failed during 'embedding_model_preflight': embedding preflight failed for 'mistralai/codestral-embed-2505': Var error: Error from env variable, original: environment variable not found.
```

The same failure appears in both:

- `/home/brasides/.ploke-eval/batches/prototype1/p1-handofffix-5g1x2-a2-20260607-190702/ripgrep-burntsushi-ripgrep-2209-eval-slice-20260608021147/batch-run-summary.json`
- `/home/brasides/.ploke-eval/campaigns/p1-handofffix-5g1x2-a2-20260607-190702/closure-state.json`

Broad request/result mtimes:

- No `prototype1/messages` directory existed.
- No `edit-harness-request`, `edit-harness-result`, `.headless-tui.json`,
  `.turn-live`, or `llm-full-responses.jsonl` files were found under the
  campaign, instance, or batch roots.

Node invocation/result/successor records:

- Present node record:
  `/prototype1/nodes/node-2e58ea711dd2446e/node.json`
  with `status="planned"`, `generation=0`, and branch
  `prototype1-parent-p1-handofffix-5g1x2-a2-20260607-190702-gen0`.
- Present runner request:
  `/prototype1/nodes/node-2e58ea711dd2446e/runner-request.json`.
- Promised runner result path:
  `/prototype1/nodes/node-2e58ea711dd2446e/runner-result.json`
  was absent.
- No `invocations`, `results`, `successor`, or `streams` paths were present.
- `scheduler.json` kept `node-2e58ea711dd2446e` in `frontier_node_ids` and had
  empty `completed_node_ids` and `failed_node_ids`.

Stream stderr/stdout terminal evidence:

- No campaign-owned `stderr`, `stdout`, or `streams` files were found.
- The only `*.log` files found in the worktree scan were unrelated checkout
  fixture logs under `tests/fixture_chat/`; they are not Prototype 1 terminal
  evidence.

Final verdict: `failed`.

This campaign terminated before broad child generation because baseline
database setup failed at `embedding_model_preflight`. The exact artifacts proving
the failed ending are `batch-run-summary.json` with `instances_failed=1` and
`closure-state.json` with `eval_status="failed"` for
`BurntSushi__ripgrep-2209`.
