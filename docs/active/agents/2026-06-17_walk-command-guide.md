# 2026-06-17 Prototype 1 Walk Command Guide

Short description: quick operator guide for trying the debug-only `walk` server, reviewing behavior, and collecting feedback on the command surface.

Related planning/code:

- `crates/ploke-eval/src/cli/prototype1_state/walk/`
- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- [`2026-06-16_walk-server.md`](2026-06-16_walk-server.md)

## What this is

`walk` starts a local debug server that holds one in-memory Prototype 1 typestate value and lets you advance it with short CLI commands.

It is **not** production loop authority. It calls the same live transition functions as `ploke-eval loop prototype1-state`, so later phases can perform real side effects. Today the admitted path is:

```text
R0 -> R1 -> R3 -> R4a -> R4b -> R4c -> R5
```

`R2a` is the alternate parent-identity initialization boundary when started with `--init-parent-identity`.

## Important safety notes

- `R5` is side-effectful: it appends `parent_started` and `resource` entries to the campaign transition journal.
- `R4a -> R4b/R4c` may inspect/switch the active checkout. Use a clean Prototype 1 parent worktree when you want to reach `R5`.
- If you run from a dirty development checkout, failing at `R4a` because local changes would block a checkout switch is expected.
- Always stop the server when done:

```text
ploke-eval loop walk stop
```

## Common flags

```text
--repo-root PATH   Parent checkout to walk. Defaults to current directory.
--socket PATH      Explicit Unix socket. Useful for isolated experiments.
--format json      Machine-readable output.
--format table     Human-readable output, the default.
```

Tip: use a short explicit socket while experimenting:

```text
sockdir=$(mktemp -d /tmp/ploke-walk.XXXXXX)
sock="$sockdir/walk.sock"
```

## Commands

### `status`

Checks whether a server is listening. It does not start one.

```text
ploke-eval loop walk status --socket "$sock" --format json
```

Use this before and after experiments to make sure you do not leave background servers running.

### `start`

Starts the server if needed, creates a fresh in-memory walk, and stops at the requested phase. Default is `R0`.

```text
ploke-eval loop walk start \
  --repo-root "$ROOT" \
  --socket "$sock" \
  --until r0 \
  --format json
```

You can jump through admitted setup phases:

```text
ploke-eval loop walk start --repo-root "$ROOT" --socket "$sock" --until r4c
```

### `step`

Advances one typestate edge from the current in-memory state.

```text
ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --format json
```

Or advance repeatedly until a target phase:

```text
ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r5
```

### `show`

Shows the current phase, server pid, tracked paths, and server-local step history.

```text
ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock"
```

This is the main command for reviewing what happened earlier in the server session.

### `files`

Prints previews of known output files for the current walk.

```text
ploke-eval loop walk files --repo-root "$ROOT" --socket "$sock"
```

Once `R1` has resolved the campaign, this includes:

- parent identity;
- active monitor target;
- campaign manifest;
- transition journal.

### `reset`

Clears the in-memory walk without stopping the server. The server-local history records that reset happened.

```text
ploke-eval loop walk reset --repo-root "$ROOT" --socket "$sock"
```

Use this when you want to try another start without spawning another server process.

### `stop`

Stops the server and removes its socket.

```text
ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock" --format json
```

### `serve`

Runs server mode directly. Normally you do not need this; `start` auto-spawns the same binary in `serve` mode when no healthy server exists.

```text
ploke-eval loop walk serve --repo-root "$ROOT" --socket "$sock"
```

## Recommended review session

Use a clean Prototype 1 parent worktree as `ROOT` if you want to reach `R5`.

```text
ROOT=/path/to/clean/prototype1-parent-worktree
sockdir=$(mktemp -d /tmp/ploke-walk.XXXXXX)
sock="$sockdir/walk.sock"

ploke-eval loop walk status --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk start  --repo-root "$ROOT" --socket "$sock" --until r0 --format json

ploke-eval loop walk step   --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk status --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk step   --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk status --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk step   --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk step   --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk step   --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk step   --repo-root "$ROOT" --socket "$sock" --format json

ploke-eval loop walk show   --repo-root "$ROOT" --socket "$sock"
ploke-eval loop walk files  --repo-root "$ROOT" --socket "$sock"
ploke-eval loop walk reset  --repo-root "$ROOT" --socket "$sock"
ploke-eval loop walk show   --repo-root "$ROOT" --socket "$sock"
ploke-eval loop walk stop   --repo-root "$ROOT" --socket "$sock" --format json
ploke-eval loop walk status --repo-root "$ROOT" --socket "$sock" --format json

rm -rf "$sockdir"
```

## Feedback prompts

While reviewing, useful questions are:

- Is `show` enough to understand the server-local history?
- Should `files` print whole files, tails, JSON summaries, or selectable paths?
- Should `reset` preserve or clear history by default?
- Should reaching `R5` require an explicit `--live-debug` flag because it writes the journal?
- What information should be added before admitting `R6+`?
