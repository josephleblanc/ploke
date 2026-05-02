---
name: prototype1-loop-runtime
description: Use when setting up, building, running, or observing Prototype 1 trampoline loop campaigns, especially `prototype1-setup`, `prototype1-state`, parent worktrees, binary self-propagation, or Runtime/Artifact provenance. Ensures live loop execution uses the binary built from the active parent worktree and that Codex observes rather than advances the sandbox-sensitive loop.
---

# Prototype 1 Loop Runtime

Use this skill before giving or running commands for Prototype 1 loop setup,
execution, or observation.

## Core Rule

For live `prototype1-state`, the Runtime must come from the active parent
Artifact/worktree.

Do not run:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-state --repo-root <other-worktree>
```

That uses a binary from one checkout against another checkout's parent identity
and source state, which breaks Runtime/Artifact provenance.

## Command Classes

Classify the command before acting:

- `setup`: campaign/bootstrap setup. It admits the current checkout as
  `Parent(0)`, creates/checks out the fresh parent branch, writes and commits
  `.ploke/prototype1/parent_identity.json`, and validates the checkout. For
  now, the stable main binary may be used because setup-from-worktree still has
  record/path cleanup pending.
- `build`: must run in the active parent worktree. Codex should run this.
- `run`: live `prototype1-state`. Codex must not run this; the user runs it in
  a normal terminal from the active parent worktree.
- `observe`: read-only monitoring of persisted files/projections. Codex may run
  this from outside the live runtime.

## Setup Workflow

1. Create or reuse an experimental worktree.
2. Run `prototype1-setup` from that worktree with the intended campaign and
   policy. The default `--max-generations` is `1`; pass it explicitly for
   multi-generation runs.
3. If setup fails after creating campaign artifacts, prefer a new campaign
   suffix unless the user explicitly asks to clean up and retry the same id.
4. Verify the campaign slice if selection warns that the campaign does not
   include the selected instance:

```bash
jq -r '.instance_id? // .task_id? // empty' ~/.ploke-eval/campaigns/<campaign>/slice.jsonl
```

Five-generation setup shape:

```bash
cd <active-parent-worktree>
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-setup \
  --campaign <campaign> \
  --dataset-key <key> \
  --instance <instance> \
  --max-generations 5 \
  --max-total-nodes 32
```

Current search-policy knobs are `--max-generations`, `--max-total-nodes`,
`--stop-on-first-keep`, and `--require-keep-for-continuation true|false`.

## Build Workflow

After setup, Codex should build the Runtime from the active parent worktree:

```bash
cd <active-parent-worktree>
cargo build -p ploke-eval
```

Use the resulting worktree-local binary for the live loop command.

## Live Loop Handoff

Codex must not run `prototype1-state`. The sandbox interferes with Git
ref/worktree creation, and the command is the live self-propagating runtime.

Tell the user to open another terminal and run:

```bash
cd <active-parent-worktree>
PLOKE_PROTOTYPE1_TRACE_JSONL=auto \
./target/debug/ploke-eval --debug-tools loop prototype1-state --repo-root .
```

`PLOKE_PROTOTYPE1_TRACE_JSONL` controls the machine-readable observation stream:

- unset or empty disables the JSONL layer
- `1`, `true`, `auto`, or `default` writes
  `~/.ploke-eval/logs/prototype1_observation_<run_id>.jsonl`
- any other nonempty value is treated as an explicit output path

## Observe Workflow

`observe` means inspect persisted artifacts without advancing the runtime.

Default live monitor:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-monitor \
  --campaign <campaign> \
  --repo-root <active-parent-worktree> \
  watch --print-initial --interval-ms 1000
```

For analysis, prefer summary projections and targeted file reads over broad
snapshots:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-monitor \
  --campaign <campaign> \
  --repo-root <active-parent-worktree> \
  report
```

Then use narrow `jq`, `tail`, or `rg` commands against the specific
`scheduler.json`, `transition-journal.jsonl`, stream log, or result artifact
named by the summary.

Other current read-only monitor surfaces:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-monitor \
  --campaign <campaign> \
  --repo-root <active-parent-worktree> \
  list

/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-monitor \
  --campaign <campaign> \
  --repo-root <active-parent-worktree> \
  history-metrics

/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-monitor \
  --campaign <campaign> \
  --repo-root <active-parent-worktree> \
  history-preview
```

For Codex-side liveness checks, prefer persisted records over `ps`. Inside the
Codex sandbox, broad process-list commands such as `ps -eo pid,ppid,etime,stat,cmd`
can mostly show the sandbox wrapper and may match their own searched argv,
producing large low-signal output. If host process identity matters, ask the
user to run the `ps` command in their terminal; otherwise infer progress from
the scheduler, stream mtimes, result files, and transition records.

Low-cap snapshot, only when a compact operator-facing dump is useful:

```bash
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-monitor \
  --campaign <campaign> \
  --repo-root <active-parent-worktree> \
  peek --lines 20 --bytes 4000
```

Do not raise `peek` caps to avoid truncation unless the user specifically asks
for that dump. A failed observation attempt used `peek --lines 60 --bytes 20000`
to check a successor handoff; it did answer the question, but it also printed
large branch payloads and journal records into context. The better pattern is
`report` first, then targeted reads of the relevant node/runtime paths.

Use secondary metrics/history/inspect commands only when the user asks for
analysis or the run appears stuck or complete. See
`references/observe-commands.md`.

## Claim Boundary

Observation commands read current records and projections. They do not make the
records sealed History and do not provide Crown authority.
