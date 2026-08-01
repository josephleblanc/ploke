# Prototype 1 Post-Edit Indexer Failure Leaves Broad Lane Nonterminal

Status: source repair implemented and locally verified; the v14 campaign is
permanently abandoned for loop progress and retained as evidence. Fresh live
campaign validation remains required.

Discovered: 2026-07-16

## Broken Contract

The declared post-apply refresh deadline must cover the scan barrier and
terminalize the harness lane when that barrier does not resolve. A queued scan
must not remain outside the deadline until the broader attempt timeout.

## Scope

Campaign:

```text
p1-v14-livehandoff-keeponly-g35f-oropenai-3g1x3-p3-20260716-222839
```

Campaign root:

```text
/home/brasides/.ploke-eval/campaigns/p1-v14-livehandoff-keeponly-g35f-oropenai-3g1x3-p3-20260716-222839
```

The admitted walk epoch used parent commit
`ad2555771227d2b2dc1f4960d5a504f9f3bb9ca4`, executable modification time
`1784266225767`, and source-status hash
`29d2225ce4ca9d62cc8bac4397e019c73e8b655e27400908126402230caa6fec`.

## Evidence

| Surface | Observation |
| --- | --- |
| `prototype1/control/sessions/618c41d4e8b49954ad3eecfeb57978d13b6e574cb464336ab2fbaa7d0c9d33df/control-journal.jsonl` | Fence 8 began transition `2d5d964f-5a06-5e6b-898d-ad39bc3afa91` from R7 to R8. No matching `finished` record exists; the later recovery record is `abandon_session`. |
| `/home/brasides/.ploke-eval/walk/operations/dd29705cbda947d8/72d2e92c-97d5-47f1-b421-7ef1cbe9aadb.json` | Walk job 3 started at R7 with target R12 and journal revision 27. It was later marked `abandoned`, still at R7, with observed revision 29 and a warning that prior effects remain possible. |
| `prototype1/debug/tool-loop/96a48666-2d8b-46fd-a7a5-ce1a80e4eb00/session.json` | The base lane remained `paused` and was linked to the pending R7-to-R8 transition. |
| `prototype1/debug/tool-loop/96a48666-2d8b-46fd-a7a5-ce1a80e4eb00/resume.json` | `terminal` is `false` and `next_step` is 47. Snapshot `0046.json` is the last durable step; there is no `0047.json`. |
| `prototype1/workspaces/edit-harness/node-7c2b02875989a10d/crates/ploke-db/src/database.rs` | The file was modified at `2026-07-16 23:05:05.071362083 -0700`, after snapshot 46 and the `resume.json` write at about 23:05:00. |
| `prototype1/messages/edit-harness-result/node-7c2b02875989a10d.json` | No submitted base-lane result exists. A late `.headless-tui.json` sidecar eventually classified the dirty attempt as `applied_timed_out` after 1,800 seconds and correctly refused admission. |
| `prototype1/messages/edit-harness-result/node-7c2b02875989a10d-r2.json` and `node-7c2b02875989a10d-r3.json` | The sibling r2 and r3 lanes had already reached terminal submitted results. The stalled base lane, not provider fanout, held the batch/outer edge open. |
| `/home/brasides/.ploke-eval/logs/ploke_eval_20260716_223456_2714163.log` | The process emitted callback-manager shutdown attempts without clean closure, then three `Indexing Failed` messages at 23:19:22, 23:21:12, and 23:22:43. Each reported an indexer task panic with `Test timed out without completion signal`. Dense indexing continued after those failures. At 23:23:43 the state-manager channel was closed, the chat session was cancelled, and only then did the 1,800-second applied timeout reach the admission guard. |

Transient live process inspection ruled out an in-flight provider wait. At
23:14 local time, server PID `2714163` had no owned TCP connection, consumed
about 171% CPU and 26.9% of system memory, and had 52 threads. At 23:16 its
resident set was about 8.3 GiB, with three Tokio workers consuming
approximately 99%, 99%, and 78% CPU. At 23:21 it still consumed about 180% CPU
while the lane snapshots and operation receipt had not changed.

The strict recovery evidence is:

```text
/home/brasides/.ploke-eval/walk/operations/dd29705cbda947d8/2c242fc4-c67b-4b24-8722-0aa08ee76f90.json
```

Recovery job 4 records the controller session as permanently abandoned at
`AttemptPending` R7-to-R8 and directs the operator to preserve the run and
start fresh.

## Source Trace

The verified blocking path is:

```text
non_semantic_patch mutates database.rs
-> proposal approval starts the normal post-edit scan
-> settle_staged_batch starts its explicit post-apply refresh barrier
-> wait_for_refresh sends ScanPathsForChange with a 600-second deadline
-> wait_for_refresh awaits scan_rx without applying that deadline
-> the state manager remains occupied by the earlier scan/transform
-> the queued barrier resolves only after about 1,139 seconds
-> the 600-second refresh budget is never observed
-> run_broad_slot_for_admission returns only at the 1,800-second outer timeout
-> the controller still lacks R8 when the server exits and the operation is abandoned
```

The server log records the base lane's explicit scan request at 23:05:05 and
the earlier scan completing at 23:24:04. The latter reports
`total_ms=1137069`; the queued barrier then begins only after the declared
600-second refresh deadline has already elapsed.

Dense indexer timeout panics and callback shutdown failures occurred during the
same interval and remain a lifecycle concern, but they are not the await point
that held this lane. The lane was blocked on the unbounded scan oneshot before
it could enter the sparse/dense readiness gate.

Relevant source boundaries:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`
  owns the post-apply scan barrier and refresh deadline.
- `crates/ploke-tui/src/app_state/database.rs` owns the scan/parse/transform
  work that kept the state manager occupied.
- `crates/ingest/ploke-embed/src/indexer/mod.rs` still owns the concurrent
  dense task/callback lifecycle and currently panics on its own timeout.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/harness/tui.rs`
  `TuiHarness::drive_to_attempt_end` owns prompt lane termination.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  `run_broad_slot_for_admission`, `admit_broad_batch`, and
  `finish_broad_headless_tui_attempt` connect lane completion to the child plan
  and R8 transition.

`finish_broad_headless_tui_attempt` behaved correctly once the outer timeout
finally arrived: it rejected `AppliedTimedOut` rather than publishing a dirty
candidate. That downstream admission guard is not the cause and must remain
strict.

## Docs And Policy Expectation

[`crates/ploke-eval/docs/reference/knobs/timeouts.md`](../../../crates/ploke-eval/docs/reference/knobs/timeouts.md)
documents bounded post-apply indexing and a bounded outer attempt. An indexing
failure must therefore resolve to typed terminal evidence; it must not become
unbounded background work hidden behind the outer wall clock.

The walk epoch binds the executable, Git head, and source-status hash. Repairing
the running checkout or replacing its binary would invalidate that admitted
history. No digest, run artifact, candidate workspace, or validation check
should be rewritten to make v14 resumable.

## Current Repro Coverage

Existing coverage proves adjacent but narrower contracts:

- `ssot_forwards_indexing_failed_once` proves the event bus forwards one
  indexing failure.
- `sparse_post_apply_refresh_returns_on_bm25_without_dense_index_completion`
  proves sparse readiness does not wait for dense completion.
- `applied_timed_out_headless_tui_blocks_submitted_result_with_typed_detail`
  proves the final downstream guard rejects a timed-out applied candidate.
- `test_runtime_actor_guard_retains_full_stack_handles` proves handle
  ownership, not that a failed post-edit indexer terminalizes and is reaped.

The new `scan_barrier_times_out` regression now proves that an unresolved scan
oneshot returns a typed error at the post-apply deadline. The existing sparse
refresh canary still passes, proving the new barrier does not make sparse mode
wait for dense completion.

## Remaining Validation

Replay the v14 self-edit request through the repaired production path, then
validate the same direct-Google broad-harness shape in a fresh campaign. The
fresh run must show that a slow scan either completes within the refresh budget
or produces typed non-admission evidence before the outer timeout, allowing the
other terminal lanes and R7-to-R8 transition to settle normally. Do not replay
the repaired binary into v14 as handoff proof.

Deterministic cancellation/join of the dense indexer and callback work remains
separate lifecycle hardening. It must not be used to weaken the scan deadline,
the `AppliedTimedOut` guard, or candidate admission.

## Fix

`wait_for_refresh` now routes the `ScanPathsForChange` oneshot through
`await_scan_barrier`, which applies the already-computed refresh deadline and
returns a typed `HeadlessEvent` timeout. The change is limited to the barrier;
it does not change the timeout value, scan semantics, indexing semantics, or
candidate admission.

Local verification:

```text
cargo test -p ploke-eval scan_barrier_times_out -- --nocapture
cargo test -p ploke-eval sparse_post_apply_refresh_returns_on_bm25_without_dense_index_completion -- --nocapture
RUST_MIN_STACK=8388608 cargo test -p ploke-eval -- --test-threads=1
```

Both focused tests pass. The serial crate verification passes with 1,376 tests,
zero failures, and 45 ignored tests. The 8 MiB stack setting is the existing
production walk-runtime contract recorded in
[`2026-07-14-ploke-eval-walk-worker-stack-overflow.md`](./2026-07-14-ploke-eval-walk-worker-stack-overflow.md);
without it, the known R3 reconstruction test overflows the default test-thread
stack. A parallel full-suite run also exposed an unrelated shared endpoint-lock
race in the setup-preview test; that test passes both in isolation and in the
serial suite. Same-target live replay remains tracked in the validation above.

Do not lengthen the timeout, ignore dense failures, admit the dirty base
candidate, reinterpret the missing result, or weaken epoch/digest validation.

## Disposition

Abandon-and-restart. V14 has a post-edit workspace mutation and an unresolved
admitted R7-to-R8 attempt under a sealed executable/source epoch. The campaign
is stop-use evidence. Repair and regression work must happen in source, and
handoff validation must use a newly admitted campaign.

## Related Bugs

- [`2026-05-22-prototype1-google-post-apply-indexing-timeout.md`](./2026-05-22-prototype1-google-post-apply-indexing-timeout.md)
- [`2026-06-04-prototype1-headless-timeout-after-apply.md`](./2026-06-04-prototype1-headless-timeout-after-apply.md)
- [`2026-06-04-prototype1-headless-tui-runtime-actor-leak.md`](./2026-06-04-prototype1-headless-tui-runtime-actor-leak.md)
- [`2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md`](./2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md)
- [`2026-07-16-prototype1-headless-unpersisted-workspace-mutation.md`](./2026-07-16-prototype1-headless-unpersisted-workspace-mutation.md)
