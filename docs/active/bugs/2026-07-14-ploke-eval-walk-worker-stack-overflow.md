# Ploke Eval Walk Worker Stack Overflow

Status: fixed in source; committed-binary recovery and live transition validation pending.

Canary worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-stage4-observe-g35f-oaiembed-3g1x3-p3-20260714-004446
```

## Broken Contract

A guarded walk step must either persist its typed transition lifecycle and
return a terminal job result or leave a recoverable typed failure. The
production server must not abort after acquiring the controller fence but
before recording `AttemptBegan`.

## Evidence

Guarded operation `8df96ebf-ac84-40ec-bc2a-18f1a89b1701` was admitted against
session `7835dfe6-8ef5-437d-87f1-7a8e98c36663` at R3. The client then received
a connection reset and server process `1130803` terminated with `SIGABRT`.

The authoritative journal is:

```text
/home/brasides/.ploke-eval/campaigns/p1-stage4-observe-g35f-oaiembed-3g1x3-p3-20260714-004446/prototype1/control/sessions/5f70d599e287b10017e196b710b753c156e5721fdff81446c3b00169eeaf2e99/control-journal.jsonl
```

Its event sequence is exactly:

```text
Created(R3)
Acquired(fence 1)
Released(fence 1)
EpochAdmitted
Acquired(fence 2, pid 1130803, start_ticks 22522942)
```

There is no `AttemptBegan`, `AttemptFinished`, cursor commit, or release for
fence 2. The durable cursor therefore remains R3, while the dead owner must be
resolved explicitly before another mutation.

`coredumpctl --no-pager info 1130803` records the production command and the
failing path through:

```text
run_step_job
-> WalkController::step_version
-> step_at
-> advance_until / step_toward
-> step_claimed
-> advance_controlled
-> reconstruct_at
-> reconstruct_exact
-> reconstruct
-> stack overflow on tokio-rt-worker
```

Disassembly of the debug binary accounts for 2,097,792 bytes of simultaneously
active frames on the direct `step_once` reconstruction route, 640 bytes more
than Tokio's 2,097,152-byte default worker stack before scheduler overhead. The
production `advance_until -> step_toward` route that failed in this canary
retains both routing frames and is larger still, approximately 2,239,768 bytes.
`WalkResponse` is only 4,184 bytes and did not change in the evaluation-trace
commit, so response framing and the new trace inspector are not causal.

## Source Trace

`crates/ploke-eval/src/cli/prototype1_state/walk/server.rs` spawns
`run_step_job` on the Tokio runtime. It calls `WalkController::step_version`,
which claims the durable lease in
`crates/ploke-eval/src/cli/prototype1_state/walk/controller.rs` before entering
`driver::control::advance_controlled`. The latter reconstructs the exact R3
state before admission writes `AttemptBegan`. The core dump at reconstruction
therefore matches the journal boundary exactly: fence acquired, transition not
begun.

The runtime owning this production task was previously created by
`#[tokio::main]` in `crates/ploke-eval/src/main.rs`, which inherited Tokio's
default worker-stack budget.

## Docs and Policy Expectation

`docs/active/agents/2026-07-13_prototype1-loop-operator-control-plan.md`
requires UI and CLI mutations to share one durable authority path and requires
each accepted mutation to remain observable through typed job and transition
receipts. `docs/active/bugs/2026-07-14-walk-pre-session-phase-and-start.md`
pins the canary's exact R3 session and mutation guard.

## Current Repro Coverage

The binary now builds its production multi-thread runtime explicitly with an
8 MiB worker-stack budget. The focused runtime-capacity regression executes a
41-frame, 64 KiB-per-frame probe on a real runtime worker in an isolated child
test process:

```text
cargo test -p ploke-eval --bin ploke-eval \
  tests::runtime_worker_stack_supports_walk_debug_frames -- --exact --nocapture
```

Before the stack budget was configured, the test process aborted with
`SIGABRT` after a `tokio-rt-worker` stack overflow. With the configured runtime,
the same test passes and returns the exact recursive checksum.

This pins the production runtime builder's worker capacity; it does not create
a synthetic R3 campaign or replace the production walk-path validation below.

## Missing Repro / Validation

There is not yet a deterministic fixture-backed subprocess regression that
creates a legitimate R3 session and drives the production server Step request
through its complete journal lifecycle. The preserved canary provides the
current production-path validation. The source fix still needs to be committed
and rebuilt before touching it. Live validation must then:

1. prove process `1130803` and its recorded incarnation are gone;
2. resolve only the typed lost-owner cause for fence 2;
3. admit the new committed binary epoch if required;
4. replay the exact R3-to-R4a step;
5. verify the server remains online and the journal records
   `Acquired -> AttemptBegan -> committed AttemptFinished -> Released` with an
   R4a cursor.

## Fix Direction

The binary runtime owns the worker stack used by spawned walk-server jobs, so
it now configures an explicit 8 MiB stack reservation. This changes no lease,
journal, transition, cancellation, or error semantics and leaves the strict
recovery requirement intact. The root CLI future remains on the calling thread
under `Runtime::block_on`.

Do not fix this by shrinking trace carriers, tolerating incomplete journal
state, blindly retrying after `Acquired`, or detaching lease-bearing work into
an unsupervised task. A future library embedding that runs `Cli::run` on an
externally supplied runtime must provide an equivalent stack contract or first
split the oversized debug poll chain without weakening cancellation behavior.

## Related Bugs

- [`2026-07-14-walk-pre-session-phase-and-start.md`](./2026-07-14-walk-pre-session-phase-and-start.md)
  establishes the authoritative R3 session used by this canary.
