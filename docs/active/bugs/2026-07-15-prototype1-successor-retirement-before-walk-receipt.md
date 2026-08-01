# Prototype 1 Successor Retirement Precedes Outer Walk Receipt

Status: fixed and live verified on a fresh stepped handoff canary
Discovered: 2026-07-15

Incident campaign:

```text
p1-stage7-handoff-canary-g35f-orembed-3g1x3-p3-20260715-023614
```

Fresh validation campaign:

```text
p1-handoff-drainfix-g35f-orembed-3g1x3-p3-20260715-112133
```

## Broken Contract

After a step-mode successor publishes its atomic R4c Ready receipt and the
predecessor releases the accepted R12-to-R13b controller attempt, the successor
walk service must remain online until the predecessor's exact outer walk
operation is durably terminal and the predecessor endpoint retires.

Controller-session release proves authority transfer. It does not by itself
prove that the sibling walk service has finished projecting and publishing the
outer operation receipt.

## Live Evidence

The successor runtime was
`41f0b802-47a4-4596-8bfc-4b219d72c0e7`. Its R4c Ready record was written at
`2026-07-15T10:34:07.568218409Z`. The predecessor controller committed R13b and
released fence 12 at approximately `10:34:07.746Z`.

The successor then attempted to stop the predecessor while outer walk job 12
was still `Running`. Its stderr was written at `10:34:15.670Z` with:

```text
predecessor endpoint ... did not retire after handoff release: ... walk stop refused while job 12 (step) is Running
```

Operation `50d299f7-ee5c-4c28-91fd-8bd31af9aaf7` became durably `Succeeded` at
`2026-07-15T10:34:20.277996111Z`, revision 49, with an R12-to-R13b receipt. The
successor's fatal stderr had been written about 4.6 seconds earlier.

The same projection delay was visible before handoff: jobs 10 and 11 settled
about 13.0 and 12.5 seconds after their inner controller releases. This was not
a provider, configuration, or persisted-typestate failure.

Primary evidence:

- `/home/brasides/.ploke-eval/walk/operations/be288492240f51eb/50d299f7-ee5c-4c28-91fd-8bd31af9aaf7.json`
- campaign `prototype1/transition-journal.jsonl`
- predecessor control session
  `845e6e61427fdd2b7e81b850f1281f8fb6d97069404c429cbef61ad0f900cd4b`
- successor control session
  `951389e16e61f1cf15ffd813922a10737c5275a3c8f539a90fd82cffa20d2be3`
- successor invocation, Ready channel, and stderr below
  `prototype1/nodes/node-7338c5e359abf3de`

The exact durable fixture is retained at
`crates/ploke-eval/src/tests/fixtures/prototype1-stage7-handoff-retirement-20260715`.

## Source Trace

The inner path committed R12-to-R13b and released the controller lease. The
successor observed that release in `serve_successor` and called
`retire_predecessor`.

The outer `run_step_job` still had to call `record_walk_event`, which performs a
synchronous owner-database projection, before it could call `finish_job` and
publish the terminal operation. `stop_active_job` correctly refused Stop while
that outer operation remained active. The predecessor retirement loop treated
all errors as retryable but allowed only five seconds, so it timed out during a
valid drain.

After the outer receipt was finally published, `finish_job` atomically set the
predecessor admission fence to stopping. That protected correctness, but the
successor process was already gone and the predecessor listener still required
an accepted Stop to exit.

## Fix

The active-operation guard remains strict. The repair changes the service
coordination contract instead:

- active `Running` or `CancelRequested` Stop refusals use the existing typed
  `WalkErrorCode::JobActive` response;
- `Indeterminate` uses `RecoveryInProgress` and is never treated as ordinary
  drain;
- predecessor retirement receives a full bounded 30-second post-capture drain
  window, matching the existing successor transfer scale;
- retirement retries only `JobActive` semantic responses; typed recovery,
  epoch, ownership, protocol, and other non-drain responses stop immediately;
- an in-flight Stop shares the remaining handoff-drain deadline, and ambiguous
  transport outcomes return through exact endpoint ownership/rebind checks;
- only a successful Stop status whose authority is `Stopping` terminates the
  listener.

This coordinates retirement with the predecessor service's typed operation
state. It does not weaken Stop admission, reorder owner-database projection
after terminal publication, or treat controller release as outer-operation
settlement.

## Regression Coverage

`stage7_retirement_replays_outer_receipt_gap` replays the exact persisted
journals through the production session reader, loads the exact invocation
through the production authority loader, and decodes the exact Ready channel
and operation record into their typed carriers. It proves Ready and controller
release preceded the outer terminal receipt by more than the former
five-second deadline.

`successor_retirement_waits_for_predecessor_job_receipt` runs the production
Unix-socket server and retirement path, keeps the exact outer handoff job active
beyond five seconds, publishes a typed R12-to-R13b terminal receipt through
`finish_job`, and requires clean endpoint retirement.

`successor_retirement_rejects_indeterminate_predecessor` proves that an
unresolved operation fails immediately with `RecoveryInProgress` and leaves the
predecessor service online.

`successor_retirement_waits_for_stop_response` holds the predecessor job
registry across the former 250 ms request deadline, proving a Stop already in
flight can finish its terminal-persistence synchronization without killing the
successor or bypassing endpoint ownership checks.

Fail-before evidence:

```text
active Stop code: RequestFailed, expected JobActive
retirement exited before 5.25 seconds while the outer handoff job was Running
```

Focused fix-after evidence:

```text
stage7_retirement_replays_outer_receipt_gap: passed
stop_requires_observed_epoch_and_awaits_task_abort: passed
successor_retirement_waits_for_predecessor_job_receipt: passed in 5.30s
successor_retirement_rejects_indeterminate_predecessor: passed
successor_retirement_waits_for_stop_response: passed in 0.38s
cargo test -p ploke-eval --lib: 1323 passed, 0 failed, 28 ignored
```

## Fresh Live Validation

The repaired source lineage was exercised through a fresh CLI-driven step-mode
run. Successor runtime `f91b8692-982e-49af-9658-1d15ea459a97` published its
typed R4c Ready receipt at `2026-07-15T12:31:07.411316513Z`. The predecessor's
outer operation `48113938-affa-4d81-a6e9-bb59fe937bdd` then became durably
`Succeeded` at `2026-07-15T12:31:20.062050924Z`, revision 53, with an
R12-to-R13b receipt.

The approximately 12.65-second interval exceeded the former five-second
retirement deadline and therefore exercised the repaired drain path. During
that interval the successor remained online. After the outer receipt became
terminal, predecessor PID `1648138` and socket
`p1walk-fb6cf8c18c26c391.sock` retired, while successor PID `1802768` and socket
`p1walk-fb6cf8c18c26c391-f91b8692.sock` remained reachable. Unpinned walk status
followed the durable endpoint to successor session
`01742ea6-6f33-4f52-8197-08f5cda3f530` at R4c with active mutation authority.

Primary evidence:

- `/home/brasides/.ploke-eval/walk/operations/fb6cf8c18c26c391/48113938-affa-4d81-a6e9-bb59fe937bdd.json`
- `prototype1/transition-journal.jsonl`
- `prototype1/control/sessions/d706b89e2ffd2525d50eaa10961e83df0ed6ef5b7b4eb044f876120075894188/control-journal.jsonl`
- `prototype1/nodes/node-1c6b86abba9f76c7/invocations/f91b8692-982e-49af-9658-1d15ea459a97.json`
- `prototype1/nodes/node-1c6b86abba9f76c7/channels/f91b8692-982e-49af-9658-1d15ea459a97/child-to-parent.jsonl`

This closes the fresh-live validation gap for step-mode receipt draining and
endpoint transfer. It does not yet prove continuous-mode handoff or R14b
completion.

## Remaining Design Follow-up

The 30-second drain window is now an explicit handoff policy. Exceeding it
fails closed and preserves both endpoint and operation evidence; it does not
weaken Stop admission. A later protocol revision may carry the exact outer walk
operation ID in the successor proof, which would provide richer correlation
than the current single-admitted-job invariant. That wider carrier change is
not required for this incident repair.

## Run Disposition

Stage 7 remains recoverable historical evidence. Its outer predecessor
operation and inner controller session both committed R13b, while the successor
session durably reached R4c. The predecessor was retired through the supported
Stop path after receipt reconciliation; no run artifact was rewritten or
falsely abandoned.

The fresh stepped handoff canary proved that the repaired binary keeps the
successor endpoint reachable and active while the live outer receipt settles.
Continuous-mode handoff and R14b completion remain separate validation steps.

## Related Documents

- [`2026-06-08-prototype1-successor-handoff-stale-parent-identity.md`](./2026-06-08-prototype1-successor-handoff-stale-parent-identity.md)
- [`../agents/2026-06-30_p1-gated-live-run-handoff.md`](../agents/2026-06-30_p1-gated-live-run-handoff.md)
- [`../agents/2026-07-13_prototype1-loop-operator-control-plan.md`](../agents/2026-07-13_prototype1-loop-operator-control-plan.md)
