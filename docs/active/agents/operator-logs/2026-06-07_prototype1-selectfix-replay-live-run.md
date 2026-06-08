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

## Related Reports

- [`../2026-06-07_prototype1-loop-termination-reports/README.md`](../2026-06-07_prototype1-loop-termination-reports/README.md)
  collects termination reports for earlier loop campaigns.
