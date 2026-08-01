# Prototype 1 Child Runner Processes Are Not Reaped

Status: source repaired in `7b862e4e9`; fresh live repair validation pending
Discovered: 2026-07-15

Campaign:

```text
p1-handoff-drainfix-g35f-orembed-3g1x3-p3-20260715-112133
```

## Broken Contract

A parent that spawns a Prototype 1 child runtime must retain or explicitly
transfer operating-system lifecycle ownership until that process has been
waited and reaped. The typed terminal `ToParent::Result` remains semantic
completion authority, but channel evidence does not reap an OS process.

The same owner must terminate and wait a child that fails before the durable
Ready acknowledgement. A journal projection must not describe a timed-out
child as terminated while the process is still running.

## Live Evidence

The fresh stepped canary spawned three child runtimes from predecessor PID
`1648138`:

| Child PID | Runtime | Node |
|---:|---|---|
| `1778831` | `642b14a4-4431-4db1-9d2b-7a03f44af5a4` | `node-1c6b86abba9f76c7` |
| `1778866` | `fe3469e0-a106-4ffd-a1f8-d84a22edeee9` | `node-00850047cd74d296` |
| `1778901` | `03d0f503-8327-4d6d-8620-2302fa93e21a` | `node-91e924cec258643e` |

All three had exact Linux process-incarnation evidence in their `Spawned` and
`Observed(Acknowledged)` journal entries. All three then wrote successful
runner results with exit code zero. A live process check after those results
showed every PID as `Z` / `<defunct>` with PPID `1648138`. They remained
defunct for several minutes and disappeared only when that predecessor process
retired after successor handoff.

Primary durable evidence:

- `prototype1/transition-journal.jsonl`
- `prototype1/nodes/node-1c6b86abba9f76c7/results/642b14a4-4431-4db1-9d2b-7a03f44af5a4.json`
- `prototype1/nodes/node-00850047cd74d296/results/fe3469e0-a106-4ffd-a1f8-d84a22edeee9.json`
- `prototype1/nodes/node-91e924cec258643e/results/03d0f503-8327-4d6d-8620-2302fa93e21a.json`

The transition and result records durably bind the parent, child PID, exact
incarnation, runtime, node, and semantic result. The transient `Z` process
state itself came from the live OS process table and was not rewritten into
campaign authority.

## Prior Expectation and Recurrence

The [runtime discovery draft](../../workflow/evalnomicon/drafts/runtime/child.md)
already identified that the Ready-timeout path did not visibly perform the
explicit kill/wait used by successor startup.

A June 2 run review also recorded defunct earlier child `ploke-eval` processes
while the parent and a later child remained active in the
[R4 broad-harness review](../agents/run-reviews/2026-06-02-p1-gemini35-flash-direct-3g2x3-par2-20260602-131345-node-26f01da56959fd47-r4-broad-harness.md).

The July canary therefore reproduced a previously visible lifecycle gap rather
than a one-off provider or configuration failure.

## Root Cause

`SpawnChild::transition` owned `std::process::Child` only while polling for the
typed Ready message. On success it returned C4, whose carrier retained only the
`RuntimeId`; dropping `Child` does not wait or reap it. `ObserveChild::transition`
later consumed the semantic terminal channel message but had no process handle.

The same seam had two related failure-path defects:

- `ReadyTimedOut` projected `ChildRuntimeLifecycle::Terminated` without killing
  and waiting the still-live process;
- channel-read or process-poll errors returned through `?`, dropping the handle
  without cleanup.

`terminate_spawned_child` already killed the isolated child process group and
waited it for selected early spawn failures. The ownership transfer after Ready
and the other wait failures bypassed that helper.

## Repair

Commit `7b862e4e9` keeps one explicit lifecycle owner across the C3-to-C4
authority boundary:

- a local `ChildReaper` takes the process handle after Ready but remains under
  transition control while fallible pre-ack projections and journal work run;
- pre-ack failure sends `Terminate` and joins the waiter before returning;
- the durable `Observed(Acknowledged)` journal append is the C4 authority
  boundary;
- after that acknowledgement, the reaper receives `Wait` and reaps natural
  child exit without interpreting exit status as semantic success;
- a later node-projection error leaves the acknowledged child running under
  the waiter so journal-based recovery remains truthful;
- reaper-thread creation failure returns the original process handle for
  kill-and-wait cleanup;
- Ready timeout now kills and waits the isolated process group;
- channel-read and process-poll errors pass through the same cleanup path.

The repair does not weaken typed terminal-result authority, infer evaluation
success from an exit code, or make a malformed lifecycle state admissible.

## Regression Coverage

Fail-before production-path evidence:

```text
child_spawn_observes_ready ... FAILED
acknowledged child process 1805428 must be reaped after it exits
```

Fix-after coverage:

- `child_spawn_observes_ready` drives `run_planned_child`, requires the fake
  child to reach a post-sleep normal-completion marker, then requires its exact
  Linux incarnation to disappear while the parent test process remains alive;
- `child_reaper_abort_terminates_and_reaps_child` proves a pre-commit abort
  kills and joins the isolated process;
- `ready_timeout_terminates_and_reaps_child` proves timeout cleanup kills and
  waits rather than merely projecting termination;
- the C3 unit set passes 5/5;
- `cargo test -p ploke-eval --lib` passes 1,325 tests with 28 ignored and no
  failures.

## Remaining Validation

A fresh live child fanout from a binary containing `7b862e4e9` is still needed
to close live repair validation. That run should retain the parent after child
completion long enough to confirm each recorded incarnation has disappeared.

Termination policy after the later C4 child-result stale timeout is a separate
design question. The current repair guarantees an OS waiter remains attached;
it does not introduce a new policy to kill an acknowledged child merely because
semantic result observation timed out.

An injected transition-level test for a pre-ack projection or journal failure
would strengthen recovery coverage, but the reviewed repair has direct tests
for both reaper control actions and the production normal-exit path.

## Run Disposition

The source canary remains valid evidence. All three children produced typed
successful results and the parent later completed successor handoff; the
defunct processes disappeared when the predecessor retired. No campaign file,
result, node record, or journal entry was rewritten to hide the lifecycle bug.

## Related Documents

- [`2026-07-15-prototype1-successor-retirement-before-walk-receipt.md`](./2026-07-15-prototype1-successor-retirement-before-walk-receipt.md)
- [`../agents/2026-07-13_prototype1-loop-operator-control-plan.md`](../agents/2026-07-13_prototype1-loop-operator-control-plan.md)
- [`2026-06-09-prototype1-late-child-result-recovery-corrupts-node-state.md`](./2026-06-09-prototype1-late-child-result-recovery-corrupts-node-state.md)
