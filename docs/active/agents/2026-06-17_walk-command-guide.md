# 2026-06-17 Walk Command Guide

Short description: quick operator guide for trying the debug-only `walk` server, reviewing behavior, and collecting feedback on the command surface.

Related planning/code:

- `crates/ploke-eval/src/cli/prototype1_state/walk/`
- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- [`2026-06-16_walk-server.md`](2026-06-16_walk-server.md)

## What this is

`ploke-eval loop walk` starts or talks to a local debug server that holds one in-memory Prototype 1 typestate value and lets you advance it with short CLI commands.

It is **not** production loop authority. It calls the same live transition functions as `ploke-eval loop prototype1-state`, so later phases can perform real side effects. Today the admitted path is:

```text
R0 -> R1 -> R3 -> R4a -> R4b -> R4c -> R5 -> R6 -> R7
R7 -> R8 requires walk step --watch for live child-plan work
R8 -> R9 runs as pure schedule shaping when R8 is already held/reconstructed
R9 -> R10 resolves successor-selection strategy
R10 -> R11a | R11 requires walk step --watch for rejected-only/fanout work
R11a | R11 -> R12 projects report facts without emitting final report
R12 -> R13a records stopped/no-selection or selected-successor stopped-by-policy continuation
R12 -> R13b commits selected-successor handoff only with --watch --allow git-changes
R13a -> R14a emits stopped/no-selection final report
R13b -> R14b emits final report after successor handoff
```

`R8` can also be reconstructed when an existing child-plan message is present. `R11/R12` can be reconstructed after restart when all planned terminal children have validated channel `Result` evidence, matching runner results, and successful children have matching branch evaluation reports. Stopped `R13a/R14a` and handoff `R13b/R14b` can be reconstructed from R12 report facts plus matching durable successor handoff/stopped and parent-complete evidence. `R2a` is the alternate parent-identity initialization boundary when started with `--init-parent-identity`.

## Important safety notes

- `R5` is side-effectful: it appends `parent_started` and `resource` entries to the campaign transition journal.
- `R5 -> R6` may establish/advance baseline closure state before loading the parent baseline; reconstruction uses durable baseline evidence when present.
- `R6 -> R7` derives run policy and child-planning budget and preserves hard budget/max-generation stops.
- `R8` can be reconstructed from existing child-plan authority. Live `R7 -> R8` requires explicit `walk step --watch` because it may wait on provider/harness work.
- `R8 -> R9` and `R9 -> R10` are pure in-process edges and do not require `--watch`.
- `R10 -> R11a | R11` requires explicit `walk step --watch` because it may project rejected-only selection evidence or run live child fanout.
- `R11a | R11 -> R12` is a pure projection of report facts and does not emit the final report. Restart reconstruction can rebuild R12 from channel-derived terminal child outcomes without rerunning fanout.
- `R12 -> R13a` is the stopped continuation branch. If R12 has selected-successor evidence and the target is the stopped branch, `walk` rejects the target and tells you to use the matching handoff target.
- `R12 -> R13b` is selected-successor handoff. It is admitted only with `walk step --watch --allow git-changes` because it may seal/advance History, install the selected successor into the active checkout, retire the parent, spawn the successor runtime, and wait for successor readiness.
- `R13a -> R14a` emits the stopped/no-selection final report and records parent-complete evidence; restart reconstruction uses that evidence read-only and does not re-emit the report.
- `R13b -> R14b` emits the final report after successor handoff once R13b exists.
- If you run from a dirty development checkout, failing at `R4a` because local changes would block a checkout switch is expected.
- `walk start --until ...` on an existing matching parent checkout prefers durable reconstruction before creating a fresh R0 walk. If reconstruction is already past the requested phase, `start` returns the later reconstructed phase instead of replaying live setup; historical `--until r12` smokes therefore do not duplicate parent-start/resource journal entries.
- The auto-started server exits after 30 idle minutes by default.
- You can still stop it explicitly:

```text
ploke-eval loop walk stop
```

## Defaults

Human-readable table output is the default. Use JSON only when scripting:

```text
ploke-eval loop walk status --format json
```

Protocol and transition-graph versions are hidden in table output by default. Show them when needed with:

```text
ploke-eval loop walk status --with-version
ploke-eval loop walk show --with-version
```

Repo root resolution order:

1. explicit `--repo-root PATH`;
2. saved `walk use` context;
3. current working directory.

Socket resolution order:

1. explicit `--socket PATH`;
2. saved `walk use --socket PATH` context;
3. repo-hashed socket under the runtime directory.

The default socket follows the Zellij-style runtime-dir pattern:

```text
$XDG_RUNTIME_DIR/ploke-eval/walk/p1walk-<repo-hash>.sock
```

If `XDG_RUNTIME_DIR` is unavailable, it falls back under:

```text
/tmp/ploke-eval-$USER/walk/
```

## First-time setup: `use`

Remember a clean Prototype 1 parent worktree so later commands can be short:

```text
ploke-eval loop walk use /path/to/prototype1-parent-worktree
```

If you are already in the parent worktree:

```text
ploke-eval loop walk use
```

Optional explicit socket, mainly useful for isolated experiments:

```text
ploke-eval loop walk use /path/to/parent --socket /tmp/my-walk.sock
```

## Main commands

### `status`

Checks whether a server is listening. It does not start one.

```text
ploke-eval loop walk status
```

### `summary`

Reads durable campaign artifacts without contacting the walk server. This is the fastest first command for a completed historical run.

```text
ploke-eval loop walk summary
ploke-eval loop walk summary -v
ploke-eval loop walk summary --format json
```

Verbose mode explains compact fields such as `branch=reject decision=Stop handoff=acknowledged` and points to typed `history` inspection commands.

### `replay`, `back`, and `forward`

Move a read-only historical cursor over the transition journal. These commands do not undo side effects and do not call providers, spawn children, mutate checkout state, or append History.

```text
ploke-eval loop walk replay --index 0
ploke-eval loop walk forward --steps 10 --tail 20
ploke-eval loop walk back --steps 5
```

The default recent-entry window is 3 entries. Use `--tail N` to expand it. Current rendering shows the journal tail, not a cursor-centered window.

### `branch-live`

Records explicit provenance before leaving read-only replay toward future live work. It does not materialize a live branch by itself.

```text
ploke-eval loop walk branch-live --reason "investigate cursor 42" --allow provenance-record
```

### `start`

Starts the server if needed and stops at the requested phase. Default is `R0`. For `--until` targets beyond R0 on an existing parent checkout, `start` first attempts durable reconstruction and uses that state when it matches the requested campaign/checkout. If the reconstructed durable phase is already later than the target, `start` returns that later phase instead of replaying live setup; otherwise it creates a fresh R0 walk only when reconstruction is unavailable or for a different explicit campaign.

```text
ploke-eval loop walk start
```

Start and advance immediately:

```text
ploke-eval loop walk start --until r4c
```

Server idle TTL controls for auto-started servers:

```text
ploke-eval loop walk start --ttl-secs 3600
ploke-eval loop walk start --no-ttl
```

### `step`

Advances one typestate edge from the current in-memory state and prints the from/to phases, edge name, typestate axis changes, and next admitted edges.

```text
ploke-eval loop walk step
```

Or advance repeatedly until a target phase:

```text
ploke-eval loop walk step --until r7
```

At R7, start the live child-plan authority edge only when you are ready to wait:

```text
ploke-eval loop walk step --watch
```

At R8, the default next step is the pure schedule edge:

```text
ploke-eval loop walk step
# r8_to_r9 -> r9
```

At R9, the default next step resolves selection strategy:

```text
ploke-eval loop walk step
# r9_to_r10 -> r10
```

At R10, the default step is intentionally safe/no-op. Use explicit watch mode to admit rejected-only/fanout work:

```text
ploke-eval loop walk step --watch
# r10_to_r11 --watch -> r11a | r11
```

At R11a/R11, the default next step projects report facts:

```text
ploke-eval loop walk step
# r11_to_r12 -> r12
```

At R12, the default next step follows the branch that matches the report facts. No-selection or stopped-by-policy states go to R13a. Selected-successor states require explicit live handoff admission:

```text
ploke-eval loop walk step
# r12_to_r13 -> r13a, when no handoff is allowed/available

ploke-eval loop walk step --watch --allow git-changes
# r12_to_r13 -> r13b, when selected-successor handoff is allowed
```

At R13a, the default next step emits the stopped/no-selection final report and stops at R14a. At R13b, the default next step emits the final report after handoff and stops at R14b:

```text
ploke-eval loop walk step
# r13_to_r14 -> r14a | r14b
```

### `show`

Shows the current phase, server pid, root/tracking directories, tracked paths, current Rust typestate alias, next admitted edges, and server-local previous entries.

```text
ploke-eval loop walk show
```

Tracked paths are summarized with placeholders like `{root}/...` and `{tracking_dir}/...` to keep the output readable. This is the main command for reviewing what happened earlier in the server session.

Show only the last successful step delta:

```text
ploke-eval loop walk show delta
ploke-eval loop walk show delta --verbose
ploke-eval loop walk show delta --no-color
```

`show delta` colors added/removed/changed type axes by default in table output. `--verbose` also lists nested type structures added or removed by the last step.

### `files`

Prints previews of known output files for the current walk.

```text
ploke-eval loop walk files
```

Once `R1` has resolved the campaign, this includes:

- parent identity;
- active monitor target;
- campaign manifest;
- transition journal.

### `reset`

Clears the in-memory walk without stopping the server. The server-local history records that reset happened.

```text
ploke-eval loop walk reset
```

Use this when you want to try another start without spawning another server process.

### `stop`

Stops the server and removes its socket.

```text
ploke-eval loop walk stop
```

### `serve`

Runs server mode directly. Normally you do not need this; `start` auto-spawns the same binary in `serve` mode when no healthy server exists.

```text
ploke-eval loop walk serve --ttl-secs 1800
ploke-eval loop walk serve --no-ttl
```

## Recommended review session

Use a clean Prototype 1 parent worktree as `ROOT` if you want to reach `R7`, or one with existing valid child-plan evidence if you want to reconstruct R8 and step to R10 without additional provider/harness work. R10 -> R11 requires `--watch` and may run live child fanout; R11 -> R12 is an in-memory report-facts projection. From R12, stopped/no-selection paths go to R13a/R14a, while selected-successor handoff goes to R13b/R14b only with `--watch --allow git-changes`.

```text
ploke-eval loop walk use /path/to/prototype1-parent-worktree
ploke-eval loop walk status
ploke-eval loop walk start

ploke-eval loop walk step
ploke-eval loop walk status
ploke-eval loop walk step
ploke-eval loop walk status
ploke-eval loop walk step
ploke-eval loop walk step
ploke-eval loop walk step
ploke-eval loop walk step

ploke-eval loop walk show
ploke-eval loop walk files
ploke-eval loop walk reset
ploke-eval loop walk show
ploke-eval loop walk stop
ploke-eval loop walk status
```

## Feedback prompts

While reviewing, useful questions are:

- Is `show` enough to understand the server-local history?
- Should `files` print whole files, tails, JSON summaries, or selectable paths?
- Should `reset` preserve or clear history by default?
- Should reaching `R5`/`R6` require an explicit `--live-debug` flag because these edges can write journal/baseline evidence?
- Should `R8 -> R9 -> R10` remain default steps, or should all post-child-plan edges require an explicit mode even when they are pure?
- Should R10 fanout watches be split further so rejected-only projection and live child fanout have separate operator commands?
- Is 30 minutes the right default idle TTL?
- Should `--watch` become a background progress stream with `walk show progress` for long R8+ edges?
