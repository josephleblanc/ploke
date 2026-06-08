# 2026-06-07 Prototype 1 Selectfix Replay Live Run

Status: active operator log for the current live Prototype 1 loop campaign.

Campaign:
`p1-selectfix-replay-5g1x2-a2-20260607-224502`

Worktree:
`/home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502`

Related commits:

- `4e97c75d Fix Prototype 1 traversal replay selection`
- `5d09b4e7 docs: add Prototype 1 termination reports`

## Current Profile

Observed from
`/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/run-profile.toml`
and the run-summary helper:

- `max_generations = 5`
- `parallel_targets = 2`
- `max_attempts = 2`
- `fresh_slots_per_child = 2`
- `require_keep_for_continuation = false`
- `explore_from_rejected = true`
- parent model `google/gemini-3.5-flash`, direct Google route
- protocol model `google/gemini-2.5-flash`, direct Google route, `max_tokens=8000`

## Operator Notes

2026-06-07 23:43:18 -0700:

- The run successfully handed off from the gen-1 kept parent
  `node-10b418a02e91f3f1` to historical rejected parent
  `node-f21de5ba2e927ab0`.
- Durable evidence:
  `prototype1/transition-journal.jsonl` has `successor_handoff`,
  `parent_started`, `resource parent_start` for `node-f21de5ba2e927ab0`, and
  `resource parent_complete` / `successor completed status=succeeded` for
  `node-10b418a02e91f3f1`.
- Live process evidence:
  PID `1489538` is running `ploke-eval loop prototype1-state` with
  `--handoff-invocation` pointing at
  `prototype1/nodes/node-f21de5ba2e927ab0/invocations/f5fc594b-dc27-4e2d-ba87-e8747fe249aa.json`.

2026-06-07 23:48:36 -0700:

- The first broad attempt for rejected parent `node-f21de5ba2e927ab0`
  committed workspace result `10c9de99`.
- Result artifacts:
  `prototype1/messages/edit-harness-result/node-f21de5ba2e927ab0.json` and
  `node-f21de5ba2e927ab0.headless-tui.json`.
- Evidence caveat: `node-f21de5ba2e927ab0.turn-live/llm-full-responses.jsonl`
  was present at zero bytes. The attempt still produced a result JSON, trace,
  summary, and workspace commit.

2026-06-07 23:49:41 -0700:

- The `node-f21de5ba2e927ab0-r2` broad attempt had not produced a result
  sidecar yet.
- The `-r2` workspace had recent cargo build artifacts through about
  `23:48:04 -0700` and no uncommitted diff.
- The active parent process was still live. Continue read-only monitoring; do
  not run `prototype1-state`, `prototype1-step`, or `prototype1-continue`
  against this campaign while PID `1489538` is active.

2026-06-07 23:55:28 -0700:

- The `node-f21de5ba2e927ab0-r2` broad attempt produced result sidecars:
  `node-f21de5ba2e927ab0-r2.json` and
  `node-f21de5ba2e927ab0-r2.headless-tui.json`.
- The attempt had modified
  `crates/ploke-tui/src/rag/utils.rs`,
  `crates/ploke-tui/src/tools/code_item_lookup.rs`, and
  `crates/ploke-tui/src/tools/get_code_edges.rs` in the candidate workspace.
- The stream stdout recorded another `INVALID_MODEL_RESPONSE` warning at
  `23:55:28`, but the result sidecars were still written.

2026-06-07 23:55:31 to 23:55:36 -0700:

- The loop wrote child plan
  `prototype1/messages/child-plan/node-f21de5ba2e927ab0.json`.
- It materialized two gen-2 child nodes from the rejected parent:
  `node-e0215ac6e2825a28` / `branch-01ade840a0a918b6` and
  `node-655e9397c9273545` / `branch-e304e47deb78395e`.
- Both nodes reached `status=workspace_staged`, and the transition journal has
  `build_child before` records for both.
- This is the first fresh evidence in this run that continuation from a
  historical rejected parent advanced through broad generation into child
  materialization/build.

2026-06-08 00:09:57 -0700:

- Both children materialized from the historical rejected parent
  `node-f21de5ba2e927ab0` completed treatment self-eval:
  `node-655e9397c9273545` wrote result
  `results/ae02174b-adc0-4768-91a4-2d3e47d85a41.json` at
  `00:03:54 -0700`, and `node-e0215ac6e2825a28` wrote result
  `results/73f6dc16-de0d-4a05-b7e5-fdab442ee3d5.json` at
  `00:09:38 -0700`.
- The successor selector chose `node-e0215ac6e2825a28`; the transition journal
  then recorded `successor_handoff`, `parent_started`, and
  `resource parent_start` for generation 2 at `00:09:57 -0700`.
- Live process evidence after handoff: PID `1585253` is running
  `ploke-eval loop prototype1-state` for the same campaign with
  `--handoff-invocation` under
  `prototype1/nodes/node-e0215ac6e2825a28/invocations/a720628b-478f-45df-8838-9e76dd838388.json`.
- This proves the current `prototype1-state` path has crossed parent to
  successor handoff twice in this run, including a successor selected from a
  rejected-parent continuation path.

2026-06-08 00:18:37 to 00:21:20 -0700:

- Generation 2 parent `node-e0215ac6e2825a28` completed two broad attempts:
  `node-e0215ac6e2825a28-r2` wrote result sidecars at `00:18:37 -0700`, and
  `node-e0215ac6e2825a28` wrote result sidecars at `00:18:46 -0700`.
- The loop wrote child plan
  `prototype1/messages/child-plan/node-e0215ac6e2825a28.json` at
  `00:18:49 -0700`.
- It materialized and built two generation 3 children:
  `node-666e03c70c944722` / `branch-c17c4cda5764dbb7` and
  `node-d245f3418c712226` / `branch-0e230b45dab588be`.
- The transition journal recorded `build_child after`, `child_artifact_committed`,
  `spawn_child spawned`, `child ready`, `child evaluating`, and
  `observe_child before` for both children by `00:21:20 -0700`.
- Live process evidence at `00:22:17 -0700`: PID `1585253` remained the active
  parent, with runner PIDs `1667583` and `1667637` evaluating the two
  generation 3 children.

2026-06-08 00:33:20 -0700:

- Both generation 3 children from parent `node-e0215ac6e2825a28` completed
  treatment self-eval:
  `node-666e03c70c944722` / `branch-c17c4cda5764dbb7` wrote a `keep`
  evaluation at `00:27:30 -0700`, and `node-d245f3418c712226` /
  `branch-0e230b45dab588be` wrote a `keep` evaluation at `00:32:59 -0700`.
- The sealed successor-selection row sampled historical rejected
  `node-7d41954c057f3002` / `branch-a21b6ff8dee21a03` under
  `continue_explore_from_rejected`, even though the two fresh generation 3
  candidates were both `keep`.
- The transition journal recorded checkout of installed commit
  `e931bd482782248fc86db59d81bc9a115752a882`, `successor_handoff`,
  `parent_started`, `resource parent_start` for `node-7d41954c057f3002`, and
  `resource parent_complete` / `successor completed status=succeeded` for
  `node-e0215ac6e2825a28`.
- Live process evidence after the handoff: PID `1676219` is running
  `ploke-eval loop prototype1-state` with `--handoff-invocation` under
  `prototype1/nodes/node-7d41954c057f3002/invocations/910f2e3e-a907-4667-aec4-3b1efaa99dfd.json`.
- This is the third successful parent-to-successor handoff in this run, and it
  shows the current state path can continue through selection from the
  historical rejected frontier after fresh kept children have completed.

2026-06-08 00:39:38 to 00:43:50 -0700:

- Historical rejected parent `node-7d41954c057f3002` completed both broad
  attempts: `node-7d41954c057f3002` wrote result sidecars at
  `00:39:38 -0700`, and `node-7d41954c057f3002-r2` wrote result sidecars at
  `00:41:20 -0700`.
- The loop wrote child plan
  `prototype1/messages/child-plan/node-7d41954c057f3002.json` at
  `00:41:23 -0700`.
- It materialized and spawned two generation 3 child runners:
  `node-86b020f1dfdcf8af` / `branch-564844dfd9b65469` and
  `node-44b3f97965446494` / `branch-e21d8f841040888f`; both target
  `crates/ploke-protocol/src/tool_calls/review.rs`.
- Live process evidence at `00:44:11 -0700`: parent PID `1676219` remained
  active, with runner PIDs `1746105` and `1746106` evaluating the two child
  branches.

2026-06-08 00:57:16 -0700:

- Both generation 3 children from historical rejected parent
  `node-7d41954c057f3002` completed treatment self-eval:
  `node-44b3f97965446494` / `branch-e21d8f841040888f` wrote a `keep`
  evaluation at `00:54:36 -0700`, and `node-86b020f1dfdcf8af` /
  `branch-564844dfd9b65469` wrote a `keep` evaluation at `00:57:01 -0700`.
- The successor selector chose kept child `node-86b020f1dfdcf8af` with
  `disposition=continue_ready`; the sealed row lists performance `19120`,
  `child_count=2`, and selected weight `0.249817374`.
- The transition journal recorded checkout of installed commit
  `a7272f6706c85d1a4b1b3e0b37c70fbb586b4424`, `successor_handoff`,
  `parent_started`, `resource parent_start` for `node-86b020f1dfdcf8af`, and
  `resource parent_complete` / `successor completed status=succeeded` for
  `node-7d41954c057f3002`.
- Live process evidence after the handoff: PID `1753984` is running
  `ploke-eval loop prototype1-state` with `--handoff-invocation` under
  `prototype1/nodes/node-86b020f1dfdcf8af/invocations/f17b3fb3-3701-4725-b7e3-855df00143e2.json`.
- This is the fourth successful parent-to-successor handoff observed in this
  live run after disabling the keep gate, and it follows a historical rejected
  parent that produced fresh kept children.

## Related Reports

- [`../2026-06-07_prototype1-loop-termination-reports/README.md`](../2026-06-07_prototype1-loop-termination-reports/README.md)
  collects termination reports for earlier loop campaigns.
