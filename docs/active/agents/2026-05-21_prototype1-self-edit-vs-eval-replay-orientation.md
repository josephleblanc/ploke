# 2026-05-21 Prototype 1 Self-Edit vs Eval Replay Orientation

Status: current orientation note for replay-loop use-testing and the next
runtime-playback implementation pass.

Use this when discussing "self-edit patch", "eval patch", "target workspace",
or `ploke-eval run replay turn-live`.

## Verified Surface

This note is grounded in:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:40`
  The Prototype 1 model treats every checkout as an Artifact, every Artifact as
  a dehydrated Runtime, and a Runtime as able to operate over an Artifact that
  may be different from the Artifact that hydrated the Runtime.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:1167`
  The broad self-edit harness prompt asks the agent to modify the candidate
  checkout while respecting protected-core constraints.
- `crates/ploke-eval/src/runner.rs:1253`
  The benchmark/eval prompt asks the agent to solve a benchmark issue in a
  repository root and produce patch output.
- `crates/ploke-eval/src/cli/prototype1_process.rs:2235`
  Child runner invocations load a workspace request, enter child evaluation,
  and call `run_prototype1_resolved_branch_treatment`.
- `crates/ploke-eval/src/cli/prototype1_process.rs:2386`
  Treatment self-evaluation materializes the candidate branch, prepares a
  treatment campaign, prepares a node-local instance target cache, and advances
  eval/protocol closure.
- `crates/ploke-eval/src/cli.rs:6428`
  `advance_eval_closure` prepares benchmark batches; when a repo-cache override
  is supplied, it ensures the target repo cache before running eval batches.
- `crates/ploke-eval/src/msb.rs:130`
  Multi-SWE-Bench prepared runs set `repo_root` to
  `repo_cache/<org>/<repo>`.
- `crates/ploke-eval/src/runner.rs:3985`
  Benchmark runs reset/check out the target repo to the base SHA before the
  turn.

## Two Patch Meanings

`self-edit patch`

This is the patch to the candidate `ploke` Artifact. It belongs to the
self-improvement side of Prototype 1: the Parent prepares candidate child
Artifacts, broad-harness prompts ask the agent to modify the candidate checkout,
and protected-core rules decide whether those edits are admissible. This is the
patch that may produce a successor `ploke` Runtime later.

`eval patch`

This is the patch produced inside a benchmark target repository such as
`BurntSushi/ripgrep`. It belongs to child self-evaluation. A candidate `ploke`
Runtime is evaluated by running benchmark tasks against external target repos.
The benchmark prompt says "Solve the following benchmark issue" and records
artifacts such as `agent-turn-trace.json`, `llm-full-responses.jsonl`,
`record.json.gz`, and `multi-swe-bench-submission.jsonl`.

Do not collapse these two. The self-edit patch mutates the candidate `ploke`
Artifact. The eval patch is evidence produced by that candidate while operating
over a benchmark target Artifact.

## Replay Command Meaning

The command:

```bash
target/debug/ploke-eval run replay turn-live \
  --run-dir <historical-run-dir> \
  --workspace <current-target-workspace> \
  --event-index <N> --through-event \
  --tail stop|live-step|live
```

is currently replaying the eval phase, not the self-edit phase.

- `--run-dir` supplies historical eval-run artifacts and the provider response
  sidecar.
- `--workspace` supplies the current benchmark target checkout whose files,
  search index, and tools will be used by the live TUI/tool loop.
- recorded provider responses are replayed into the current `ploke-tui`
  session path;
- tool calls are executed now against `--workspace`;
- `--tail live-step` takes one new provider response, executes its tools, writes
  a branch tape when requested, and stops at the next provider boundary.

This is useful for asking whether our current tools give the eval agent a fair
path through the benchmark target. It does not by itself replay the broad
self-edit patch production path over the candidate `ploke` checkout.

## Current Replay Target

Historical run currently being used for CLI grounding:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-smoke-broad-harness-1x3-20260519-1/treatments/branch-d68716f09d5982ae/instances/BurntSushi__ripgrep-2209/runs/run-1779193194784-structured-current-policy-966f1ec8
```

Important observed replay fact:

```text
historical prompt root:
/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/nodes/node-21eaf04f2c01c6f2/instance-targets/p1-smoke-broad-harness-1x3-20260519-1-treatment-branch-d68716f09d5982ae-1779193191294/BurntSushi/ripgrep

status from replay inspect:
missing, not-target
```

That means replay must be explicit about which current workspace replaces the
missing historical target.

Do not use this shared cache for clean replay unless intentionally inspecting
its dirty state:

```text
/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep
dirty file:
crates/printer/src/util.rs
```

Clean disposable replay workspace created for continued CLI use-testing:

```text
/home/brasides/.ploke-eval/replay-workspaces/ripgrep-2209-clean-20260521
HEAD:
4dc6c73c5a9203c5a8a89ce2161feca542329812
```

Use the clean disposable workspace for continued `turn-live` probing unless the
task is specifically to inspect the dirty shared cache.

## What This Means for Runtime Playback

Runtime playback needs to preserve the distinction between:

- the candidate `ploke` Artifact that produced the evaluating Runtime;
- the benchmark target Artifact that Runtime operated over;
- the eval patch emitted against that target;
- the self-edit patch that may later be admitted as a successor Artifact;
- the replay workspace used as an operator substitute when a historical target
  checkout has been cleaned up.

The CLI replay command is currently a useful operator probe, but it is a
projection over fragmented records plus an explicit workspace override. The
graph-backed `RuntimePlaybackRef<'g, G>` work should make those roles explicit
so CLI and egui views can share one cursor without treating workspace paths or
rendered CLI output as authority.
