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
```

`R2a` is the alternate parent-identity initialization boundary when started with `--init-parent-identity`.

## Important safety notes

- `R5` is side-effectful: it appends `parent_started` and `resource` entries to the campaign transition journal.
- `R5 -> R6` may establish/advance baseline closure state before loading the parent baseline; reconstruction uses durable baseline evidence when present.
- `R6 -> R7` derives run policy and child-planning budget and preserves hard budget/max-generation stops.
- `R4a -> R4b/R4c` may inspect/switch the active checkout. Use a clean Prototype 1 parent worktree when you want to reach `R7`.
- If you run from a dirty development checkout, failing at `R4a` because local changes would block a checkout switch is expected.
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

### `start`

Starts the server if needed, creates a fresh in-memory walk, and stops at the requested phase. Default is `R0`.

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

Use a clean Prototype 1 parent worktree as `ROOT` if you want to reach `R7`.

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
- Is 30 minutes the right default idle TTL?
- What information should be added before admitting `R8+`?
