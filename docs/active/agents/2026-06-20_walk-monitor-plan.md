# 2026-06-20 Walk Monitor Plan

Short description: implementation plan for `ploke-eval loop walk monitor`, an observation-only dashboard for the Prototype 1 debug walk server.

Related planning/code:

- `docs/active/agents/2026-06-16_walk-server.md`
- `docs/active/agents/2026-06-17_walk-command-guide.md`
- `crates/ploke-eval/src/cli/args/loop_args.rs`
- `crates/ploke-eval/src/cli/prototype1_state/walk/{args,client,controller,phase,protocol,server,paths}.rs`
- `crates/ploke-eval/src/cli/prototype1_state/driver/replay.rs`
- `crates/ploke-eval/src/replay/tool_loop.rs`

## Goal

Add:

```text
ploke-eval loop walk monitor
```

as a continuous, read-only dashboard for:

- live outer typestate phase;
- currently active or last active edge;
- server pid, socket, runtime status, and idle TTL posture;
- historical replay cursor position;
- nested LLM/tool-loop lane progress.

This command must be distinct from `step --watch`: it must not drive typestate edges, call providers, execute tools, mutate worktrees, append History/journal evidence, or move replay/LLM cursors.

## Current architecture notes

### CLI shape

`crates/ploke-eval/src/cli/args/loop_args.rs` defines the `Prototype1StateWalkSubcommand` surface. Existing walk commands generally use either:

- `Prototype1StateWalkControlCommand` for `--repo-root`, `--socket`, `--format`, `--with-version`; or
- bespoke structs for live/effectful commands such as `start`, `step`, and `branch-live`.

A monitor command should probably flatten `Prototype1StateWalkControlCommand` and add only dashboard-specific options.

### Client/server model

`walk/client.rs` is a short-lived one-request client. Most commands resolve repo/socket with `walk::args::resolve_socket`, send a framed JSON `WalkRequest`, print one `WalkResponse`, then exit. `start`, `show`, `llm`, and replay movement currently auto-start or ensure a server; `status` does not.

`walk/server.rs` binds one Unix socket and processes requests serially:

```text
serve -> accept_loop -> handle_stream -> WalkServer::handle
```

`WalkServer` owns one `WalkController`. Mutating requests run through the epoch guard.

Important constraint: during a long `step --watch`, the server is inside `WalkServer::handle` and is not accepting another request. A monitor implemented only as repeated socket `Show`/`Health` requests will hang or time out while the long edge is running, exactly when active-edge visibility is most useful.

### Controller state already available

`WalkController` holds:

```rust
repo_root: PathBuf,
state: WalkState,
steps: usize,
previous: WalkPrevious,
files: WalkFiles,
last_delta: Option<WalkAdvanceReport>,
reconstruction: Option<WalkReconstruction>,
replay: Option<ReplayCursor>,
llm_focus: Option<String>,
llm_cursors: BTreeMap<String, usize>,
```

Existing read-only renderers already cover much of the desired monitor content:

- `describe()` for phase, steps, files, typestate, next steps, previous entries;
- `delta_report()` for last step delta;
- `replay_report/back/forward()` through `ReplayCursor`;
- `llm_lanes_report()` for lane status/cursor/head/next-step/model;
- `llm_timeline()` for a selected lane timeline.

For monitor, prefer small structured snapshot helpers rather than parsing these strings.

### Replay cursor

`ReplayCursor` in `driver/replay.rs` is read-only over durable journal evidence. It currently renders text and keeps the operator cursor in memory. To show replay cursor status from monitor, add a small read-only accessor/snapshot method rather than parsing `render()` output.

### LLM/tool-loop progress

`crates/ploke-eval/src/replay/tool_loop.rs` persists sessions, steps, and resume records under:

```text
{campaign}/prototype1/debug/tool-loop/{session}/...
```

`WalkController::llm_lanes()` already groups latest sessions by lane and derives:

- lane id;
- session id;
- `ToolLoopStatus`;
- cursor from server memory;
- latest recorded head;
- resume `next_step`;
- model.

This can directly feed monitor lane rows.

### Existing “monitor target” terminology

`prototype1-monitor-target.json` is an active target cache containing `campaign_id` and `repo_root`. It is useful for locating the active run but is not a live dashboard state file. Do not overload its semantics.

## Recommended implementation approach

Use a **runtime monitor state file** next to the walk socket, plus a lightweight CLI dashboard that reads that file and durable artifacts. This avoids a broad server-concurrency refactor and lets `monitor` show active-edge state while the server is blocked inside a long `step --watch`.

### New runtime state file

Add a debug-only status file, for example:

```text
{socket_stem}.status.json
# default example:
$XDG_RUNTIME_DIR/ploke-eval/walk/p1walk-<repo-hash>.status.json
```

Likely path helper:

```rust
// walk/paths.rs
pub(crate) fn monitor_status_path(socket: &Path) -> PathBuf
```

Write this file atomically with temp-file + rename. It is observability only, not authority.

Suggested schema:

```rust
struct WalkMonitorState {
    schema_version: String,
    repo_root: PathBuf,
    socket: PathBuf,
    server_pid: u32,
    status: WalkRuntimeStatus,
    phase: WalkPhase,
    phase_detail: String,
    steps: usize,
    active: Option<WalkActiveEdge>,
    idle_ttl_secs: Option<u64>,
    idle_deadline_unix_ms: Option<u128>,
    updated_at_unix_ms: u128,
    last_error: Option<String>,
    replay: Option<WalkReplayCursorSummary>,
    llm: WalkLlmSummary,
}

enum WalkRuntimeStatus {
    Starting,
    Idle,
    Active,
    Failed,
    Stopping,
    Stopped,
}

struct WalkActiveEdge {
    operation: String,      // e.g. "step", "start", "llm_finish"
    from: WalkPhase,
    target: Option<WalkPhase>,
    edge_hint: String,      // e.g. "r10_to_r11 --watch"
    started_at_unix_ms: u128,
    watch: bool,
    mutation_admission: Vec<String>,
}
```

Keep names at or under three semantic parts where possible in the final code; use nested structs instead of flattened names if more detail is needed.

### Server write points

Add monitor-state writes in `walk/server.rs` around request handling:

1. server startup after bind: `Starting` then `Idle`;
2. before `Start`, `Step`, `LlmStep`, `LlmFinish`, `Reset`, and `BranchLive`: write `Active` with operation and edge hint;
3. after each request: write `Idle`, `Failed`, `Stopping`, or `Stopped` with final phase/error;
4. when idle TTL is configured: include current idle deadline after each request completes.

For `Step`, derive `edge_hint` from current phase and request options using `WalkPhase::next_steps()`:

- `R7 + --watch` => `r7_to_r8 --watch`;
- `R10 + --watch` => `r10_to_r11 --watch`;
- `R12 + --watch --allow git-changes` => `r12_to_r13 --watch --allow git-changes`;
- `--until` => include the target phase even if the exact branch is not known yet.

This gives active-edge visibility during long edges without requiring the server to accept another socket request.

### Controller snapshot helpers

Add focused read-only helpers instead of parsing text renderers:

- `WalkController::monitor_base()` or similar: phase, steps, failed detail, next steps, previous tail.
- `WalkController::replay_summary()` returning current replay cursor metadata if loaded or loadable.
- `WalkController::llm_summary(limit)` returning grouped lane status rows.

If implementing the runtime status file first, the server can call these helpers after each handled request. The monitor command can also use the same pure render functions when it has a fresh snapshot.

### Monitor CLI command

Add a new subcommand:

```rust
Monitor(Prototype1StateWalkMonitorCommand)
```

Suggested initial options:

```text
ploke-eval loop walk monitor [--repo-root PATH] [--socket PATH]
                           [--interval-ms 1000]
                           [--once]
                           [--no-clear]
                           [--llm-limit 8]
                           [--format table|json]
                           [--with-version]
```

Behavior:

- default continuous table dashboard;
- Ctrl-C exits naturally;
- `--once` prints one snapshot and exits, useful for tests/scripts;
- `--no-clear` appends snapshots rather than clearing the screen;
- `--format json --once` prints one structured snapshot;
- continuous JSON should either be JSONL or rejected initially with a clear message. Prefer `--once` for JSON in the first slice.

Important TTL policy: a file-based monitor should not contact the server on every tick, so it should not keep the server alive. It can optionally probe the socket on startup or on a slow cadence later, but the first implementation should rely on the status file and mark stale states by `updated_at` age.

### Dashboard table sketch

```text
walk monitor                         updated 2s ago
-----------------------------------------------
server: pid=12345 status=active socket={runtime}/p1walk-...sock
repo:   /path/to/parent
phase:  r10 - selection strategy ready   steps=9
edge:   active for 04:12  r10_to_r11 --watch  target=r11|r11a
TTL:    1800s configured; paused while active
replay: #0042 / #0091 r12 successor.selected parent-node
llm:    lanes=5 active=1 paused=2 terminal=2 focus=node-abc
        * node-abc status=active cursor=6 head=8 next=9 model=...
          node-def status=terminal cursor=- head=4 next=- model=...
next:   walk show delta | walk llm timeline --lane node-abc | walk replay --tail 20
```

## Slices

### Slice 1 — status file foundation

- Add monitor status path helper in `walk/paths.rs`.
- Add serializable monitor DTOs, probably in a new `walk/monitor.rs` module to keep `protocol.rs` from growing too much.
- Write status on server startup, after normal requests, and on stop.
- Include server pid, repo root, socket, phase, steps, status, configured TTL, and update time.
- Keep replay/LLM summaries optional in this slice.

Verification:

- unit tests for status path derivation and atomic write/read;
- server smoke with temp socket: `start --until r0`, read status JSON, `stop`, read stopped status.

### Slice 2 — `monitor --once`

- Add clap command and parse test.
- Add client dispatch that resolves repo/socket, reads the status file, and renders a single table snapshot.
- If status file is missing, print an offline/stale-friendly message with exact next commands.
- Support `--format json --once`.

Verification:

- CLI parse test in `crates/ploke-eval/src/cli/tests.rs`;
- renderer unit tests for idle, active, stale, and missing status cases.

### Slice 3 — LLM lane and replay summaries

- Add `ReplayCursor` summary accessor for current cursor metadata.
- Add `WalkController` or standalone helper for LLM lane summaries using `FsToolLoopStore`.
- Have the server refresh these summaries in the monitor status file after requests that can affect them:
  - `Replay`, `ReplayBack`, `ReplayForward`, `BranchLive`;
  - `LlmFocus`, `LlmBack`, `LlmForward`, `LlmHead`, `LlmStep`, `LlmFinish`;
  - `Step` after fanout/tool-loop-producing edges.
- For the monitor client, optionally re-read LLM durable checkpoint files each tick so lane heads progress even while a long edge is running.

Verification:

- controller unit tests can reuse existing tool-loop fixture helpers near `controller.rs` tests;
- assert lane rows show `status`, `cursor`, `head`, and `next_step`.

### Slice 4 — continuous dashboard loop

- Add a Tokio interval loop in `walk/client.rs` for `monitor`.
- Clear screen by default with ANSI escape; `--no-clear` appends.
- Mark status stale when `updated_at` is older than a threshold, e.g. `max(3 * interval, 5s)`.
- Do not poll the socket every tick by default.

Verification:

- pure renderer tests;
- one narrow async test for `--once`; avoid brittle continuous-loop tests.

### Slice 5 — optional heartbeat/progress refinement

If operators need stronger in-progress fidelity:

- add heartbeat updates during known long operations;
- pass a tiny progress sink into `WalkController::step`/`step_once` so multi-edge `--until` can update the active edge before each transition;
- add tool-loop checkpoint polling to show growing heads while child fanout runs;
- only consider concurrent socket handling after this file-based approach proves insufficient.

## Why not start with server concurrency?

A concurrent server would require splitting `WalkServer` into shared state, controller locks, activity locks, and try-lock monitor paths. That could work, but it changes the core invariant that the server processes one request at a time. The status-file approach preserves the existing serial request model and directly solves the biggest monitor problem: observing a long request while the server is busy.

If concurrency is later needed, use a design pass first. The likely shape would be:

```text
Arc<WalkServerShared> {
  controller: Mutex<WalkController>,
  activity: RwLock<WalkActivity>,
  runtime: RwLock<ServerRuntime>,
}
```

Monitor requests would read `activity` without waiting for the controller lock and use the latest published snapshot when the controller is busy. This is a larger change than the first monitor feature needs.

## Safety guardrails

- Monitor state is not History, journal, replay, identity, or handoff authority.
- Monitor must never call provider APIs or TUI tools.
- Monitor must never mutate replay/LLM cursors.
- Monitor should avoid per-tick socket requests so it does not accidentally keep an idle server alive.
- Status-file reads must tolerate missing/stale/partial files and report uncertainty rather than weakening invariants.

## GitNexus/code-intel notes

GitNexus index was stale at first and was refreshed with `npx gitnexus analyze` before impact checks.

Impact checks for likely future touch points:

- `Prototype1StateWalkSubcommand`: LOW, no upstream callers reported.
- `WalkRequestBody`: LOW, no upstream callers reported.
- `WalkServer::handle`: LOW; direct caller `handle_stream`, then `accept_loop`, then `serve` in the `Walk` module.
- `WalkController::describe`: LOW, no upstream callers reported by GitNexus.

The graph-level blast radius is low, but the conceptual risk is moderate if implementation changes server concurrency. The recommended status-file plan avoids that concurrency risk in the first slices.

## Open decisions

1. Should `monitor` auto-start the server when no status file exists? Recommendation: no; keep it observation-only and suggest `walk start`.
2. Should continuous JSON be supported initially? Recommendation: require `--once` for JSON first; add JSONL later if needed.
3. Should monitor read durable LLM checkpoint files directly every tick? Recommendation: yes after Slice 2, because it gives progress during long fanout without contacting the server.
4. Should monitor status files be removed on stop? Recommendation: leave a final `Stopped` status for postmortem visibility.
