# 2026-06-16 Prototype 1 Typestate Walk Server

Short description: cold-restart orientation for the debug-only local server that steps `prototype1-state` through the new live typestate transitions in memory.

Related planning/code:

- `crates/ploke-eval/src/cli/prototype1_state/walk/`
- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- `crates/ploke-eval/src/cli/prototype1_state/typestate/transition.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- Zellij reference clone used during design: `/home/brasides/clones/zellij`

## Current status

Implemented across commits:

```text
89f56f00 Add Prototype 1 typestate walk server
2d912680 docs: document Prototype 1 walk server code
5c121e73 Extend Prototype 1 walk server through R5
```

Prerequisite edge/API commits:

```text
3eeb707a Extract Prototype 1 live typestate edges
 d786b30d Use direct functions for Prototype 1 live edges
```

The server is a debug harness. It is not production loop authority and should not become the controller for the live self-evolving loop without an explicit later design pass.

The existing live command remains:

```text
ploke-eval loop prototype1-state ...
```

The new debug command is:

```text
ploke-eval loop walk ...
```

## Operator commands

Set an active parent checkout for short later commands:

```text
ploke-eval loop walk use /path/to/prototype1-parent-worktree
```

Check server status without starting it:

```text
ploke-eval loop walk status
```

Start a server if needed, create a new in-memory walk, and stop at R0:

```text
ploke-eval loop walk start
```

Advance one step from the current in-memory state:

```text
ploke-eval loop walk step
```

Advance until a supported phase:

```text
ploke-eval loop walk step --until r5
```

Show current state, root/tracking dirs, typestate alias, next admitted edges, and previous entries:

```text
ploke-eval loop walk show
```

Show only the last successful step delta:

```text
ploke-eval loop walk show delta
ploke-eval loop walk show delta --verbose
ploke-eval loop walk show delta --no-color
```

Print tracked output files for the current walk:

```text
ploke-eval loop walk files
```

Reset the in-memory walk without stopping the server:

```text
ploke-eval loop walk reset
```

Stop the server:

```text
ploke-eval loop walk stop
```

For isolated smoke tests, pass an explicit short socket path:

```text
sockdir=$(mktemp -d /tmp/ploke-walk.XXXXXX)
sock="$sockdir/walk.sock"

cargo run -q -p ploke-eval -- loop walk start --socket "$sock" --until r0
cargo run -q -p ploke-eval -- loop walk step  --socket "$sock"
cargo run -q -p ploke-eval -- loop walk show  --socket "$sock"
cargo run -q -p ploke-eval -- loop walk stop  --socket "$sock"

rm -rf "$sockdir"
```

## Module map

`walk/mod.rs`

- Wires the walk command to either server mode or client mode.
- Keeps the server separate from `cli_facing` and `typestate`.

`walk/phase.rs`

- Defines the CLI/protocol phase enum `WalkPhase`.
- Current supported phases are:

```text
Empty, R0, R1, R2a, R3, R4a, R4b, R4c, R5, R6, R7, R8, R9
```

`walk/protocol.rs`

- Defines `WalkRequest`, `WalkResponse`, and `WalkStartConfig`.
- Uses serde JSON payloads over framed Unix socket IPC.
- `WalkStartConfig` mirrors the current `Prototype1StateCommand` surface enough to build the real command on the server side.

`walk/ipc.rs`

- Implements length-prefixed JSON framing:

```text
4 bytes little-endian payload length
N bytes serde_json payload
```

- This borrows the key robustness idea from Zellij's length-prefixed IPC framing, but does not use protobuf.

`walk/paths.rs`

- Computes the Unix socket path.
- Default location:

```text
$XDG_RUNTIME_DIR/ploke-eval/walk/p1walk-<repo-hash>.sock
```

- If `XDG_RUNTIME_DIR` is unavailable, falls back under `/tmp/ploke-eval-$USER/walk/`.
- Supports `PLOKE_EVAL_WALK_SOCKET_DIR`, explicit `--socket`, and saved `walk use --socket` context.
- Saves active walk context at `context.json` under the same runtime socket directory.
- Creates parent directory with `0700` permissions on Unix.
- Checks Unix socket path length against platform limits.

`walk/epoch.rs`

- Captures the server/client freshness epoch:

```text
protocol_version
transition_graph_version
repo_root
current executable path
current executable modified time
git HEAD
guarded source status hash
```

- Mutating requests (`start`, `step`, `reset`) include a client epoch and are rejected if the server epoch differs.
- Non-mutating requests (`health`, `show`, `files`, `stop`) do not require a matching epoch.

Important limitation: the current source guard is based on `git status --porcelain` for selected paths. It detects clean/dirty file set changes, but it is not a full content hash of every dirty file. If exact dirty-content drift detection becomes required, replace or supplement this with a content hash over the guarded source paths.

`walk/controller.rs`

- Owns the in-memory `WalkState`.
- Calls canonical direct functions from `live_edges.rs`; it does not duplicate transition semantics.
- Uses the value-first typestate API:

```rust
r0.advance(r0_to_r1)?
r4a.advance(r4a_to_r4b_or_r4c)?
```

- Current supported path:

```text
Empty -> R0
R0 -> R1
R1 -> R2a | R3
R3 -> R4a
R4a -> R4b | R4c
R4b -> R4c
R4c -> R5
R5 -> R6
R6 -> R7
R7 -> R8  # requires explicit walk step --watch for live child-plan work
R8 -> R9  # pure schedule shaping
```

- `R2a` and `R9` are current stops.
- Default live stepping at `R7` is a safe boundary; live `R7 -> R8` requires explicit `walk step --watch`.
- `R8` is reconstructable from existing child-plan authority.
- Later phases (`R10+`) are intentionally not admitted yet by the server slice.
- `show` includes server-local previous entries, root/tracking directories, tracked file paths, current typestate alias, and next admitted edges.
- `show delta` includes only the last successful step delta; `--verbose` also lists nested type structures, and `--no-color` disables ANSI colors.
- `step` includes from/to phases, edge names, typestate axis deltas, and next admitted edges.
- `files` prints parent identity, active monitor target, campaign manifest, and transition journal previews once those paths are known.

Failure behavior:

- A failed consuming transition cannot keep the original Rust typestate value.
- The controller records `WalkState::Failed { phase, detail }` so `show` remains informative.
- After failure, start a new walk to continue.

`walk/server.rs`

- Runs the long-lived local server.
- Binds the Unix socket.
- Accepts one request at a time.
- Owns `WalkController` and `ServerEpoch`.
- Removes the socket file when exiting.

`walk/client.rs`

- Implements short-lived CLI client behavior.
- `use` saves the active repo/socket context for short later commands.
- `start` probes health, cleans up a stale socket if offline, spawns the same binary in server mode, waits for health, then sends `Start`.
- `status`, `show`, `files`, `step`, `reset`, and `stop` send one request and print one response.
- Table output hides protocol/transition-graph versions unless `--with-version` is passed.
- Connection-refused health probes remove stale socket files, matching Zellij's stale-session cleanup pattern.

`walk/args.rs`

- Converts the clap command structs into protocol DTOs and socket resolution inputs.

## Zellij-inspired decisions

Borrowed from Zellij's client/server shape:

- same binary can run as normal CLI client or local server;
- client commands are short-lived and communicate over a Unix socket;
- server owns in-memory session state;
- socket lives under a runtime directory with private permissions;
- saved walk context also lives under that runtime directory;
- socket path length is checked;
- health/status and stop requests exist;
- IPC messages are explicitly framed.

Differences from Zellij in this first slice:

- Uses length-prefixed JSON, not protobuf.
- Uses `tokio::net::UnixListener` / `UnixStream`, not the `interprocess` crate.
- Uses a background child process with stdio redirected to null, not full Unix double-fork daemonization.
- Auto-started servers have a 30-minute idle TTL by default; use `--ttl-secs` or `--no-ttl` to change it.
- Supports one active walk per server, not multiple named sessions.
- Processes requests serially, not via a router thread and central bus.

These differences are intentional for the debug harness. If the server becomes part of parent/successor lifecycle later, revisit daemonization, lifecycle supervision, logging, and session identity.

## Guardrails

Do not treat the server as History, journal, parent identity, or handoff authority.

The server is allowed to hold:

- current in-memory `WalkState`;
- current phase cursor;
- debug response summaries;
- server epoch.

The server is not allowed to become authoritative for:

- sealed History;
- journal truth;
- parent/successor identity;
- live loop continuation policy;
- handoff finality.

The live loop must remain capable of self-editing without being controlled by a stale server. The epoch guard exists to fail closed when client/server/source versions drift.

## Known sharp edges

- Default live stepping is supported through `R7`; `R8` is reconstructable from existing child-plan message evidence and live-entered only with `walk step --watch`.
- `R5` is live-effectful: it appends parent-start/resource entries to the transition journal.
- `R5 -> R6` may establish/advance baseline closure state before loading the parent baseline; reconstruction uses durable baseline evidence when present.
- `R6 -> R7` derives run policy and child-planning budget and preserves hard budget/max-generation stops.
- Live `R7 -> R8` is exposed only through explicit `walk step --watch`; default step at R7 does not start provider/harness work.
- Later async/live-effectful phases (`R9+`, child fanout, handoff, final report) are not yet exposed through the server.
- Failed consuming transitions are recorded as `Failed`, not retryable from the exact consumed Rust state.
- The current source freshness check is status-hash based, not full content-hash based.
- Server stdout/stderr are redirected to null when auto-spawned. Add an explicit log path before relying on the server for long debugging sessions.
- The server currently handles one active walk. A second `start` is rejected unless the controller is `Empty` or `Failed`.

## Next implementation slices

1. Add explicit server log file support.
2. Add unit tests around `paths`, `protocol`, and `WalkController` failure-state behavior.
3. Add automated `step --until r7` smoke coverage with a fixture checkout that has the required parent identity/campaign setup.
4. Replace the blocking `--watch` implementation with background progress reporting before exposing more long R8+ live edges.
5. Upgrade epoch source drift detection from `git status` hash to content hash if long-running server use becomes common.
6. Only after the server is stable as a debug harness, consider parent/successor lifecycle integration:
   - parent server terminates or becomes read-only at handoff;
   - successor starts a fresh server from the successor checkout/binary.

## Verification from implementation turn

Commands run after implementation:

```text
cargo test -p ploke-eval typestate --all-targets
cargo test -p ploke-eval prototype1_state --all-targets
cargo check -p ploke-eval --all-targets
```

Smoke commands run with a temp socket:

```text
cargo run -q -p ploke-eval -- loop walk start --socket "$sock" --until r0
cargo run -q -p ploke-eval -- loop walk step  --socket "$sock"
cargo run -q -p ploke-eval -- loop walk show  --socket "$sock"
cargo run -q -p ploke-eval -- loop walk stop  --socket "$sock"
```

Additional R5 smoke run after `5c121e73` used a clean existing parent worktree and an explicit socket:

```text
ploke-eval loop walk use "$ROOT" --socket "$SOCK"
ploke-eval loop walk start --until r0
ploke-eval loop walk step # repeated through r5
ploke-eval loop walk files
ploke-eval loop walk reset
ploke-eval loop walk stop
```
